// mcp_stdio.rs — end-to-end test of `cpd --mcp`: spawn the real binary and
// speak newline-delimited JSON-RPC over its stdin/stdout (issue #891).

mod common;

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

/// Spawn `cpd --mcp <extra> <dir>` with every stream piped, its caches
/// beside `dir`.
fn spawn_mcp(dir: &Path, extra: &[&str]) -> std::process::Child {
    Command::new(cpd_bin())
        .arg("--mcp")
        .args(extra)
        .arg(dir)
        .env("JSCPD_CACHE_DIR", dir.with_extension("cache"))
        .env_remove("JSCPD_SEMANTIC_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cpd --mcp")
}

/// Write one request per line, close stdin so the server exits, and parse
/// every stdout line as JSON. Returns the responses and the raw output.
fn drive(
    mut child: std::process::Child,
    requests: &[String],
) -> (Vec<Value>, std::process::Output) {
    {
        let stdin = child.stdin.as_mut().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    } // drop stdin → EOF → clean exit
    let output = child.wait_with_output().expect("cpd --mcp must exit");
    assert!(output.status.success(), "exit 0 on stdin EOF");
    let responses = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("every stdout line must be valid JSON"))
        .collect();
    (responses, output)
}

/// A `tools/call` request.
fn tool_call(id: u32, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string()
}

/// The JSON a tool answered with.
fn payload(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], false, "{response}");
    response["result"]["structuredContent"].clone()
}

/// A fresh folder with two copies of one function. The tests of this file
/// run in parallel in one process, so each call gets a folder of its own.
fn fixture_dir() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cpd-mcp-stdio-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let body = "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}\n";
    std::fs::write(dir.join("one.js"), body).unwrap();
    std::fs::write(dir.join("two.js"), body).unwrap();
    dir
}

/// Drive a full session: initialize → initialized → tools/list → tools/call,
/// then close stdin and collect one JSON response per request line.
#[test]
fn mcp_stdio_session_end_to_end() {
    let dir = fixture_dir();
    let child = spawn_mcp(&dir, &["--min-tokens", "15", "--min-lines", "1"]);

    let requests = [
        INITIALIZE.to_string(),
        INITIALIZED.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
        tool_call(
            2,
            "check_duplication",
            json!({
                "code": "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}",
                "format": "javascript",
            }),
        ),
        tool_call(3, "get_statistics", json!({})),
        tool_call(4, "check_current_directory", json!({})),
        r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#.to_string(),
    ];
    let (responses, output) = drive(child, &requests);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 7 messages sent, 1 is a notification → 6 responses.
    assert_eq!(responses.len(), 6, "stdout: {stdout}");

    assert_eq!(responses[0]["id"], 0);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "jscpd");

    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);

    let check = payload(&responses[2]);
    assert!(
        check["count"].as_u64().unwrap() >= 1,
        "snippet must match the scanned project, got: {check}"
    );
    assert_eq!(check["duplications"][0]["kind"], "exact");
    // The text content is the structured one, for clients of older
    // revisions.
    let text = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(
        text,
        responses[2]["result"]["structuredContent"].to_string()
    );

    let stats = payload(&responses[3]);
    assert_eq!(stats["files"], 2);

    let rescan = payload(&responses[4]);
    let dups = rescan["duplications"].as_array().unwrap();
    assert!(
        !dups.is_empty(),
        "check_current_directory must return the clone list, got: {rescan}"
    );
    assert!(dups[0]["fileA"].as_str().unwrap().ends_with(".js"));
    assert_eq!(dups[0]["kind"], "exact");

    assert_eq!(responses[5]["id"], 5, "ping answered");

    // Transport hygiene: stderr may log, stdout must be protocol-only (checked
    // above by parsing every line as JSON).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scanned 2 files"), "{stderr}");
}

/// A client of the stateless revision names it in every request's `_meta`
/// and gets results that say they are complete.
#[test]
fn mcp_stdio_stateless_session() {
    let dir = fixture_dir();
    let child = spawn_mcp(&dir, &["--min-tokens", "15", "--min-lines", "1"]);
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    let requests = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": { "_meta": meta } })
            .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "get_statistics", "arguments": {}, "_meta": meta },
        })
        .to_string(),
    ];
    let (responses, _) = drive(child, &requests);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"]["supportedVersions"][0], "2026-07-28");
    assert_eq!(responses[0]["result"]["resultType"], "complete");
    assert_eq!(responses[1]["result"]["resultType"], "complete");
    assert_eq!(payload(&responses[1])["files"], 2);
}

