//! `--mcp`: jscpd as a Model Context Protocol server over stdio (issue #891).
//!
//! `jscpd --mcp [PATH]...` scans the given paths and serves newline-delimited
//! JSON-RPC 2.0 on stdin and stdout, as the MCP stdio transport specifies:
//! stdout carries protocol messages only, and logs go to stderr. The scan
//! runs in the background, so a client's first requests are answered at
//! once and its first tool call waits for the scan.
//!
//! Two eras of the protocol share the stream. A client of the handshake
//! revisions (2024-11-05 to 2025-11-25) opens with `initialize`; a client of
//! the stateless revision (2026-07-28) names its version in the `_meta` of
//! every request, and may ask `server/discover` first. The server keeps no
//! state per client, so both get the same tools.
//!
//! The tools are in [`tools`]; what they search, the project's scan, in
//! [`project`].

mod project;
#[cfg(test)]
mod tests;
mod tools;

pub use project::Settings;

use project::Project;
use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};
use tools::Failure;

/// Revisions served after an `initialize` handshake, oldest first.
const HANDSHAKE_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
/// Revisions served statelessly: each request names one in its `_meta`.
const STATELESS_VERSIONS: &[&str] = &["2026-07-28"];
const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
const SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";
/// How long a client may keep the tool list and the discovery result: they
/// do not change while the process runs.
const CACHE_TTL_MS: u64 = 3_600_000;
/// The stack of the thread that reads requests: snippets are parsed on it,
/// the parsers recurse, and code can nest deeper than a main thread's stack
/// allows (1 MiB on Windows). The scan's pool threads have the same size.
const STACK_SIZE: usize = 64 * 1024 * 1024;

// JSON-RPC 2.0 error codes, and the one MCP adds.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// The protocol a request speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    /// A revision with the `initialize` handshake, or a request naming no
    /// revision at all.
    Handshake,
    /// The stateless revision: results carry `resultType` and the server's
    /// identity, and cacheable ones a time to live.
    Stateless,
}

impl Era {
    /// The era of a request from its `_meta`: a version there is checked,
    /// and a stateless request must say what the client can do.
    fn of(params: &Value) -> Result<Era, Failure> {
        let meta = params.get("_meta");
        let Some(version) = meta.and_then(|m| m.get(VERSION_KEY)) else {
            return Ok(Era::Handshake);
        };
        let Some(version) = version.as_str() else {
            return Err(Failure::new(
                INVALID_PARAMS,
                format!("_meta[\"{VERSION_KEY}\"] must be a string"),
            ));
        };
        if HANDSHAKE_VERSIONS.contains(&version) {
            return Ok(Era::Handshake);
        }
        if !STATELESS_VERSIONS.contains(&version) {
            return Err(Failure {
                code: UNSUPPORTED_PROTOCOL_VERSION,
                message: "Unsupported protocol version".to_string(),
                data: Some(json!({ "supported": supported_versions(), "requested": version })),
            });
        }
        if !meta
            .and_then(|m| m.get(CAPABILITIES_KEY))
            .is_some_and(Value::is_object)
        {
            return Err(Failure::new(
                INVALID_PARAMS,
                format!("missing _meta[\"{CAPABILITIES_KEY}\"], which every request must carry"),
            ));
        }
        Ok(Era::Stateless)
    }

    /// `result` as a result of this era.
    fn complete(self, result: Value) -> Value {
        match (self, result) {
            (Era::Stateless, Value::Object(mut fields)) => {
                fields.insert("resultType".into(), json!("complete"));
                let mut meta = Map::new();
                meta.insert(SERVER_INFO_KEY.into(), server_info());
                fields.insert("_meta".into(), Value::Object(meta));
                Value::Object(fields)
            }
            (_, result) => result,
        }
    }
}

/// Every revision the server speaks, the newest first.
fn supported_versions() -> Vec<&'static str> {
    STATELESS_VERSIONS
        .iter()
        .chain(HANDSHAKE_VERSIONS.iter().rev())
        .copied()
        .collect()
}

fn server_info() -> Value {
    json!({
        "name": "jscpd",
        "title": "jscpd Copy/Paste Detector",
        "version": env!("CARGO_PKG_VERSION"),
        "websiteUrl": "https://jscpd.dev",
    })
}

pub struct McpServer {
    project: Project,
}

impl McpServer {
    pub fn new(settings: Settings) -> Self {
        Self {
            project: Project::new(settings),
        }
    }

