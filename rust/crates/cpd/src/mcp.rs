// mcp.rs — Model Context Protocol server over stdio transport (issue #891).
//
// `cpd --mcp [PATH]...` scans the given paths once, keeps the detection-ready
// token hashes in memory, and then serves newline-delimited JSON-RPC 2.0 on
// stdin/stdout as the MCP stdio transport specifies. stdout carries protocol
// messages only; all logging goes to stderr.
//
// Tools mirror the HTTP jscpd-server:
//   - check_duplication        (code, format) — check a snippet against the scan
//   - get_statistics           ()             — project duplication statistics
//   - check_current_directory  ()             — re-scan the configured paths

use cpd_core::detect::{PreparedSource, detect_prepared};
use cpd_core::models::{CpdClone, Statistics};
use cpd_finder::orchestrate::{
    PreparedScan, RunConfig, build_thread_pool, prepare_scan_in, strip_types_formats,
};
use cpd_finder::statistics;
use cpd_tokenizer::tokenizer::{TokenizeOptions, tokenize_to_detection};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

const SNIPPET_ID: &str = "snippet://check";
/// Default cap on clones returned by check_current_directory — keeps tool
/// results from flooding an LLM context on heavily duplicated projects.
const DEFAULT_CLONE_LIMIT: usize = 100;
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

// JSON-RPC 2.0 error codes.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub struct McpServer {
    config: RunConfig,
    pool: rayon::ThreadPool,
    /// Detection-ready sources grouped by pool key (format or cross-format
    /// group), each pool sorted for deterministic detection.
    pools: HashMap<String, Vec<PreparedSource>>,
    stats: Statistics,
    /// Project clones from the last scan, served by check_current_directory.
    clones: Vec<CpdClone>,
    file_count: usize,
    /// Canonicalized scan roots, for skip_local and for relativizing paths.
    scan_roots: Vec<PathBuf>,
}

impl McpServer {
    pub fn new(config: RunConfig) -> Self {
        let pool = build_thread_pool(config.workers);
        let scan_roots = config
            .paths
            .iter()
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
            .collect();
        let mut server = Self {
            config,
            pool,
            pools: HashMap::new(),
            stats: Statistics {
                total: Default::default(),
                formats: HashMap::new(),
                detection_date: String::new(),
            },
            clones: Vec::new(),
            file_count: 0,
            scan_roots,
        };
        server.rescan();
        server
    }

    /// Pool key for a format: cross-format groups share one pool, mirroring
    /// `orchestrate::build_pools`.
    fn pool_key(&self, format: &str) -> String {
        for (idx, group) in self.config.cross_formats.iter().enumerate() {
            if group.iter().any(|f| f == format) {
                return format!("cross:{idx:04}");
            }
        }
        format!("format:{format}")
    }

