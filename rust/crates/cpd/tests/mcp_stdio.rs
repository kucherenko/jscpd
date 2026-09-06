// mcp_stdio.rs — end-to-end test of `cpd --mcp`: spawn the real binary and
// speak newline-delimited JSON-RPC over its stdin/stdout (issue #891).

use std::io::Write;
use std::process::{Command, Stdio};

fn cpd_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

fn fixture_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-mcp-stdio-{}", std::process::id()));
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
    let mut child = Command::new(cpd_bin())
        .args(["--mcp", "--min-tokens", "15", "--min-lines", "1"])
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cpd --mcp");

    let requests = [
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"check_duplication","arguments":{{"code":{},"format":"javascript"}}}}}}"#,
            serde_json::to_string(
                "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}"
            )
            .unwrap()
        ),
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_statistics","arguments":{}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"check_current_directory","arguments":{}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#.to_string(),
    ];
    {
        let stdin = child.stdin.as_mut().unwrap();
        for req in &requests {
            writeln!(stdin, "{req}").unwrap();
        }
    } // drop stdin → EOF → clean exit

    let output = child.wait_with_output().expect("cpd --mcp must exit");
    assert!(output.status.success(), "exit 0 on stdin EOF");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("every stdout line must be valid JSON"))
        .collect();
    // 7 messages sent, 1 is a notification → 6 responses.
    assert_eq!(responses.len(), 6, "stdout: {stdout}");

    assert_eq!(responses[0]["id"], 0);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "jscpd");

    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);

    let check: serde_json::Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(
        check["count"].as_u64().unwrap() >= 1,
        "snippet must match the scanned project, got: {check}"
    );

    let stats: serde_json::Value = serde_json::from_str(
        responses[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["files"], 2);

    let rescan: serde_json::Value = serde_json::from_str(
        responses[4]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let dups = rescan["duplications"].as_array().unwrap();
    assert!(
        !dups.is_empty(),
        "check_current_directory must return the clone list, got: {rescan}"
    );
    assert!(dups[0]["fileA"].as_str().unwrap().ends_with(".js"));
    assert_eq!(dups[0]["kind"], "exact", "clone payloads carry their kind");
    assert!(
        dups[0].get("similarity").is_none(),
        "no score on exact clones"
    );

    assert_eq!(responses[5]["id"], 5, "ping answered");

    // Transport hygiene: stderr may log, stdout must be protocol-only (checked
    // above by parsing every line as JSON).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scanned"), "startup log goes to stderr");
}

/// `check_duplication` with a `similarity` argument returns structurally
/// similar project functions (issue #999, stage 2) and rejects ratios
/// outside (0, 1].
#[test]
fn mcp_check_duplication_similarity() {
    let dir = std::env::temp_dir().join(format!("cpd-mcp-similarity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("invoice.js"),
        "export function buildInvoice(order, customer, taxRate) {\n  const lines = [];\n  for (const item of order.items) {\n    const net = item.price * item.quantity;\n    lines.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = lines.reduce((sum, line) => sum + line.net, 0);\n  const tax = Math.round(subtotal * taxRate * 100) / 100;\n  return { number: nextInvoiceNumber(), customer: customer.id, lines, subtotal, tax, total: subtotal + tax };\n}\n",
    )
    .unwrap();
    let snippet = "export function buildCreditNote(refund, account, vatRate) {\n  const entries = [];\n  for (const item of refund.items) {\n    if (!item.refundable) continue;\n    const net = item.price * item.quantity;\n    entries.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = entries.reduce((sum, entry) => sum + entry.net, 0);\n  const vat = Math.round(subtotal * vatRate * 100) / 100;\n  logger.info('credit note', { account: account.id, subtotal });\n  return { number: nextCreditNoteNumber(), account: account.id, entries, subtotal, vat, total: subtotal + vat };\n}\n";
    let mut child = Command::new(cpd_bin())
        .args(["--mcp"])
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cpd --mcp");
    let code = serde_json::to_string(snippet).unwrap();
    let requests = [
        r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#.to_string(),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"check_duplication","arguments":{{"code":{code},"format":"javascript","similarity":0.7}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"check_duplication","arguments":{{"code":{code},"format":"javascript","similarity":0.95}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"check_duplication","arguments":{{"code":{code},"format":"javascript","similarity":2}}}}}}"#
        ),
        format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"check_duplication","arguments":{{"code":{code},"format":"javascript","similarity":1}}}}}}"#
        ),
    ];
    {
        let stdin = child.stdin.as_mut().unwrap();
        for req in &requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    let output = child.wait_with_output().expect("cpd --mcp must exit");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let responses: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON per line"))
        .collect();
    assert_eq!(responses.len(), 5, "stdout: {stdout}");
    let payload = |i: usize| -> serde_json::Value {
        serde_json::from_str(
            responses[i]["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    };
    let loose = payload(1);
    assert_eq!(loose["count"], 0, "no exact match: {loose}");
    assert_eq!(loose["similarCount"], 1, "{loose}");
    let hit = &loose["similar"][0];
    assert!(hit["file"].as_str().unwrap().ends_with("invoice.js"));
    assert_eq!(hit["name"], "buildInvoice");
    assert_eq!(hit["snippetName"], "buildCreditNote");
    let sim = hit["similarity"].as_f64().unwrap();
    assert!(sim > 0.7 && sim < 0.9, "got {sim}");
    let strict = payload(2);
    assert_eq!(strict["similarCount"], 0, "{strict}");
    let exact_only = payload(4);
    assert!(
        exact_only.get("similarCount").is_none(),
        "1 means exact matches only, no similarity section: {exact_only}"
    );
    assert!(
        responses[3]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("similarity"),
        "out-of-range ratio is a parameter error: {}",
        responses[3]
    );
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
    let responses: Vec<serde_json::Value> = stdout
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