/// `check_duplication` with a `similarity` argument returns structurally
/// similar project functions (issue #999, stage 2) and rejects ratios
/// outside (0, 1].
#[test]
fn mcp_check_duplication_similarity() {
    let dir = std::env::temp_dir().join(format!("cpd-mcp-similarity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("invoice.js"), common::INVOICE_JS).unwrap();
    let child = spawn_mcp(&dir, &[]);
    let check = |id: u32, extra: Value| {
        let mut args = json!({ "code": common::CREDIT_NOTE_JS, "format": "javascript" });
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        tool_call(id, "check_duplication", args)
    };
    let requests = [
        INITIALIZE.to_string(),
        INITIALIZED.to_string(),
        check(1, json!({ "similarity": 0.7 })),
        check(2, json!({ "similarity": 0.95 })),
        check(3, json!({ "similarity": 2 })),
        check(4, json!({ "similarity": 1 })),
        check(5, json!({ "kinds": ["exact"] })),
    ];
    let (responses, output) = drive(child, &requests);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(responses.len(), 6, "stdout: {stdout}");
    // Requests without `kinds` get the ast matches under `similar`, as
    // earlier versions answered.
    let loose = payload(&responses[1]);
    assert_eq!(loose["count"], 0, "no exact match: {loose}");
    assert_eq!(loose["similarCount"], 1, "{loose}");
    let hit = &loose["similar"][0];
    assert!(hit["file"].as_str().unwrap().ends_with("invoice.js"));
    assert_eq!(hit["name"], "buildInvoice");
    assert_eq!(hit["snippetName"], "buildCreditNote");
    let sim = hit["similarity"].as_f64().unwrap();
    assert!(sim > 0.7 && sim < 0.9, "got {sim}");
    let strict = payload(&responses[2]);
    assert_eq!(strict["similarCount"], 0, "too strict: {strict}");
    let message = responses[3]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(responses[3]["result"]["isError"], true);
    assert!(message.contains("similarity"), "{message}");
    let exact_only = payload(&responses[4]);
    assert!(
        exact_only.get("similarCount").is_none(),
        "1 means exact matches only, no similarity section: {exact_only}"
    );
    let asked = payload(&responses[5]);
    assert_eq!(asked["count"], 0, "no exact copy of the rewrite: {asked}");
    assert!(asked.get("similar").is_none(), "{asked}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fresh folder with the stand-in server's cart project.
fn cart_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(dir.with_extension("cache"));
    common::embeddings::cart_project(&dir);
    dir
}

/// With `--semantic`, the tools look for semantic clones too: the Rust cart
/// total and its Svelte port pair up, a snippet finds the function that does
/// its job, and compare_folders pairs the two sides as `--compare` does.
#[test]
fn mcp_semantic_clones_and_compare_folders() {
    let server = common::embeddings::Server::start();
    let dir = cart_dir("semantic");
    let child = spawn_mcp(
        &dir,
        &[
            "--semantic",
            "--semantic-url",
            &server.url,
            "--semantic-model",
            "stand-in",
            "--min-tokens",
            "15",
            "--min-lines",
            "3",
        ],
    );
    let snippet = "export function totalOfCart(lines: Line[], couponPercent: number): number {\n  let subtotal = 0;\n  for (const line of lines) {\n    subtotal += line.unitPrice * line.quantity;\n  }\n  const discount = Math.floor((subtotal * couponPercent) / 100);\n  const shipping = subtotal - discount > 5000 ? 0 : 499;\n  return subtotal - discount + shipping;\n}\n";
    let requests = [
        INITIALIZE.to_string(),
        tool_call(
            1,
            "check_current_directory",
            json!({ "kinds": ["semantic"] }),
        ),
        tool_call(
            2,
            "check_duplication",
            json!({ "code": snippet, "format": "ts", "kinds": ["type4"] }),
        ),
        tool_call(
            3,
            "compare_folders",
            json!({ "left": "backend", "right": "frontend" }),
        ),
        tool_call(
            4,
            "compare_folders",
            json!({ "left": "backend", "right": "frontend", "limit": 0 }),
        ),
    ];
    let (responses, output) = drive(child, &requests);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(responses.len(), 5, "{stderr}");

    let scan = payload(&responses[1]);
    assert_eq!(scan["kinds"], json!(["semantic"]));
    let pairs: Vec<(String, String)> = scan["duplications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            assert_eq!(d["kind"], "semantic");
            (
                d["fileA"].as_str().unwrap().to_string(),
                d["fileB"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [(
            "backend/src/cart.rs".to_string(),
            "frontend/src/Cart.svelte".to_string()
        )],
        "{scan}"
    );

    let checked = payload(&responses[2]);
    let names: Vec<&str> = checked["duplications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            assert_eq!(d["kind"], "semantic", "{d}");
            assert_eq!(d["snippetName"], "totalOfCart", "{d}");
            d["name"].as_str().unwrap()
        })
        .collect();
    assert!(
        names.contains(&"cart_total") || names.contains(&"cartTotal"),
        "{checked}"
    );

    let compared = payload(&responses[3]);
    assert_eq!(compared["model"], "stand-in");
    let pairs: Vec<(&str, &str)> = compared["code"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["a"]["name"].as_str().unwrap(),
                p["b"]["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(pairs, [("cart_total", "cartTotal")], "{compared}");
    assert_eq!(compared["code"]["sides"][0]["functions"], 13);
    assert!(compared.get("note").is_none());

    let cut = payload(&responses[4]);
    assert_eq!(cut["code"]["pairs"], json!([]));
    assert_eq!(cut["code"]["sides"][0]["functions"], 13, "totals stay");
    assert!(
        cut["note"].as_str().unwrap().contains("code.pairs (1)"),
        "{cut}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The kinds a request asks for are the detectors that run: a pair of
/// functions with one structure that do one job is a semantic clone when
/// only semantic clones are asked for, and an ast one, not both, when the
/// two kinds are, as `jscpd --similarity --semantic` reports it.
#[test]
fn mcp_semantic_clones_do_not_wait_for_an_ast_pass_nobody_asked_for() {
    let server = common::embeddings::Server::start();
    let dir = std::env::temp_dir().join(format!("cpd-mcp-kinds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(dir.with_extension("cache"));
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    std::fs::write(dir.join("a/report.js"), "export function summarizeOrders(orders) {\n  const shipped = orders.filter((order) => order.shipped);\n  const revenue = shipped.reduce((sum, order) => sum + order.total, 0);\n  const largest = shipped.reduce((max, order) => Math.max(max, order.total), 0);\n  return { count: shipped.length, revenue, largest };\n}\n").unwrap();
    std::fs::write(dir.join("b/summary.js"), "export function summarizeOrderList(orders) {\n  const sent = orders.filter((order) => order.shipped);\n  const income = sent.reduce((acc, order) => acc + order.total, 0);\n  const biggest = sent.reduce((top, order) => Math.max(top, order.total), 0);\n  return { count: sent.length, income, biggest };\n}\n").unwrap();
    let child = spawn_mcp(
        &dir,
        &[
            "--semantic-url",
            &server.url,
            "--semantic-model",
            "stand-in",
            "--semantic-same-threshold",
            "0.5",
            "--min-tokens",
            "30",
            "--min-lines",
            "3",
        ],
    );
    let requests = [
        INITIALIZE.to_string(),
        tool_call(1, "get_statistics", json!({ "kinds": ["semantic"] })),
        tool_call(2, "get_statistics", json!({ "kinds": ["ast", "semantic"] })),
    ];
    let (responses, output) = drive(child, &requests);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        payload(&responses[1])["byKind"],
        json!({ "semantic": 1 }),
        "{stderr}"
    );
    assert_eq!(
        payload(&responses[2])["byKind"],
        json!({ "ast": 1 }),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Without the model, semantic clones and compare_folders say what is
/// missing, and every other kind is still found.
#[test]
fn mcp_semantic_without_the_model_says_so() {
    let dir = cart_dir("no-model");
    let child = spawn_mcp(
        &dir,
        &["--semantic", "--min-tokens", "15", "--min-lines", "3"],
    );
    let requests = [
        INITIALIZE.to_string(),
        tool_call(1, "get_statistics", json!({})),
        tool_call(
            2,
            "compare_folders",
            json!({ "left": "backend", "right": "frontend" }),
        ),
    ];
    let (responses, _) = drive(child, &requests);
    let stats = payload(&responses[1]);
    assert_eq!(stats["kinds"], json!(["exact", "semantic"]));
    let why = stats["unavailable"]["semantic"].as_str().unwrap();
    assert!(why.contains("not downloaded yet"), "{why}");
    assert!(why.contains("Ask the user"), "{why}");
    assert_eq!(responses[2]["result"]["isError"], true);
    let message = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(message.contains("not downloaded yet"), "{message}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_parse_error_is_reported_not_fatal() {
    let dir = fixture_dir();
    let mut child = Command::new(cpd_bin())
        .args(["--mcp"])
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cpd --mcp");
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "this is not json").unwrap();
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":9,"method":"ping"}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses: Vec<Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        responses[0]["error"]["code"], -32700,
        "parse error reported"
    );
    assert_eq!(
        responses[1]["id"], 9,
        "server keeps serving after bad input"
    );
}