    /// Walk + tokenize the configured paths, refresh the cached pools and
    /// project statistics.
    fn rescan(&mut self) {
        let PreparedScan { sources, prepared } = prepare_scan_in(&self.pool, &self.config);
        self.file_count = sources.len();

        let mut pools: HashMap<String, Vec<PreparedSource>> = HashMap::new();
        for ps in prepared {
            pools.entry(self.pool_key(&ps.format)).or_default().push(ps);
        }
        for pool in pools.values_mut() {
            pool.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));
        }

        // Project-wide clones: detection consumes its input, so hand it a copy
        // and keep the pools cached for snippet checks.
        let mut format_groups: Vec<(&String, Vec<PreparedSource>)> =
            pools.iter().map(|(k, v)| (k, v.clone())).collect();
        format_groups.sort_by(|a, b| a.0.cmp(b.0));
        let groups: Vec<Vec<PreparedSource>> = format_groups.into_iter().map(|(_, v)| v).collect();
        let clones = self.pool.install(|| {
            detect_prepared(
                groups,
                self.config.min_tokens,
                self.config.skip_local,
                self.config.min_lines,
                &self.scan_roots,
            )
        });
        self.stats = statistics::compute(&sources, &clones);
        self.clones = clones;
        self.pools = pools;
    }

    /// Compact JSON for one project clone, paths relativized to the scan roots.
    /// Sub-format fragments (`<path>:<format>` ids from embedded code blocks)
    /// are folded into their parent file.
    fn clone_to_json(&self, clone: &CpdClone) -> Value {
        let display = |source_id: &str| {
            let path = source_id
                .strip_suffix(&format!(":{}", clone.format))
                .unwrap_or(source_id);
            self.display_path(path)
        };
        let a = &clone.fragment_a;
        let b = &clone.fragment_b;
        json!({
            "format": clone.format,
            "fileA": display(&a.source_id),
            "startA": a.start.line,
            "endA": a.end.line,
            "fileB": display(&b.source_id),
            "startB": b.start.line,
            "endB": b.end.line,
            "lines": a.end.line.saturating_sub(a.start.line),
            "tokens": clone.token_count,
        })
    }

    /// Relativize a canonical source id to the first matching scan root.
    fn display_path(&self, id: &str) -> String {
        let path = std::path::Path::new(id);
        for root in &self.scan_roots {
            if let Ok(stripped) = path.strip_prefix(root) {
                return stripped.to_string_lossy().into_owned();
            }
        }
        id.to_string()
    }

    /// Handle one JSON-RPC message; None means no response (notification).
    pub fn handle_message(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str);

        let (Some(method), Some(id)) = (method, id) else {
            // Notification (no id): nothing to answer. Message without a
            // method (a response) is not expected — we never issue requests.
            return match (method, msg.get("id")) {
                (None, Some(id)) => Some(err(id.clone(), INVALID_REQUEST, "missing method")),
                _ => None,
            };
        };

        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => Some(ok(id, self.initialize_result(&params))),
            "ping" => Some(ok(id, json!({}))),
            "tools/list" => Some(ok(id, json!({ "tools": tool_definitions() }))),
            "tools/call" => Some(self.tools_call(id, &params)),
            _ => Some(err(id, METHOD_NOT_FOUND, "method not found")),
        }
    }

    fn initialize_result(&self, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(LATEST_PROTOCOL_VERSION);
        let version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            LATEST_PROTOCOL_VERSION
        };
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "jscpd",
                "title": "jscpd Copy/Paste Detector",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "Check code snippets for duplication against the scanned project (check_duplication), read project duplication statistics (get_statistics), or re-scan after edits (check_current_directory).",
        })
    }

    fn tools_call(&mut self, id: Value, params: &Value) -> Value {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return err(id, INVALID_PARAMS, "missing tool name");
        };
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match name {
            "check_duplication" => {
                let (Some(code), Some(format)) = (
                    args.get("code").and_then(Value::as_str),
                    args.get("format").and_then(Value::as_str),
                ) else {
                    return err(
                        id,
                        INVALID_PARAMS,
                        "check_duplication requires string arguments 'code' and 'format'",
                    );
                };
                match self.check_duplication(code, format) {
                    Ok(payload) => ok(id, tool_text(&payload, false)),
                    Err(message) => ok(id, tool_text(&json!({ "error": message }), true)),
                }
            }
            "get_statistics" => ok(
                id,
                tool_text(
                    &json!({
                        "files": self.file_count,
                        "clones": self.clones.len(),
                        "statistics": self.stats,
                    }),
                    false,
                ),
            ),
            "check_current_directory" => {
                let limit = match args.get("limit") {
                    None | Some(Value::Null) => DEFAULT_CLONE_LIMIT,
                    Some(v) => match v.as_u64() {
                        Some(n) => n as usize,
                        None => {
                            return err(
                                id,
                                INVALID_PARAMS,
                                "'limit' must be a non-negative integer",
                            );
                        }
                    },
                };
                self.rescan();
                let duplications: Vec<Value> = self
                    .clones
                    .iter()
                    .take(limit)
                    .map(|c| self.clone_to_json(c))
                    .collect();
                let mut payload = json!({
                    "files": self.file_count,
                    "clones": self.clones.len(),
                    "returned": duplications.len(),
                    "duplicatedLines": self.stats.total.duplicated_lines,
                    "percentage": self.stats.total.percentage,
                    "duplications": duplications,
                });
                if self.clones.len() > limit {
                    payload["note"] = json!(format!(
                        "clone list truncated to {limit}; pass a higher 'limit' for more"
                    ));
                }
                ok(id, tool_text(&payload, false))
            }
            other => err(id, INVALID_PARAMS, &format!("unknown tool '{other}'")),
        }
    }

    fn check_duplication(&self, code: &str, format: &str) -> Result<Value, String> {
        if !cpd_tokenizer::formats::list_formats().contains(&format)
            && !self.config.formats_exts.contains_key(format)
        {
            return Err(format!(
                "unknown format '{format}': run `cpd --list` for supported formats"
            ));
        }

        let opts = TokenizeOptions {
            mode: self.config.mode,
            ignore_case: self.config.ignore_case,
            ignore_ranges: Vec::new(),
            code_ignore_regexes: Vec::new(),
            strip_types_formats: strip_types_formats(&self.config.cross_formats),
        };
        let det_tokens = tokenize_to_detection(format, code, &opts);
        if det_tokens.len() < self.config.min_tokens {
            return Ok(json!({
                "count": 0,
                "duplications": [],
                "note": format!(
                    "snippet has {} tokens, below the detection threshold of {} (--min-tokens)",
                    det_tokens.len(), self.config.min_tokens
                ),
            }));
        }

        let snippet =
            PreparedSource::from_detection_tokens(SNIPPET_ID.into(), format.into(), &det_tokens);
        let mut pool = self
            .pools
            .get(&self.pool_key(format))
            .cloned()
            .unwrap_or_default();
        pool.push(snippet);
        pool.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));

        let clones = self.pool.install(|| {
            detect_prepared(
                vec![pool],
                self.config.min_tokens,
                false,
                self.config.min_lines,
                &[],
            )
        });

        let duplications: Vec<Value> = clones
            .iter()
            .filter(|c| {
                c.fragment_a.source_id == SNIPPET_ID || c.fragment_b.source_id == SNIPPET_ID
            })
            .map(|c| {
                // Present the snippet side and the project side explicitly.
                let (snip, file) = if c.fragment_a.source_id == SNIPPET_ID {
                    (&c.fragment_a, &c.fragment_b)
                } else {
                    (&c.fragment_b, &c.fragment_a)
                };
                let file_name = if file.source_id == SNIPPET_ID {
                    "(snippet)".to_string()
                } else {
                    self.display_path(&file.source_id)
                };
                json!({
                    "file": file_name,
                    "fileStartLine": file.start.line,
                    "fileEndLine": file.end.line,
                    "snippetStartLine": snip.start.line,
                    "snippetEndLine": snip.end.line,
                    "tokens": c.token_count,
                })
            })
            .collect();

        Ok(json!({ "count": duplications.len(), "duplications": duplications }))
    }
}

