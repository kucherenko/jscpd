// rust/crates/cpd/tests/semantic.rs — `--semantic` against a stand-in
// embeddings server.
//
// The server answers the OpenAI embeddings protocol with bag-of-words
// vectors: every word of a text (camelCase and snake_case split, so that
// `unitPrice` and `unit_price` agree) is hashed into one of 256 buckets. Two
// functions that share their vocabulary point the same way, which is what a
// code model does for a function and its port to another language.

mod common;

use common::embeddings::Server;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

/// A cart total implemented in a Rust backend and again in a Svelte
/// component, plus a dozen unrelated functions on each side.
/// A folder for test `name`, gone with its cache and report folders.
fn fresh(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cpd-semantic-{name}-{}", std::process::id()));
    for dir in [root.clone(), beside(&root, "cache"), beside(&root, "out")] {
        let _ = std::fs::remove_dir_all(dir);
    }
    root
}

fn project(name: &str) -> PathBuf {
    let root = fresh(name);
    common::embeddings::cart_project(&root);
    root
}

fn run(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(cpd_bin());
    command
        .args(args)
        .args([
            "--min-tokens",
            "15",
            "--min-lines",
            "3",
            "--no-tips",
            "--no-colors",
        ])
        .arg(dir)
        .current_dir(dir)
        .env("JSCPD_CACHE_DIR", beside(dir, "cache"))
        .env_remove("JSCPD_SEMANTIC_API_KEY");
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("failed to run cpd")
}

/// A directory next to the scanned one, so reports and caches written there
/// are never scanned.
fn beside(dir: &Path, what: &str) -> PathBuf {
    dir.with_file_name(format!(
        "{}-{what}",
        dir.file_name().unwrap().to_string_lossy()
    ))
}

