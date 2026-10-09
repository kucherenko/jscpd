// test_server.rs — a scripted HTTP server on 127.0.0.1 for the tests of the
// embeddings API and the model download: no test reaches the network.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// What the server does with one connection.
pub enum Reply {
    /// A whole response.
    Http {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    },
    /// These bytes as they are, then the connection closes: a response cut
    /// short, or nothing at all.
    Raw(Vec<u8>),
}

impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self::Http {
            status,
            headers: vec![("Content-Type", "application/json".into())],
            body: body.as_bytes().to_vec(),
        }
    }

    pub fn status(status: u16, body: &str) -> Self {
        Self::Http {
            status,
            headers: vec![],
            body: body.as_bytes().to_vec(),
        }
    }

    pub fn with_header(mut self, name: &'static str, value: &str) -> Self {
        if let Self::Http { headers, .. } = &mut self {
            headers.push((name, value.to_string()));
        }
        self
    }
}

/// One request the server got.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// Header names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// A server answering one connection per reply, in order, then gone.
pub struct Server {
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let thread = std::thread::spawn(move || {
            for reply in replies {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                answer(stream, reply, &seen);
            }
        });
        Self {
            url,
            requests,
            thread: Some(thread),
        }
    }

    /// The requests so far, once every reply was given.
    pub fn requests(mut self) -> Vec<Request> {
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
        self.requests.lock().unwrap().clone()
    }

    /// The requests so far, without waiting for the rest of the script.
    pub fn requests_so_far(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

/// A URL where nothing listens.
pub fn refused_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// Read one request into `seen`, then send `reply`. The request is kept
/// before the reply goes out, so a client that has its answer finds its
/// request in [`Server::requests_so_far`].
fn answer(stream: TcpStream, reply: Reply, seen: &Mutex<Vec<Request>>) -> Option<()> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let mut body = Vec::new();
    if let Some(length) = header("content-length").and_then(|l| l.parse::<usize>().ok()) {
        body.resize(length, 0);
        reader.read_exact(&mut body).ok()?;
    } else if header("transfer-encoding").is_some_and(|t| t.contains("chunked")) {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).ok()?;
            let size = usize::from_str_radix(size.trim(), 16).ok()?;
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk).ok()?;
            if size == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..size]);
        }
    }
    seen.lock().unwrap().push(Request {
        method,
        path,
        headers,
        body,
    });
    let mut stream = stream;
    match reply {
        Reply::Http {
            status,
            headers: extra,
            body: payload,
        } => {
            let mut head = format!(
                "HTTP/1.1 {status} Status\r\nContent-Length: {}\r\nConnection: close\r\n",
                payload.len()
            );
            for (name, value) in extra {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&payload);
        }
        Reply::Raw(bytes) => {
            let _ = stream.write_all(&bytes);
        }
    }
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Some(())
}
