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
}