    /// Handle a JSON-RPC batch, which clients of 2025-03-26 may send: the
    /// responses to its requests, as an array, or `None` when it holds
    /// notifications only.
    pub fn handle_batch(&mut self, batch: &[Value]) -> Option<Value> {
        if batch.is_empty() {
            return Some(err(Value::Null, INVALID_REQUEST, "empty batch", None));
        }
        let responses: Vec<Value> = batch
            .iter()
            .filter_map(|msg| self.handle_message(msg))
            .collect();
        (!responses.is_empty()).then_some(Value::Array(responses))
    }

    /// Handle one JSON-RPC message; `None` means no response (a
    /// notification).
    pub fn handle_message(&mut self, msg: &Value) -> Option<Value> {
        if !msg.is_object() {
            return Some(err(Value::Null, INVALID_REQUEST, "invalid request", None));
        }
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str);
        let (Some(method), Some(id)) = (method, id) else {
            // A notification gets no answer, and the server sends no
            // requests, so a message without a method is not expected.
            return match (method, msg.get("id")) {
                (None, Some(id)) => Some(err(id.clone(), INVALID_REQUEST, "missing method", None)),
                _ => None,
            };
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        // `initialize` opens the handshake, whatever else it carries.
        let era = match method {
            "initialize" => Era::Handshake,
            _ => match Era::of(&params) {
                Ok(era) => era,
                Err(failure) => return Some(failure_response(id, failure)),
            },
        };
        let result = match method {
            "initialize" => Ok(self.initialize_result(&params)),
            "server/discover" => match era {
                Era::Stateless => Ok(self.discover_result()),
                Era::Handshake => Err(Failure::new(
                    INVALID_PARAMS,
                    format!(
                        "server/discover belongs to the stateless protocol: name its version in _meta[\"{VERSION_KEY}\"]"
                    ),
                )),
            },
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools_list(era)),
            "tools/call" => tools::call(&mut self.project, &params),
            _ => Err(Failure::new(METHOD_NOT_FOUND, "method not found")),
        };
        Some(match result {
            Ok(result) => ok(id, era.complete(result)),
            Err(failure) => failure_response(id, failure),
        })
    }

    fn initialize_result(&self, params: &Value) -> Value {
        let latest = HANDSHAKE_VERSIONS[HANDSHAKE_VERSIONS.len() - 1];
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(latest);
        let version = match HANDSHAKE_VERSIONS.contains(&requested) {
            true => requested,
            false => latest,
        };
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": server_info(),
            "instructions": tools::instructions(&self.project),
        })
    }

    fn discover_result(&self) -> Value {
        json!({
            "supportedVersions": supported_versions(),
            "capabilities": { "tools": {} },
            "instructions": tools::instructions(&self.project),
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })
    }

    fn tools_list(&self, era: Era) -> Value {
        let mut result = json!({ "tools": tools::definitions(&self.project) });
        if era == Era::Stateless {
            result["ttlMs"] = json!(CACHE_TTL_MS);
            result["cacheScope"] = json!("public");
        }
        result
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({ "code": code, "message": message });
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}

fn failure_response(id: Value, failure: Failure) -> Value {
    err(id, failure.code, &failure.message, failure.data)
}

/// Serve MCP over stdio until stdin closes. Returns the process exit code.
pub fn serve(settings: Settings) -> i32 {
    let server = std::thread::Builder::new()
        .name("jscpd-mcp".to_string())
        .stack_size(STACK_SIZE)
        .spawn(move || run(settings));
    match server.map(|handle| handle.join()) {
        Ok(Ok(code)) => code,
        Ok(Err(_)) => 1,
        Err(e) => {
            eprintln!("Error: --mcp: {e}");
            1
        }
    }
}

fn run(settings: Settings) -> i32 {
    let mut server = McpServer::new(settings);
    eprintln!("jscpd MCP server (stdio): scanning in the background, waiting for the client");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("jscpd MCP server: stdin error: {e}");
                return 1;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => server.handle_batch(&batch),
            Ok(msg) => server.handle_message(&msg),
            Err(e) => Some(err(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {e}"),
                None,
            )),
        };
        if let Some(response) = response {
            let mut out = stdout.lock();
            // to_string is single-line JSON: safe for the line-delimited transport.
            if writeln!(out, "{response}")
                .and_then(|_| out.flush())
                .is_err()
            {
                return 0; // client hung up
            }
        }
    }
    0
}
