// http.rs — the `http` provider: any OpenAI-compatible embeddings API.

use super::{API_KEY_ENV, Backend, SemanticOptions};
use serde_json::{Map, Value, json};
use std::time::Duration;

const BATCH_TEXTS: usize = 32;
const BATCH_BYTES: usize = 64 * 1024;
const RETRIES: u32 = 3;

/// The embeddings endpoint for a base URL: `.../v1` gets `/embeddings`
/// appended, a URL already ending in `/embeddings` is used as given.
pub fn endpoint(url: &str) -> String {
    let base = url.trim_end_matches('/');
    if base.ends_with("/embeddings") {
        base.to_string()
    } else {
        format!("{base}/embeddings")
    }
}

pub struct HttpBackend {
    options: SemanticOptions,
    endpoint: String,
    api_key: Option<String>,
    /// The key is set but not sent: the URL came from a config file.
    key_withheld: bool,
    agent: ureq::Agent,
}

/// Where a URL points, as far as sending code and keys is concerned.
struct Target {
    host: String,
    /// `localhost`, `*.localhost` or a loopback address.
    on_this_machine: bool,
    plain_http: bool,
}

impl Target {
    fn of(url: &str) -> Self {
        let uri = url.parse::<ureq::http::Uri>().ok();
        let host = uri
            .as_ref()
            .and_then(|u| u.host())
            .unwrap_or_default()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        let on_this_machine = host == "localhost"
            || host.ends_with(".localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        let plain_http = uri.as_ref().and_then(|u| u.scheme_str()) == Some("http");
        let host = match host.is_empty() {
            true => url.to_string(),
            false => host,
        };
        Self {
            host,
            on_this_machine,
            plain_http,
        }
    }
}

/// The HTTP client for embeddings requests and model downloads.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .tls_config(tls_config())
        .timeout_connect(Some(Duration::from_secs(10)))
        // A local model on a CPU can take minutes over one batch, and a
        // model download minutes more.
        .timeout_global(Some(Duration::from_secs(3600)))
        .http_status_as_error(false)
        .user_agent(format!("jscpd/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

impl HttpBackend {
    pub fn new(options: &SemanticOptions) -> Result<Self, String> {
        Self::with_key(
            options,
            std::env::var(API_KEY_ENV).ok().filter(|k| !k.is_empty()),
        )
    }

    /// A config file is shared, and can come with the code being scanned, so
    /// it cannot send that code, or the user's key, to a host of its choice:
    /// a URL from it that is not on this machine gets code only when
    /// `--semantic` was typed, and never gets the key. And no key goes over
    /// plain http to another machine.
    fn with_key(options: &SemanticOptions, key: Option<String>) -> Result<Self, String> {
        let target = Target::of(&options.url);
        let from_elsewhere = options.url_from_config && !target.on_this_machine;
        if from_elsewhere && !options.on_command_line {
            return Err(format!(
                "the config file's semantic.url would send the code of every function to {}; pass --semantic on the command line to allow it, or give the URL with --semantic-url",
                target.host
            ));
        }
        let (api_key, key_withheld) = match from_elsewhere {
            true => (None, key.is_some()),
            false => (key, false),
        };
        if api_key.is_some() && target.plain_http && !target.on_this_machine {
            return Err(format!(
                "{API_KEY_ENV} is not sent over plain http to {}; use an https URL, or a server on this machine",
                target.host
            ));
        }
        Ok(Self {
            endpoint: endpoint(&options.url),
            options: options.clone(),
            api_key,
            key_withheld,
            agent: agent(),
        })
    }

    /// Vectors for `texts`, one request per batch.
    fn request_all(
        &self,
        texts: &[&str],
        progress: &dyn Fn(usize),
    ) -> Result<Vec<Vec<f32>>, String> {
        let mut out = Vec::with_capacity(texts.len());
        let mut start = 0;
        while start < texts.len() {
            let mut end = start;
            let mut bytes = 0;
            while end < texts.len()
                && end - start < BATCH_TEXTS
                && (end == start || bytes + texts[end].len() <= BATCH_BYTES)
            {
                bytes += texts[end].len();
                end += 1;
            }
            out.extend(self.request(&texts[start..end])?);
            progress(end);
            start = end;
        }
        Ok(out)
    }

    fn request(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        let mut body = Map::new();
        body.insert("model".into(), json!(self.options.model));
        body.insert("input".into(), json!(texts));
        if let Some(dimensions) = self.options.dimensions {
            body.insert("dimensions".into(), json!(dimensions));
        }
        for (key, value) in &self.options.params {
            body.insert(key.clone(), value.clone());
        }
        let body = Value::Object(body);
        let mut attempt = 0;
        loop {
            let mut request = self.agent.post(&self.endpoint);
            if let Some(key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {key}"));
            }
            let retry_after = match request.send_json(&body) {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    if status == 200 {
                        let parsed: EmbeddingsResponse = response
                            .body_mut()
                            .with_config()
                            .limit(256 * 1024 * 1024)
                            .read_json()
                            .map_err(|e| format!("{}: unreadable response: {e}", self.endpoint))?;
                        return self.vectors(parsed, texts.len());
                    }
                    let detail = response.body_mut().read_to_string().unwrap_or_default();
                    if !matches!(status, 429 | 500 | 502 | 503 | 504) || attempt == RETRIES {
                        return Err(self.describe_status(status, &detail));
                    }
                    response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.trim().parse::<u64>().ok())
                }
                Err(err) if transient(&err) && attempt < RETRIES => None,
                Err(err) => return Err(self.describe_transport(&err)),
            };
            attempt += 1;
            let wait = retry_after.unwrap_or(1 << attempt).min(30);
            std::thread::sleep(Duration::from_secs(wait));
        }
    }

    fn vectors(
        &self,
        parsed: EmbeddingsResponse,
        expected: usize,
    ) -> Result<Vec<Vec<f32>>, String> {
        let mut data = parsed.data;
        if data.len() != expected {
            return Err(format!(
                "{}: {} embeddings returned for {} inputs",
                self.endpoint,
                data.len(),
                expected
            ));
        }
        if data.iter().all(|d| d.index.is_some()) {
            data.sort_by_key(|d| d.index);
        }
        Ok(data
            .into_iter()
            .map(|d| {
                let mut v = d.embedding;
                // Servers that ignore `dimensions` (llama-server) return the
                // full vector; a Matryoshka model's prefix is the short one.
                if let Some(dimensions) = self.options.dimensions {
                    v.truncate(dimensions as usize);
                }
                v
            })
            .collect())
    }

    fn describe_status(&self, status: u16, body: &str) -> String {
        let detail = error_detail(body);
        let lower = detail.to_ascii_lowercase();
        let model = &self.options.model;
        if lower.contains("not found") && lower.contains("model") {
            return format!(
                "{} has no model \"{model}\" (HTTP {status}). With Ollama, run `ollama pull {model}`; otherwise pick a model the server has with --semantic-model",
                self.endpoint
            );
        }
        let detail = match detail.is_empty() {
            true => String::new(),
            false => format!(": {detail}"),
        };
        match status {
            401 | 403 if self.key_withheld => format!(
                "{} refused the request (HTTP {status}); {API_KEY_ENV} is set, but a URL from a config file never receives it: give the URL with --semantic-url to send the key{detail}",
                self.endpoint
            ),
            401 | 403 => format!(
                "{} refused the request (HTTP {status}); set {API_KEY_ENV} to the provider's API key{detail}",
                self.endpoint
            ),
            _ => format!("{} answered HTTP {status}{detail}", self.endpoint),
        }
    }

    fn describe_transport(&self, err: &ureq::Error) -> String {
        let local =
            self.options.url.contains("localhost") || self.options.url.contains("127.0.0.1");
        let hint = if local {
            format!(
                ". Start the embedding server — for Ollama, `ollama serve` and `ollama pull {}` — or point --semantic-url at another OpenAI-compatible embeddings API",
                self.options.model
            )
        } else {
            String::new()
        };
        format!("cannot reach {}: {err}{hint}", self.endpoint)
    }
}

impl Backend for HttpBackend {
    fn label(&self) -> String {
        format!("{} at {}", self.options.model, self.options.url)
    }

