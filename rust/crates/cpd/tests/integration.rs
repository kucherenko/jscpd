// rust/crates/cpd/tests/integration.rs

use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

fn cpd_bin() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_cpd for integration tests; prefer it because
    // it already points at the correct target directory and executable suffix.
    if let Ok(bin) = std::env::var("CARGO_BIN_EXE_cpd") {
        return PathBuf::from(bin);
    }
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/cpd");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path
}

fn build_cpd() {
    let status = Command::new("cargo")
        .args(["build", "--bin", "cpd"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("failed to run cargo build");
    assert!(status.success(), "cargo build must succeed");
}

fn maybe_bin() -> Option<PathBuf> {
    build_cpd();
    let bin = cpd_bin();
    if bin.exists() { Some(bin) } else { None }
}

fn run_cpd<I, S>(args: I) -> Option<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let bin = maybe_bin()?;
    Some(
        Command::new(&bin)
            .args(args)
            .output()
            .expect("failed to run cpd"),
    )
}

fn run_cpd_config(fixture: &str) -> Option<Output> {
    let config_path = fixtures_dir().join(fixture);
    run_cpd([
        "--config",
        config_path.to_str().unwrap(),
        "--reporters",
        "silent",
        ".",
    ])
}

fn assert_config_loads_successfully(fixture: &str) {
    let output = run_cpd_config(fixture).expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("config file"),
        "config {} should not have config errors, got: {}",
        fixture,
        stderr
    );
    assert!(
        stderr.contains("Using config from"),
        "config {} should load config file, got: {}",
        fixture,
        stderr
    );
}

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path
}

#[test]
fn help_exits_zero() {
    let output = run_cpd(["--help"]).expect("cpd binary must exist");
    assert!(output.status.success(), "--help must exit 0");
}

#[test]
fn list_prints_formats() {
    let output = run_cpd(["--list"]).expect("cpd binary must exist");
    assert!(output.status.success(), "--list must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("javascript"),
        "--list must include 'javascript'"
    );
    assert!(stdout.contains("python"), "--list must include 'python'");
}

#[test]
fn scan_nonexistent_path_exits_without_panic() {
    let _output = run_cpd(["--reporters", "silent", "/tmp/cpd-nonexistent-xyz-12345"]);
    // Just verify it doesn't crash (SIGSEGV etc.) — any exit code is fine
}

#[test]
fn store_flag_prints_warning() {
    let output = run_cpd(["--store", "leveldb", "--reporters", "silent", "."])
        .expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not supported")
            || stderr.contains("Warning")
            || stderr.contains("ignored"),
        "--store must print warning, got stderr: {}",
        stderr
    );
}

#[test]
fn time_printed_automatically() {
    let output = run_cpd(["--reporters", "console", "."]).expect("cpd binary must exist");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("time:") && (stdout.contains("ms") || stdout.contains("s")),
        "timing should be printed automatically, got: {}",
        stdout
    );
}

#[test]
fn time_not_printed_for_silent() {
    let output = run_cpd(["--reporters", "silent", "."]).expect("cpd binary must exist");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("time:"),
        "timing should NOT be printed for silent reporter, got: {}",
        stdout
    );
}

#[test]
fn explicit_config_malformed_json_exits_with_error() {
    let output = run_cpd_config("malformed_json.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "malformed config must exit non-zero, got: {}",
        output.status
    );
    assert!(
        stderr.contains("config file"),
        "stderr must mention 'config file', got: {}",
        stderr
    );
    assert!(
        stderr.contains("ParseError")
            || stderr.contains("parse")
            || stderr.contains("trailing comma")
            || stderr.contains("expected"),
        "stderr must mention JSON parse error, got: {}",
        stderr
    );
}

#[test]
fn explicit_config_unknown_field_warns() {
    let output = run_cpd_config("unknown_fields.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "unknown field must not be fatal, got: {}",
        output.status
    );
    assert!(
        stderr.contains("minTokenz"),
        "stderr must mention the unknown field 'minTokenz', got: {}",
        stderr
    );
    assert!(
        stderr.contains("unknown field"),
        "stderr must contain 'unknown field', got: {}",
        stderr
    );
}

#[test]
fn explicit_config_invalid_mode_warns() {
    let output = run_cpd_config("invalid_mode.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "invalid mode must not be fatal, got: {}",
        output.status
    );
    assert!(
        stderr.contains("mode"),
        "stderr must mention 'mode', got: {}",
        stderr
    );
    assert!(
        stderr.contains("fast"),
        "stderr must mention 'fast', got: {}",
        stderr
    );
    assert!(
        stderr.contains("mild") || stderr.contains("weak") || stderr.contains("strict"),
        "stderr must mention valid modes, got: {}",
        stderr
    );
}

#[test]
fn explicit_config_valid_succeeds() {
    let output = run_cpd_config("valid.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "valid config must exit 0, got: {}",
        output.status
    );
    assert!(
        stderr.contains("Using config from"),
        "should load config file, got: {}",
        stderr
    );
}