fn json_report(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> (Value, String) {
    let out = beside(dir, "out");
    let mut full: Vec<&str> = args.to_vec();
    full.extend([
        "--reporters",
        "json,silent",
        "--output",
        out.to_str().unwrap(),
    ]);
    let output = run(dir, &full, envs);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(output.status.success(), "cpd failed: {stderr}");
    let report = std::fs::read_to_string(out.join("jscpd-report.json")).unwrap();
    (serde_json::from_str(&report).unwrap(), stderr)
}

/// The semantic pairs of a JSON report, with `/` as the path separator on
/// every platform.
fn semantic_pairs(report: &Value) -> Vec<(String, String, f64)> {
    let name = |file: &Value| file["name"].as_str().unwrap().replace('\\', "/");
    report["duplicates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["kind"] == "semantic")
        .map(|d| {
            (
                name(&d["firstFile"]),
                name(&d["secondFile"]),
                d["similarity"].as_f64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_rust_function_and_its_svelte_port_are_a_semantic_clone() {
    let server = Server::start();
    let dir = project("pair");
    let args = [
        "--semantic",
        "--semantic-url",
        &server.url,
        "--semantic-model",
        "stand-in",
    ];

    let (report, stderr) = json_report(&dir, &args, &[]);
    let pairs = semantic_pairs(&report);
    assert_eq!(pairs.len(), 1, "{pairs:?}\n{stderr}");
    // The vectors go to a folder of their own for the scanned path.
    let folders: Vec<String> = std::fs::read_dir(beside(&dir, "cache").join("embeddings"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        folders.len() == 1 && folders[0].starts_with(&format!("{name}-")),
        "{folders:?}"
    );
    let (a, b, similarity) = &pairs[0];
    assert!(a.ends_with("backend/src/cart.rs"), "{a}");
    assert!(b.ends_with("frontend/src/Cart.svelte:typescript"), "{b}");
    assert!(*similarity > 0.6 && *similarity <= 1.0, "{similarity}");
    assert!(
        stderr.contains("Semantic clones (experimental): embedding 26 functions"),
        "{stderr}"
    );

    // One request carried every function: the model name, the texts without
    // comments, no `dimensions` unless configured, no key unless provided.
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let (body, auth) = &requests[0];
    assert_eq!(body["model"], "stand-in");
    assert!(body.get("dimensions").is_none());
    assert!(auth.is_none());
    let inputs: Vec<&str> = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert!(inputs.contains(&"function cartTotal(items: Line[], percent: number): number {\n  const subtotal = items.reduce((sum, line) => sum + line.unitPrice * line.quantity, 0);\n  const discount = Math.floor((subtotal * percent) / 100);\n  const shipping = subtotal - discount > 5000 ? 0 : 499;\n  return subtotal - discount + shipping;\n}"), "{inputs:#?}");

    // The second run reads every vector from the cache.
    let (again, stderr) = json_report(&dir, &args, &[]);
    assert_eq!(semantic_pairs(&again), pairs);
    assert_eq!(
        server.requests().len(),
        1,
        "no request when everything is cached"
    );
    assert!(
        stderr.contains("26 functions, all embeddings cached (stand-in)"),
        "{stderr}"
    );

    // --semantic-rebuild-cache sends every text again, and the run after it
    // reads the new vectors.
    let rebuild = [&args[..], &["--semantic-rebuild-cache"]].concat();
    let (rebuilt, stderr) = json_report(&dir, &rebuild, &[]);
    assert_eq!(semantic_pairs(&rebuilt), pairs);
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{stderr}");
    assert_eq!(requests[1].0["input"], requests[0].0["input"]);
    assert!(
        stderr.contains("embedding 26 functions with") && stderr.contains(", rebuilding the cache"),
        "{stderr}"
    );
    let (_, stderr) = json_report(&dir, &args, &[]);
    assert_eq!(server.requests().len(), 2);
    assert!(
        stderr.contains("26 functions, all embeddings cached (stand-in)"),
        "{stderr}"
    );

    // Without the flag nothing is embedded and nothing is found.
    let (plain, _) = json_report(&dir, &[], &[]);
    assert!(plain["duplicates"].as_array().unwrap().is_empty());
    assert_eq!(server.requests().len(), 2);
    cleanup(&dir);
}

#[test]
fn kind_filter_threshold_and_the_ai_reporter() {
    let server = Server::start();
    let dir = project("kind");
    let base = [
        "--semantic",
        "--semantic-url",
        &server.url,
        "--semantic-model",
        "stand-in",
    ];

    let output = run(
        &dir,
        &[&base[..], &["--reporters", "ai", "--kind", "semantic"]].concat(),
        &[],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("semantic]"), "{stdout}");
    assert!(stdout.contains("1 clones"), "{stdout}");

    let (strict, stderr) = json_report(
        &dir,
        &[&base[..], &["--semantic-threshold", "0.999"]].concat(),
        &[],
    );
    assert!(
        semantic_pairs(&strict).is_empty(),
        "the threshold is a cosine floor"
    );
    assert!(!stderr.contains("no calibrated thresholds"), "{stderr}");
    let (_, stderr) = json_report(&dir, &base, &[]);
    assert!(
        stderr.contains("jscpd has no calibrated thresholds for stand-in, so it uses 0.6 across languages and 0.75 within one"),
        "{stderr}"
    );
    // Only the threshold still a guess is named.
    let (_, stderr) = json_report(
        &dir,
        &[&base[..], &["--semantic-same-threshold", "0.9"]].concat(),
        &[],
    );
    assert!(
        stderr.contains("so it uses 0.6 across languages. Check the scores of a few pairs you know and set --semantic-threshold;"),
        "{stderr}"
    );

    let (_, stderr) = json_report(
        &dir,
        &[&base[..], &["--semantic-threshold", "7"]].concat(),
        &[],
    );
    assert!(
        stderr.contains("Warning: --semantic-threshold: 7 is outside (0, 1]; using 0.6"),
        "{stderr}"
    );
    assert!(
        stderr.contains("no calibrated thresholds for stand-in"),
        "the replacement is a guess too: {stderr}"
    );

    let (_, stderr) = json_report(
        &dir,
        &[&base[..], &["--semantic-same-threshold", "7"]].concat(),
        &[],
    );
    assert!(
        stderr.contains("Warning: --semantic-same-threshold: 7 is outside (0, 1]; using 0.75"),
        "{stderr}"
    );

    let (_, stderr) = json_report(&dir, &["--kind", "semantic"], &[]);
    assert!(
        stderr.contains("--kind semantic: no such clones are found without --semantic"),
        "{stderr}"
    );

    let (_, stderr) = json_report(&dir, &["--semantic-model", "x"], &[]);
    assert!(
        stderr.contains("Warning: --semantic-model has no effect without --semantic"),
        "{stderr}"
    );
    let (_, stderr) = json_report(
        &dir,
        &["--semantic-model", "x", "--semantic-scope", "same"],
        &[],
    );
    assert!(
        stderr.contains("--semantic-scope and --semantic-model have no effect"),
        "{stderr}"
    );
    cleanup(&dir);
}

#[test]
fn the_config_file_section_sets_model_params_and_key_from_the_environment() {
    let server = Server::start();
    let dir = project("config");
    let config = json!({
        "semantic": {
            "enabled": true,
            "url": server.url,
            "model": "from-config",
            "params": {"task": "code2code.query"},
            "cache": false
        }
    });
    std::fs::write(dir.join(".jscpd.json"), config.to_string()).unwrap();

    let (report, _) = json_report(&dir, &[], &[("JSCPD_SEMANTIC_API_KEY", "sk-test")]);
    assert_eq!(semantic_pairs(&report).len(), 1);
    let (body, auth) = &server.requests()[0];
    assert_eq!(body["model"], "from-config");
    assert_eq!(body["task"], "code2code.query");
    assert_eq!(auth.as_deref(), Some("Bearer sk-test"));
    assert!(
        !beside(&dir, "cache").exists(),
        "\"cache\": false writes nothing"
    );

    // Flags win over the file.
    let (_, _) = json_report(&dir, &["--semantic-model", "from-flag"], &[]);
    assert_eq!(server.requests()[1].0["model"], "from-flag");
    let (_, stderr) = json_report(&dir, &["--semantic-rebuild-cache"], &[]);
    assert!(
        stderr.contains(
            "Warning: --semantic-rebuild-cache has no effect: the config file turns the cache off"
        ),
        "{stderr}"
    );

    // A model jscpd has calibrated gets the prefix of its calibration; the
    // API gets the model's name as typed.
    let (_, _) = json_report(&dir, &["--semantic-model", "qwen3-embedding:0.6b"], &[]);
    let (body, _) = server.requests().pop().unwrap();
    assert_eq!(body["model"], "qwen3-embedding:0.6b");
    let inputs = body["input"].as_array().unwrap();
    let instruction = "Instruct: Given a code snippet, retrieve code that implements the same functionality\nQuery:";
    for input in inputs {
        let text = input.as_str().unwrap();
        let code = text.strip_prefix(instruction).unwrap_or_default();
        assert!(
            code.starts_with("pub fn ") || code.starts_with("function "),
            "{text}"
        );
    }

    std::fs::write(
        dir.join(".jscpd.json"),
        r#"{"semantic": {"enabled": true, "apiKey": "sk-leak"}}"#,
    )
    .unwrap();
    let output = run(&dir, &["--reporters", "silent"], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a key in a config file stops the run: {stderr}"
    );
    assert!(
        stderr.contains("'semantic.apiKey' is not read from config files"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("sk-leak"),
        "the key is never printed: {stderr}"
    );
    cleanup(&dir);
}

#[test]
fn a_config_file_cannot_send_code_to_another_machine_on_its_own() {
    let dir = project("elsewhere");
    std::fs::write(
        dir.join(".jscpd.json"),
        json!({"semantic": {"enabled": true, "url": "https://collector.example/v1"}}).to_string(),
    )
    .unwrap();
    let output = run(
        &dir,
        &["--reporters", "silent"],
        &[("JSCPD_SEMANTIC_API_KEY", "sk-test")],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("would send the code of every function to collector.example"),
        "{stderr}"
    );
    // --compare runs the model too, but it does not stand for a typed
    // --semantic: the config's URL is refused the same way.
    let output = Command::new(cpd_bin())
        .args(["--compare", "backend", "frontend", "-r", "silent"])
        .current_dir(&dir)
        .env("JSCPD_CACHE_DIR", beside(&dir, "cache"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("Error: --compare: the config file's semantic.url would send the code of every function to collector.example"),
        "{stderr}"
    );
    cleanup(&dir);
}

#[test]
fn an_unreachable_server_fails_the_run_with_a_hint() {
    let dir = project("down");
    // Bind and drop: nothing listens on the port any more.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}/v1");
    let output = run(
        &dir,
        &[
            "--semantic",
            "--semantic-url",
            &url,
            "--reporters",
            "silent",
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("Error: --semantic: cannot reach"),
        "{stderr}"
    );
    assert!(
        stderr.contains("ollama pull unclemusclez/jina-embeddings-v2-base-code"),
        "{stderr}"
    );
    cleanup(&dir);
}

#[test]
fn modes_that_do_not_detect_ignore_semantic() {
    let dir = project("modes");
    let output = run(
        &dir,
        &["--semantic", "--complexity", "--reporters", "ai"],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("Warning: --semantic is ignored by --complexity"),
        "{stderr}"
    );
    cleanup(&dir);
}

fn cleanup(dir: &Path) {
    for path in [dir.to_path_buf(), beside(dir, "cache"), beside(dir, "out")] {
        std::fs::remove_dir_all(path).ok();
    }
}

/// A second TypeScript implementation of the cart total, for the scope
/// tests: two TypeScript functions and one Rust function do the same thing.
fn add_second_implementation(dir: &Path) {
    std::fs::write(
        dir.join("frontend/src/checkout.ts"),
        "export function sumCart(lines: Line[], couponPercent: number): number {\n  let subtotal = 0;\n  for (const line of lines) subtotal += line.unitPrice * line.quantity;\n  const discount = Math.floor((subtotal * couponPercent) / 100);\n  const shipping = subtotal - discount > 5000 ? 0 : 499;\n  return subtotal - discount + shipping;\n}\n",
    )
    .unwrap();
}

#[test]
fn scope_selects_pairs_within_or_across_languages() {
    let server = Server::start();
    let dir = project("scope");
    add_second_implementation(&dir);
    let base = [
        "--semantic",
        "--semantic-url",
        &server.url,
        "--semantic-model",
        "stand-in",
    ];
    let files = |scope: &str| {
        let (report, _) = json_report(
            &dir,
            &[&base[..], &["--semantic-scope", scope]].concat(),
            &[],
        );
        let mut pairs: Vec<String> = semantic_pairs(&report)
            .into_iter()
            .map(|(a, b, _)| {
                let name = |p: &str| p.rsplit('/').next().unwrap().to_string();
                format!("{}~{}", name(&a), name(&b))
            })
            .collect();
        pairs.sort();
        pairs
    };
    assert_eq!(files("same"), ["Cart.svelte:typescript~checkout.ts"]);
    let cross = files("cross");
    assert!(
        !cross.is_empty() && cross.iter().all(|p| p.contains("cart.rs")),
        "{cross:?}"
    );
    let all = files("all");
    assert!(all.len() == 1 + cross.len(), "{all:?}");
    cleanup(&dir);
}

#[test]
fn the_local_provider_asks_for_the_download_first() {
    let dir = project("local");
    let output = run(&dir, &["--semantic", "--reporters", "silent"], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("is not downloaded yet"), "{stderr}");
    assert!(
        stderr.contains("Run `jscpd --semantic-download` once"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("Semantic clones (experimental)"),
        "fails before scanning: {stderr}"
    );
    // Another model's message downloads that model.
    let output = run(
        &dir,
        &[
            "--semantic",
            "--semantic-model",
            "jina-embeddings-v2-base-code",
            "--reporters",
            "silent",
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Run `jscpd --semantic-download jina-embeddings-v2-base-code` once"),
        "{stderr}"
    );
    cleanup(&dir);
}

#[test]
fn a_download_that_does_not_match_its_checksum_is_refused() {
    // A mirror that answers every file with the same few bytes.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mirror = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                line.clear();
            }
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"
            );
        }
    });
    let dir = project("download");
    let output = run(&dir, &["--semantic-download"], &[("HF_ENDPOINT", &mirror)]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("nomic-ai/CodeRankEmbed/resolve/3c4b6080"),
        "{stderr}"
    );
    assert!(
        stderr.contains("expected 1525 bytes with 5ff856a4"),
        "{stderr}"
    );
    let models = beside(&dir, "cache").join("models");
    let leftovers: Vec<_> = walk_files(&models);
    assert!(leftovers.is_empty(), "nothing is kept: {leftovers:?}");

    // --semantic-model picks the model to download.
    let output = run(
        &dir,
        &[
            "--semantic-download",
            "--semantic-model",
            "jina-embeddings-v2-base-code",
        ],
        &[("HF_ENDPOINT", &mirror)],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expected 1216 bytes with e426aa68"),
        "{stderr}"
    );
    assert!(!stderr.contains("no effect"), "{stderr}");

    let output = run(
        &dir,
        &[
            "--semantic-download",
            "--semantic-url",
            "http://127.0.0.1:1/v1",
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("an embeddings API needs none"), "{stderr}");
    cleanup(&dir);
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .flat_map(|e| match e.path().is_dir() {
            true => walk_files(&e.path()),
            false => vec![e.path()],
        })
        .collect()
}

/// `cpd --compare backend frontend` in the project, with the stand-in
/// server's vectors.
fn compare(dir: &Path, url: &str, extra: &[&str]) -> Output {
    compare_at(dir, url, ["15", "3"], extra)
}

/// [`compare`] with `--min-tokens` and `--min-lines` of `thresholds`.
fn compare_at(dir: &Path, url: &str, thresholds: [&str; 2], extra: &[&str]) -> Output {
    let [tokens, lines] = thresholds;
    Command::new(cpd_bin())
        .args(["--compare", "backend", "frontend"])
        .args(["--semantic-url", url, "--semantic-model", "stand-in"])
        .args(["--min-tokens", tokens, "--min-lines", lines, "--no-colors"])
        .args(extra)
        .current_dir(dir)
        .env("JSCPD_CACHE_DIR", beside(dir, "cache"))
        .env_remove("JSCPD_SEMANTIC_API_KEY")
        .output()
        .expect("failed to run cpd")
}

#[test]
fn compare_pairs_a_port_by_code_and_a_short_one_by_name() {
    let server = Server::start();
    let dir = project("compare");
    // Rounding: a full function in Rust, a one-line arrow in TypeScript,
    // too short to count on its own.
    std::fs::write(
        dir.join("backend/src/money.rs"),
        "pub fn round_cents(amount: i64) -> i64 {\n    let cents = amount % 100;\n    let rounded = amount - cents;\n    if cents >= 50 { rounded + 100 } else { rounded }\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("frontend/src/money.ts"),
        "export const roundCents = (amount: number) => { const cents = amount % 100; const rounded = amount - cents; return cents >= 50 ? rounded + 100 : rounded; };\n",
    )
    .unwrap();
    let out = beside(&dir, "out");
    let output = compare(
        &dir,
        &server.url,
        &["-r", "console-full,json,html", "-o", out.to_str().unwrap()],
    );
    // Reports print native paths; compare them with `/` everywhere.
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    let report: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("jscpd-compare.json")).unwrap())
            .unwrap();
    let pairs: Vec<(String, String, String)> = report["code"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let name = |f: &Value| f["name"].as_str().unwrap().to_string();
            (
                name(&p["a"]),
                name(&p["b"]),
                p["matchedBy"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("cart_total".into(), "cartTotal".into(), "code".into()),
            ("round_cents".into(), "roundCents".into(), "name".into()),
        ],
        "{stdout}"
    );
    let side = |k: usize, key: &str| report["code"]["sides"][k][key].as_u64().unwrap();
    // Twelve fillers and two ported functions in Rust; twelve fillers and
    // the cart total in TypeScript, the one-line arrow not counting.
    assert_eq!((side(0, "functions"), side(0, "matched")), (14, 2));
    assert_eq!((side(1, "functions"), side(1, "matched")), (13, 1));
    let path = |v: &Value| v.as_str().unwrap().replace('\\', "/");
    assert_eq!(
        path(&report["code"]["sides"][0]["files"][0]["file"]),
        "src/cart.rs"
    );
    assert_eq!(
        path(&report["code"]["sides"][0]["files"][0]["counterpart"]),
        "src/Cart.svelte"
    );
    assert!(
        stdout.starts_with(
            " 14% 2 of 14 functions in backend have a counterpart in frontend\n  8% 1 of 13 functions in frontend have a counterpart in backend\n"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("Only in backend (12):"), "{stdout}");
    assert!(
        stdout.contains("src/money.rs:1 round_cents  src/money.ts:1 roundCents"),
        "{stdout}"
    );
    // No clone detection ran: one request, the functions of both sides.
    assert_eq!(server.requests().len(), 1);

    // The html reporter writes one page with the data it draws.
    let html = std::fs::read_to_string(out.join("jscpd-compare.html")).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(!html.contains("/*DATA*/"), "the data is in place");
    for name in ["cart_total", "cartTotal", "round_cents", "roundCents"] {
        assert!(
            html.contains(&format!("\"{name}\"")),
            "{name} is on the page"
        );
    }
    // The JSON report lists what is ready to port, most called first.
    assert!(report["code"]["sides"][0]["readyToPort"].is_array());

    // A second run reads every vector from the cache; a changed function
    // is embedded again, alone, and the report uses its new vector.
    let similarity = |report: &Value| {
        report["code"]["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["a"]["name"] == "round_cents")
            .map(|p| p["similarity"].as_f64().unwrap())
            .unwrap()
    };
    let rerun = || {
        let output = compare(
            &dir,
            &server.url,
            &["-r", "json", "-o", out.to_str().unwrap()],
        );
        assert!(output.status.success());
        let text = std::fs::read_to_string(out.join("jscpd-compare.json")).unwrap();
        (
            serde_json::from_str::<Value>(&text).unwrap(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    };
    let (again, stderr) = rerun();
    assert_eq!(server.requests().len(), 1, "{stderr}");
    assert!(stderr.contains("all embeddings cached"), "{stderr}");
    std::fs::write(
        dir.join("frontend/src/money.ts"),
        "export const roundCents = (amount: number) => { const cents = amount % 100; const rounded = amount - cents; return cents >= 50 ? rounded + 100 : rounded + 0; };\n",
    )
    .unwrap();
    let (changed, stderr) = rerun();
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "{stderr}");
    assert_eq!(requests[1].0["input"].as_array().unwrap().len(), 1);
    assert!(stderr.contains("embedding 1 of"), "{stderr}");
    assert_ne!(similarity(&changed), similarity(&again));

    // A scope from the config file has no effect, and says so.
    std::fs::write(
        dir.join(".jscpd.json"),
        r#"{"semantic": {"scope": "same"}}"#,
    )
    .unwrap();
    let output = compare(&dir, &server.url, &["-r", "silent"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("Warning: --compare pairs the functions of the two sides whatever their languages; the semantic scope 'same' has no effect"),
        "{stderr}"
    );
    cleanup(&dir);
}

#[test]
fn compare_needs_two_separate_paths() {
    let dir = project("compare-paths");
    let run = |paths: &[&str]| {
        let output = Command::new(cpd_bin())
            .arg("--compare")
            .args(paths)
            .current_dir(&dir)
            .env("JSCPD_CACHE_DIR", beside(&dir, "cache"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    assert!(
        run(&["backend"])
            .contains("Error: --compare takes two paths, the two sides to compare (got 1)")
    );
    assert!(
        run(&["backend", "backend/src"]).contains(
            "Error: --compare: backend and backend/src overlap; give two separate folders"
        )
    );
    let output = Command::new(cpd_bin())
        .args(["--compare", "--dead-code", "backend", "frontend"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "clap refuses the mix");
    // Dead code switched on by the config file refuses it too, rather than
    // running instead.
    std::fs::write(dir.join(".jscpd.json"), r#"{"deadCode": true}"#).unwrap();
    let stderr = run(&["backend", "frontend"]);
    assert!(
        stderr.contains("Error: --dead-code cannot be combined with --complexity, --dashboard, --health or --compare"),
        "{stderr}"
    );
    cleanup(&dir);
}

/// Issue #1163: an `else if` after `#if … #endif` is no C# function, so
/// compare neither lists it nor counts every `if (` as a call to it. The
/// branch is small, so the thresholds go down to where it would count.
#[test]
fn compare_drops_csharp_branches_recovered_as_functions() {
    let server = Server::start();
    let dir = fresh("compare-csharp-branches");
    let csharp = "public class Shapes\n{\n    public static double Area(object shape)\n    {\n        if (shape is Square q)\n        {\n            return q.Side * q.Side;\n        }\n#if ROUND\n        else if (shape is Circle c)\n        {\n            return 3.14 * c.R * c.R;\n        }\n#endif\n        else if (shape is Rect r)\n        {\n            return r.W * r.H;\n        }\n        return 0;\n    }\n}\n";
    let python = "def area(shape):\n    if isinstance(shape, Square):\n        return shape.side * shape.side\n    return 0\n";
    for (file, text) in [
        ("backend/Shapes.cs", csharp),
        ("frontend/shapes.py", python),
    ] {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let out = beside(&dir, "out");
    let output = compare_at(
        &dir,
        &server.url,
        ["1", "1"],
        &["-r", "json", "-o", out.to_str().unwrap()],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("jscpd-compare.json")).unwrap())
            .unwrap();
    let side = &report["code"]["sides"][0];
    assert_eq!(side["path"], "backend");
    assert_eq!(side["functions"], 1, "only Area: {report}");
    for list in ["unmatched", "readyToPort"] {
        let names: Vec<&str> = side[list]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect();
        assert!(!names.contains(&"if"), "{list}: {report}");
    }
    cleanup(&dir);
}