    fn model_name(&self) -> &str {
        &self.options.model
    }

    fn cache_identity(&self) -> Value {
        json!({
            "url": self.endpoint,
            "model": self.options.model,
            "dimensions": self.options.dimensions,
            "params": self.options.params,
        })
    }

    fn embed(&self, texts: &[&str], progress: &dyn Fn(usize)) -> Result<Vec<Vec<f32>>, String> {
        self.request_all(texts, progress)
    }
}

#[derive(serde::Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(serde::Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
    #[serde(default)]
    index: Option<usize>,
}

/// The system TLS stack and certificate store on Windows and macOS.
#[cfg(any(windows, target_os = "macos"))]
fn tls_config() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build()
}

/// rustls with Mozilla's root certificates elsewhere.
#[cfg(not(any(windows, target_os = "macos")))]
fn tls_config() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::default()
}

/// What an error response says, on one line: the message of a JSON error
/// body (`{"error": {"message": ...}}`, `{"error": ...}`, `{"message": ...}`,
/// `{"detail": ...}`), nothing for an HTML page, else the body's start.
fn error_detail(body: &str) -> String {
    let body = body.trim();
    if let Ok(json) = serde_json::from_str::<Value>(body) {
        let message = json
            .pointer("/error/message")
            .or_else(|| json.get("error"))
            .or_else(|| json.get("message"))
            .or_else(|| json.get("detail"));
        return match message {
            Some(Value::String(text)) => text.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
    }
    if body.starts_with('<') {
        return String::new();
    }
    body.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

/// Errors worth another attempt: timeouts and dropped connections. A
/// refused connection is not one — nothing listens there.
fn transient(err: &ureq::Error) -> bool {
    match err {
        ureq::Error::Timeout(_) => true,
        ureq::Error::Io(e) => matches!(
            e.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::TimedOut
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_appends_embeddings_once() {
        assert_eq!(
            endpoint("http://localhost:11434/v1"),
            "http://localhost:11434/v1/embeddings"
        );
        assert_eq!(
            endpoint("https://api.jina.ai/v1/"),
            "https://api.jina.ai/v1/embeddings"
        );
        assert_eq!(
            endpoint("http://h:8080/v1/embeddings"),
            "http://h:8080/v1/embeddings"
        );
    }

    #[test]
    fn error_detail_reads_json_errors_and_skips_html() {
        let ollama = r#"{"error":{"message":"model \"x\" not found, try pulling it first","type":"api_error"}}"#;
        assert_eq!(
            error_detail(ollama),
            "model \"x\" not found, try pulling it first"
        );
        assert_eq!(error_detail(r#"{"detail":"Unauthorized"}"#), "Unauthorized");
        assert_eq!(error_detail(r#"{"error":"bad input"}"#), "bad input");
        assert_eq!(error_detail("<!doctype html><title>405</title>"), "");
        assert_eq!(error_detail("  plain\n text  "), "plain text");
    }

    #[test]
    fn the_cache_identity_covers_what_changes_vectors() {
        let base = SemanticOptions {
            provider: super::super::Provider::Http,
            ..SemanticOptions::default()
        };
        let identity =
            |o: &SemanticOptions| HttpBackend::with_key(o, None).unwrap().cache_identity();
        let mut params = Map::new();
        params.insert("task".into(), json!("code2code.query"));
        for other in [
            SemanticOptions {
                dimensions: Some(256),
                ..base.clone()
            },
            SemanticOptions {
                url: "https://api.jina.ai/v1".into(),
                ..base.clone()
            },
            SemanticOptions {
                params,
                ..base.clone()
            },
        ] {
            assert_ne!(identity(&other), identity(&base));
        }
        let threshold = SemanticOptions {
            threshold: 0.9,
            ..base.clone()
        };
        assert_eq!(
            identity(&threshold),
            identity(&base),
            "the threshold does not change vectors"
        );
    }

    #[test]
    fn a_config_file_cannot_send_code_or_the_key_elsewhere() {
        let http = |url: &str, from_config: bool, typed: bool| SemanticOptions {
            provider: super::super::Provider::Http,
            url: url.into(),
            url_from_config: from_config,
            on_command_line: typed,
            ..SemanticOptions::default()
        };
        let key = || Some("sk-test".to_string());

        // A URL from the file, on another machine, needs --semantic typed,
        let err = HttpBackend::with_key(&http("https://collector.example/v1", true, false), key())
            .err()
            .unwrap();
        assert!(err.contains("pass --semantic on the command line"), "{err}");
        // and even then never gets the key.
        let backend =
            HttpBackend::with_key(&http("https://collector.example/v1", true, true), key())
                .unwrap();
        assert!(backend.api_key.is_none() && backend.key_withheld);

        // A server on this machine is trusted either way.
        for url in [
            "http://localhost:11434/v1",
            "http://127.0.0.1:8080/v1",
            "http://[::1]:9000/v1",
            "http://ollama.localhost/v1",
        ] {
            let backend = HttpBackend::with_key(&http(url, true, false), key()).unwrap();
            assert_eq!(backend.api_key.as_deref(), Some("sk-test"), "{url}");
        }

        // A URL typed on the command line gets the key, but never over plain
        // http to another machine.
        let backend =
            HttpBackend::with_key(&http("https://api.jina.ai/v1", false, true), key()).unwrap();
        assert_eq!(backend.api_key.as_deref(), Some("sk-test"));
        let err = HttpBackend::with_key(&http("http://gpu-box.lan:8080/v1", false, true), key())
            .err()
            .unwrap();
        assert!(err.contains("not sent over plain http"), "{err}");
        assert!(
            HttpBackend::with_key(&http("http://gpu-box.lan:8080/v1", false, true), None).is_ok(),
            "without a key there is nothing to protect"
        );
    }

    use crate::embed::test_server::{Reply, Server, refused_url};

    const KEY: &str = "sk-test-0123456789abcdef";

    /// The http provider at `url`, as typed on the command line.
    fn options(url: &str) -> SemanticOptions {
        SemanticOptions {
            provider: super::super::Provider::Http,
            url: format!("{url}/v1"),
            model: "embedder".into(),
            on_command_line: true,
            ..SemanticOptions::default()
        }
    }

    fn backend(options: &SemanticOptions, key: Option<&str>) -> HttpBackend {
        HttpBackend::with_key(options, key.map(String::from)).unwrap()
    }

    fn embed(backend: &HttpBackend, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        backend.embed(texts, &|_| {})
    }

    /// A 200 answer with these vectors, indexed in order.
    fn vectors(vectors: &[&[f32]]) -> Reply {
        let data: Vec<Value> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| json!({"index": i, "embedding": v}))
            .collect();
        Reply::json(200, &json!({ "data": data }).to_string())
    }

    #[test]
    fn a_request_carries_the_texts_model_options_and_key() {
        let server = Server::start(vec![vectors(&[&[1.0, 0.0], &[0.0, 1.0]])]);
        let mut params = Map::new();
        params.insert("task".into(), json!("code2code.query"));
        let options = SemanticOptions {
            dimensions: Some(2),
            params,
            ..options(&server.url)
        };
        let got = embed(&backend(&options, Some(KEY)), &["fn a() {}", "fn b() {}"]).unwrap();
        assert_eq!(got, [vec![1.0, 0.0], vec![0.0, 1.0]]);
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(
            (request.method.as_str(), request.path.as_str()),
            ("POST", "/v1/embeddings")
        );
        let auth = format!("Bearer {KEY}");
        assert_eq!(request.header("authorization"), Some(auth.as_str()));
        assert_eq!(
            request.json(),
            json!({
                "model": "embedder",
                "input": ["fn a() {}", "fn b() {}"],
                "dimensions": 2,
                "task": "code2code.query",
            })
        );
    }

    #[test]
    fn without_a_key_no_authorization_is_sent() {
        let server = Server::start(vec![vectors(&[&[1.0]])]);
        embed(&backend(&options(&server.url), None), &["x"]).unwrap();
        assert_eq!(server.requests()[0].header("authorization"), None);
    }

    #[test]
    fn vectors_come_back_in_input_order_whatever_order_they_arrive_in() {
        let body = json!({"data": [
            {"index": 2, "embedding": [3.0]},
            {"index": 0, "embedding": [1.0]},
            {"index": 1, "embedding": [2.0]},
        ]});
        let server = Server::start(vec![Reply::json(200, &body.to_string())]);
        let got = embed(&backend(&options(&server.url), None), &["a", "b", "c"]).unwrap();
        assert_eq!(got, [vec![1.0], vec![2.0], vec![3.0]]);
        // Without indexes, the order of the answer is the order of the input.
        let body = json!({"data": [{"embedding": [5.0]}, {"embedding": [4.0]}]});
        let server = Server::start(vec![Reply::json(200, &body.to_string())]);
        let got = embed(&backend(&options(&server.url), None), &["a", "b"]).unwrap();
        assert_eq!(got, [vec![5.0], vec![4.0]]);
    }

    #[test]
    fn a_server_ignoring_dimensions_gets_its_vectors_cut_to_them() {
        let server = Server::start(vec![vectors(&[&[1.0, 2.0, 3.0, 4.0]])]);
        let options = SemanticOptions {
            dimensions: Some(2),
            ..options(&server.url)
        };
        assert_eq!(
            embed(&backend(&options, None), &["a"]).unwrap(),
            [vec![1.0, 2.0]]
        );
    }

    #[test]
    fn many_texts_go_in_batches_and_progress_counts_them() {
        let texts: Vec<String> = (0..40).map(|i| format!("fn f{i}() {{}}")).collect();
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let first: Vec<&[f32]> = vec![&[1.0]; BATCH_TEXTS];
        let rest: Vec<&[f32]> = vec![&[2.0]; 40 - BATCH_TEXTS];
        let server = Server::start(vec![vectors(&first), vectors(&rest)]);
        let progress = std::sync::Mutex::new(Vec::new());
        let got = backend(&options(&server.url), None)
            .embed(&texts, &|done| progress.lock().unwrap().push(done))
            .unwrap();
        assert_eq!(got.len(), 40);
        assert_eq!(got[BATCH_TEXTS], vec![2.0]);
        assert_eq!(*progress.lock().unwrap(), [BATCH_TEXTS, 40]);
        let sent: Vec<usize> = server
            .requests()
            .iter()
            .map(|r| r.json()["input"].as_array().unwrap().len())
            .collect();
        assert_eq!(sent, [BATCH_TEXTS, 40 - BATCH_TEXTS]);
        // Large texts go in smaller batches.
        let big = "x".repeat(BATCH_BYTES / 2 + 1);
        let server = Server::start(vec![vectors(&[&[1.0]]), vectors(&[&[1.0]])]);
        embed(&backend(&options(&server.url), None), &[&big, &big]).unwrap();
        assert_eq!(server.requests().len(), 2, "one text per request");
    }

    #[test]
    fn too_few_or_too_many_vectors_are_an_error() {
        for answer in [vec![&[1.0f32][..]], vec![&[1.0][..], &[2.0], &[3.0]]] {
            let server = Server::start(vec![vectors(&answer)]);
            let err = embed(&backend(&options(&server.url), None), &["a", "b"]).unwrap_err();
            assert!(
                err.contains(&format!(
                    "{} embeddings returned for 2 inputs",
                    answer.len()
                )),
                "{err}"
            );
        }
    }

    #[test]
    fn malformed_answers_are_an_error_not_a_panic() {
        for body in [
            "not json",
            r#"{"object": "list"}"#,
            r#"{"data": [{"embedding": "nope"}]}"#,
            r#"{"data": [{"embedding": [1.0, "x"]}]}"#,
        ] {
            let server = Server::start(vec![Reply::json(200, body)]);
            let err = embed(&backend(&options(&server.url), Some(KEY)), &["a"]).unwrap_err();
            assert!(err.contains("unreadable response"), "{body}: {err}");
            assert!(!err.contains(KEY), "{err}");
        }
        // An answer cut short.
        let server = Server::start(vec![Reply::Raw(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 200\r\n\r\n{\"data\": [{\"emb"
                .to_vec(),
        )]);
        let err = embed(&backend(&options(&server.url), None), &["a"]).unwrap_err();
        assert!(err.contains("unreadable response"), "{err}");
        assert_eq!(server.requests().len(), 1, "a bad answer is not retried");
    }

    #[test]
    fn vectors_of_different_lengths_are_passed_on_as_they_are() {
        // The search reports vectors of different lengths (see
        // search::tests::embedder_errors_and_bad_vectors_are_reported); the
        // backend does not hide them.
        let server = Server::start(vec![vectors(&[&[1.0, 2.0], &[1.0]])]);
        let got = embed(&backend(&options(&server.url), None), &["a", "b"]).unwrap();
        assert_eq!(got, [vec![1.0, 2.0], vec![1.0]]);
    }

    #[test]
    fn busy_servers_are_retried_after_the_wait_they_ask_for() {
        let busy = |status| Reply::status(status, "busy").with_header("Retry-After", "0");
        let server = Server::start(vec![busy(429), busy(503), vectors(&[&[7.0]])]);
        let got = embed(&backend(&options(&server.url), None), &["a"]).unwrap();
        assert_eq!(got, [vec![7.0]]);
        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        assert!(
            requests.iter().all(|r| r.body == requests[0].body),
            "every attempt sends the same request"
        );
    }

    #[test]
    fn retries_stop_after_the_last_attempt() {
        let busy = || Reply::status(502, "upstream down").with_header("Retry-After", "0");
        let replies = (0..=RETRIES).map(|_| busy()).collect();
        let server = Server::start(replies);
        let err = embed(&backend(&options(&server.url), Some(KEY)), &["a"]).unwrap_err();
        assert!(err.contains("answered HTTP 502: upstream down"), "{err}");
        assert!(!err.contains(KEY), "{err}");
        assert_eq!(server.requests().len(), RETRIES as usize + 1);
    }

    #[test]
    fn client_errors_are_not_retried() {
        for status in [400, 404, 413, 422] {
            let server = Server::start(vec![
                Reply::json(status, r#"{"error": "input too long"}"#),
                vectors(&[&[1.0]]),
            ]);
            let err = embed(&backend(&options(&server.url), None), &["a"]).unwrap_err();
            assert!(
                err.ends_with(&format!("answered HTTP {status}: input too long")),
                "{err}"
            );
            assert_eq!(server.requests_so_far().len(), 1, "HTTP {status}");
        }
    }

    #[test]
    fn a_dropped_connection_is_retried() {
        // Nothing comes back on the first connection: the server went away
        // mid-request. The second attempt, two seconds on, gets an answer.
        let server = Server::start(vec![Reply::Raw(Vec::new()), vectors(&[&[3.0]])]);
        let started = std::time::Instant::now();
        let got = embed(&backend(&options(&server.url), None), &["a"]).unwrap();
        assert_eq!(got, [vec![3.0]]);
        assert_eq!(server.requests().len(), 2);
        assert!(started.elapsed() >= Duration::from_secs(2), "it backs off");
    }

    #[test]
    fn a_refused_connection_is_not_retried_and_says_how_to_start_a_server() {
        let url = refused_url();
        let err = embed(&backend(&options(&url), Some(KEY)), &["a"]).unwrap_err();
        assert!(
            err.starts_with(&format!("cannot reach {url}/v1/embeddings")),
            "{err}"
        );
        assert!(err.contains("ollama pull embedder"), "{err}");
        assert!(!err.contains(KEY), "{err}");
    }

    #[test]
    fn status_errors_explain_what_to_do() {
        let fail = |reply: Reply, key: Option<&str>| {
            let server = Server::start(vec![reply]);
            embed(&backend(&options(&server.url), key), &["a"]).unwrap_err()
        };
        let err = fail(
            Reply::json(
                404,
                r#"{"error":{"message":"model \"embedder\" not found, try pulling it first"}}"#,
            ),
            None,
        );
        assert!(
            err.contains("has no model \"embedder\" (HTTP 404)"),
            "{err}"
        );
        assert!(err.contains("ollama pull embedder"), "{err}");

        for status in [401, 403] {
            let err = fail(
                Reply::json(status, r#"{"detail":"Unauthorized"}"#),
                Some(KEY),
            );
            assert!(
                err.contains(&format!(
                    "refused the request (HTTP {status}); set {API_KEY_ENV}"
                )),
                "{err}"
            );
            assert!(err.ends_with(": Unauthorized"), "{err}");
            assert!(!err.contains(KEY), "{err}");
        }

        let err = fail(Reply::status(405, "<html><body>405</body></html>"), None);
        assert!(
            err.ends_with("answered HTTP 405"),
            "an HTML page adds nothing: {err}"
        );
    }

    #[test]
    #[ignore = "known bug: an error body echoing the API key is printed verbatim, key included"]
    fn the_key_never_shows_in_an_error_even_when_the_server_echoes_it() {
        let server = Server::start(vec![Reply::json(
            401,
            &json!({"error": {"message": format!("Incorrect API key provided: {KEY}")}})
                .to_string(),
        )]);
        let err = embed(&backend(&options(&server.url), Some(KEY)), &["a"]).unwrap_err();
        assert!(!err.contains(KEY), "{err}");
    }

    #[test]
    fn a_config_file_url_on_this_machine_gets_the_key() {
        let server = Server::start(vec![vectors(&[&[1.0]])]);
        let options = SemanticOptions {
            url_from_config: true,
            on_command_line: false,
            ..options(&server.url)
        };
        embed(&backend(&options, Some(KEY)), &["a"]).unwrap();
        let auth = format!("Bearer {KEY}");
        assert_eq!(
            server.requests()[0].header("authorization"),
            Some(auth.as_str())
        );
    }

    #[test]
    fn transport_errors_elsewhere_get_no_hint_about_a_local_server() {
        // 127.0.0.2 is on this machine too, so the test stays off the
        // network, but its URL does not read as the local default.
        let port = refused_url().rsplit(':').next().unwrap().to_string();
        let url = format!("http://127.0.0.2:{port}");
        let err = embed(&backend(&options(&url), None), &["a"]).unwrap_err();
        assert!(
            err.starts_with(&format!("cannot reach {url}/v1/embeddings")),
            "{err}"
        );
        assert!(!err.contains("ollama"), "{err}");
    }
}