#[test]
fn explicit_config_type_mismatch_exits_with_error() {
    let output = run_cpd_config("type_mismatch.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "type mismatch config must exit non-zero, got: {}",
        output.status
    );
    assert!(
        stderr.contains("config file") || stderr.contains("invalid type"),
        "stderr must mention config/type error, got: {}",
        stderr
    );
}

#[test]
fn explicit_config_v4_fields_warns() {
    let output = run_cpd_config("v4_fields.jscpd.json").expect("cpd binary must exist");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "v4 removed fields must not be fatal, got: {}",
        output.status
    );
    assert!(
        stderr.contains("removed from config file in v5"),
        "stderr must warn about removed v4 fields, got: {}",
        stderr
    );
}

#[test]
fn config_with_ignore_and_ignore_pattern_succeeds() {
    assert_config_loads_successfully("v4_ignore_and_pattern.jscpd.json");
}

#[test]
fn config_with_ignore_pattern_regex_succeeds() {
    assert_config_loads_successfully("v4_ignore_pattern_regex.jscpd.json");
}

#[test]
fn config_with_mixed_v4_fields_and_ignore_succeeds() {
    assert_config_loads_successfully("v4_mixed_ignore_fields.jscpd.json");
}

#[test]
fn config_with_jsonc_comments_and_ignore_succeeds() {
    assert_config_loads_successfully("v4_ignore_with_jsonc.jscpd.json");
}

#[test]
fn cli_ignore_flag_accepted() {
    let output = run_cpd([
        "--ignore",
        "*.test.js,*.spec.ts",
        "--reporters",
        "silent",
        ".",
    ])
    .expect("cpd binary must exist");

    assert!(
        output.status.success(),
        "--ignore flag must be accepted, got: {}",
        output.status
    );
}

#[test]
fn cli_ignore_pattern_flag_accepted() {
    let output = run_cpd(["--ignore-pattern", "function", "--reporters", "silent", "."])
        .expect("cpd binary must exist");

    assert!(
        output.status.success(),
        "--ignore-pattern flag must be accepted, got: {}",
        output.status
    );
}

#[test]
fn report_snippets_populated_when_scan_root_differs_from_cwd() {
    // Regression test for the empty "show code" bug: the html/json reporters
    // re-read each source file from disk to render snippets. When the scan
    // root is a *subdirectory* of the CWD (e.g. config `path: ["pkg"]`, or
    // `cpd pkg`), the displayed source path must stay resolvable from the CWD
    // — otherwise the read fails silently and the snippet renders empty.
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    // Fresh, uniquely-named temp workspace with a `pkg/` subdirectory.
    let root = std::env::temp_dir().join(format!("cpd-snippet-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let pkg = root.join("pkg");
    std::fs::create_dir_all(&pkg).expect("create pkg dir");

    let dup = "function greet(name) {\n  \
        const message = \"Hello, \" + name + \"!\";\n  \
        console.log(message);\n  \
        console.log(\"Welcome to the system\");\n  \
        console.log(\"Have a nice day now\");\n  \
        return message;\n}\n";
    std::fs::write(pkg.join("a.js"), dup).expect("write a.js");
    std::fs::write(pkg.join("b.js"), dup).expect("write b.js");

    let out = root.join("report");

    // Run from `root`, scanning the `pkg` subdirectory: scan root != CWD.
    let output = Command::new(&bin)
        .args([
            "pkg",
            "--min-tokens",
            "10",
            "--reporters",
            "json,html",
            "--output",
            out.to_str().unwrap(),
        ])
        .current_dir(&root)
        .output()
        .expect("failed to run cpd");
    assert!(
        output.status.success(),
        "cpd must succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A code line that appears only inside the duplicated snippet, never in
    // report metadata — so its presence proves the snippet was read.
    let marker = "Welcome to the system";

    let json = std::fs::read_to_string(out.join("jscpd-report.json")).expect("json report exists");
    assert!(
        json.contains(marker),
        "JSON report must contain the duplicated snippet; empty means the read path regressed"
    );

    let html = std::fs::read_to_string(out.join("jscpd-report.html")).expect("html report exists");
    assert!(
        html.contains(marker),
        "HTML report must contain the duplicated snippet; empty means the read path regressed"
    );

    // Display path must be CWD-relative and therefore keep the scan-root
    // subdirectory prefix (so it still resolves from the CWD).
    assert!(
        json.contains("pkg/a.js") || json.contains(r"pkg\\a.js"),
        "source path must stay relative to the CWD (keep the `pkg/` prefix)"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_both_ignore_flags_work_together() {
    let output = run_cpd([
        "--ignore",
        "*.test.js",
        "--ignore-pattern",
        "function",
        "--reporters",
        "silent",
        ".",
    ])
    .expect("cpd binary must exist");

    assert!(
        output.status.success(),
        "both --ignore and --ignore-pattern must work together, got: {}",
        output.status
    );
}