/// MCP tool result: one text content item carrying compact JSON.
fn tool_text(payload: &Value, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": payload.to_string() }],
        "isError": is_error,
    })
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "check_duplication",
            "description": "Check a code snippet for duplications against the scanned project. Returns matching project locations with line ranges.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "Source code snippet to check" },
                    "format": { "type": "string", "description": "Language format, e.g. javascript, typescript, python (see `cpd --list`)" }
                },
                "required": ["code", "format"]
            }
        },
        {
            "name": "get_statistics",
            "description": "Get project duplication statistics from the last scan (totals and per-format).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "check_current_directory",
            "description": "Re-scan the configured paths and return updated duplication counts plus the list of clones (file pairs with line ranges).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Maximum number of clones to include in the response (default 100); 'clones' always carries the untruncated total"
                    }
                }
            }
        }
    ])
}

/// Serve MCP over stdio until stdin closes. Returns the process exit code.
pub fn serve(config: RunConfig) -> i32 {
    let started = std::time::Instant::now();
    let mut server = McpServer::new(config);
    eprintln!(
        "jscpd MCP server (stdio): scanned {} files, {} clones in {:.0}ms — waiting for client",
        server.file_count,
        server.clones.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );

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
            Ok(msg) => server.handle_message(&msg),
            Err(e) => Some(err(Value::Null, PARSE_ERROR, &format!("parse error: {e}"))),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a server over a temp dir with two duplicate JS files.
    fn test_server() -> McpServer {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cpd-mcp-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let body = "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}\nfunction sub(a, b) {\n  const d = a - b;\n  console.log('diff', d);\n  return d;\n}\n";
        std::fs::write(dir.join("one.js"), body).unwrap();
        std::fs::write(dir.join("two.js"), body).unwrap();
        McpServer::new(RunConfig {
            paths: vec![dir],
            min_tokens: 15,
            min_lines: 1,
            ..Default::default()
        })
    }

    fn call(server: &mut McpServer, msg: Value) -> Option<Value> {
        server.handle_message(&msg)
    }

    fn tool_call(server: &mut McpServer, name: &str, args: Value) -> Value {
        let resp = call(
            server,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": { "name": name, "arguments": args } }),
        )
        .unwrap();
        resp["result"].clone()
    }

    fn tool_payload(result: &Value) -> Value {
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn initialize_negotiates_protocol_version() {
        let mut s = test_server();
        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
                    "params": { "protocolVersion": "2025-03-26", "capabilities": {} } }),
        )
        .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(resp["result"]["serverInfo"]["name"], "jscpd");

        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "1999-01-01" } }),
        )
        .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn notifications_get_no_response() {
        let mut s = test_server();
        assert!(
            call(
                &mut s,
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
            )
            .is_none()
        );
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let mut s = test_server();
        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 7, "method": "resources/list" }),
        )
        .unwrap();
        assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(resp["id"], 7);
    }

    #[test]
    fn tools_list_exposes_three_tools() {
        let mut s = test_server();
        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        )
        .unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "check_duplication",
                "get_statistics",
                "check_current_directory"
            ]
        );
        for tool in tools {
            assert!(tool["inputSchema"]["type"] == "object", "schema required");
        }
    }

    #[test]
    fn check_duplication_finds_project_match() {
        let mut s = test_server();
        let result = tool_call(
            &mut s,
            "check_duplication",
            json!({
                "code": "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}",
                "format": "javascript"
            }),
        );
        assert_eq!(result["isError"], false);
        let payload = tool_payload(&result);
        assert!(
            payload["count"].as_u64().unwrap() >= 1,
            "snippet copied from the project must match, got: {payload}"
        );
        let dup = &payload["duplications"][0];
        assert!(dup["file"].as_str().unwrap().ends_with(".js"));
        assert!(dup["tokens"].as_u64().unwrap() >= 15);
    }

    #[test]
    fn check_duplication_clean_snippet_finds_nothing() {
        let mut s = test_server();
        let result = tool_call(
            &mut s,
            "check_duplication",
            json!({
                "code": "const totallyUnique = [9, 8, 7].map((n) => n * 31 + 5).filter((n) => n % 2 === 0);",
                "format": "javascript"
            }),
        );
        let payload = tool_payload(&result);
        assert_eq!(payload["count"], 0);
    }

    #[test]
    fn check_duplication_short_snippet_notes_threshold() {
        let mut s = test_server();
        let result = tool_call(
            &mut s,
            "check_duplication",
            json!({ "code": "let x = 1;", "format": "javascript" }),
        );
        let payload = tool_payload(&result);
        assert_eq!(payload["count"], 0);
        assert!(payload["note"].as_str().unwrap().contains("min-tokens"));
    }

    #[test]
    fn check_duplication_unknown_format_is_tool_error() {
        let mut s = test_server();
        let result = tool_call(
            &mut s,
            "check_duplication",
            json!({ "code": "whatever", "format": "not-a-language" }),
        );
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("unknown format")
        );
    }

    #[test]
    fn get_statistics_reports_scan() {
        let mut s = test_server();
        let payload = tool_payload(&tool_call(&mut s, "get_statistics", json!({})));
        assert_eq!(payload["files"], 2);
        assert!(
            payload["clones"].as_u64().unwrap() >= 1,
            "two identical files"
        );
        assert!(payload["statistics"]["total"]["tokens"].as_u64().unwrap() > 0);
    }

    #[test]
    fn check_current_directory_rescans_and_lists_clones() {
        let mut s = test_server();
        let payload = tool_payload(&tool_call(&mut s, "check_current_directory", json!({})));
        assert_eq!(payload["files"], 2);
        let total = payload["clones"].as_u64().unwrap();
        assert!(total >= 1);

        let duplications = payload["duplications"].as_array().unwrap();
        assert_eq!(
            duplications.len() as u64,
            payload["returned"].as_u64().unwrap()
        );
        assert_eq!(
            duplications.len() as u64,
            total,
            "no truncation below the default limit"
        );
        let dup = &duplications[0];
        assert_eq!(dup["format"], "javascript");
        assert!(dup["fileA"].as_str().unwrap().ends_with(".js"));
        assert!(dup["fileB"].as_str().unwrap().ends_with(".js"));
        assert_ne!(
            dup["fileA"], dup["fileB"],
            "clone spans the two duplicate files"
        );
        assert!(dup["startA"].as_u64().is_some() && dup["endA"].as_u64().is_some());
        assert!(dup["tokens"].as_u64().unwrap() >= 15);
        assert!(
            payload.get("note").is_none(),
            "no truncation note when complete"
        );
    }

    #[test]
    fn check_current_directory_limit_truncates_with_note() {
        let mut s = test_server();
        let payload = tool_payload(&tool_call(
            &mut s,
            "check_current_directory",
            json!({ "limit": 0 }),
        ));
        assert!(
            payload["clones"].as_u64().unwrap() >= 1,
            "total stays untruncated"
        );
        assert_eq!(payload["returned"], 0);
        assert_eq!(payload["duplications"].as_array().unwrap().len(), 0);
        assert!(
            payload["note"].as_str().unwrap().contains("truncated"),
            "truncation must be visible"
        );

        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                    "params": { "name": "check_current_directory",
                                "arguments": { "limit": "ten" } } }),
        )
        .unwrap();
        assert_eq!(
            resp["error"]["code"], INVALID_PARAMS,
            "non-integer limit rejected"
        );
    }

    #[test]
    fn missing_tool_arguments_is_invalid_params() {
        let mut s = test_server();
        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": { "name": "check_duplication", "arguments": { "code": "x" } } }),
        )
        .unwrap();
        assert_eq!(resp["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn unknown_tool_is_invalid_params() {
        let mut s = test_server();
        let resp = call(
            &mut s,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                    "params": { "name": "bogus", "arguments": {} } }),
        )
        .unwrap();
        assert_eq!(resp["error"]["code"], INVALID_PARAMS);
    }
}
