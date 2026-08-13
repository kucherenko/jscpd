// rust/crates/cpd/tests/integration.rs

use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

fn cpd_bin() -> PathBuf {
    // Compile-time CARGO_BIN_EXE_cpd: cargo guarantees the bin target is
    // built before integration tests run, with the correct target dir and
    // executable suffix. Do NOT shell out to `cargo build` here — each test
    // process doing so raced: cargo refreshes target/debug/cpd by removing
    // and re-creating a hardlink, and a parallel test spawning the binary in
    // that window failed with NotFound (flaked on macOS CI).
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

fn maybe_bin() -> Option<PathBuf> {
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

// --- snippet regression test (scan root != CWD) --------------------------------

#[test]
fn report_snippets_populated_when_scan_root_differs_from_cwd() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

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

    let marker = "Welcome to the system";

    let json = std::fs::read_to_string(out.join("jscpd-report.json")).expect("json report exists");
    assert!(
        json.contains(marker),
        "JSON fragment must contain snippet text, not be empty"
    );

    let html = std::fs::read_to_string(out.join("jscpd-report.html")).expect("html report exists");
    assert!(
        html.contains(marker),
        "HTML report must contain snippet text, not be empty"
    );

    // source_id should be scan-root-relative (just "a.js", not "pkg/a.js")
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let first_name = parsed["duplicates"][0]["firstFile"]["name"]
        .as_str()
        .unwrap_or("");
    assert!(
        first_name == "a.js" || first_name == "b.js",
        "source path must be scan-root-relative (a.js or b.js), got: {}",
        first_name
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sarif_includes_original_uri_base_ids() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    let root = std::env::temp_dir().join(format!("cpd-sarif-root-{}", std::process::id()));
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

    let output = Command::new(&bin)
        .args([
            "pkg",
            "--min-tokens",
            "10",
            "--reporters",
            "sarif",
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

    let sarif = std::fs::read_to_string(out.join("jscpd-report.sarif")).expect("sarif exists");
    let parsed: serde_json::Value = serde_json::from_str(&sarif).expect("valid JSON");
    let run = &parsed["runs"][0];

    assert!(
        run.get("originalUriBaseIds").is_some(),
        "SARIF must include originalUriBaseIds when source_root is set"
    );

    // #915: tool.driver.version must match what `cpd --version` prints,
    // bundled from this crate's version at build time.
    assert_eq!(
        run["tool"]["driver"]["version"],
        env!("CARGO_PKG_VERSION"),
        "SARIF driver.version must match the cpd crate version"
    );

    let uri = run["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap_or("");
    assert!(
        uri == "a.js" || uri == "b.js",
        "SARIF artifact URI must be scan-root-relative, got: {}",
        uri
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sarif_error_tokens_flag_controls_result_level() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    let root = std::env::temp_dir().join(format!("cpd-sarif-level-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");

    let dup = "function greet(name) {\n  \
        const message = \"Hello, \" + name + \"!\";\n  \
        console.log(message);\n  \
        console.log(\"Welcome to the system\");\n  \
        console.log(\"Have a nice day now\");\n  \
        return message;\n}\n";
    std::fs::write(src.join("a.js"), dup).expect("write a.js");
    std::fs::write(src.join("b.js"), dup).expect("write b.js");

    let run_and_get_level = |extra_args: &[&str]| -> String {
        let out = root.join("report");
        let _ = std::fs::remove_dir_all(&out);
        let mut args = vec![
            "src",
            "--min-tokens",
            "10",
            "--reporters",
            "sarif",
            "--output",
            out.to_str().unwrap(),
        ];
        args.extend_from_slice(extra_args);
        let output = Command::new(&bin)
            .args(&args)
            .current_dir(&root)
            .output()
            .expect("failed to run cpd");
        assert!(
            output.status.success(),
            "cpd must succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let sarif = std::fs::read_to_string(out.join("jscpd-report.sarif")).expect("sarif exists");
        let parsed: serde_json::Value = serde_json::from_str(&sarif).expect("valid JSON");
        parsed["runs"][0]["results"][0]["level"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };

    // #908: default stays warning; a low cutoff turns the clone into an error,
    // and a cutoff above the clone size leaves it a warning.
    assert_eq!(run_and_get_level(&[]), "warning");
    assert_eq!(run_and_get_level(&["--sarif-error-tokens", "10"]), "error");
    assert_eq!(
        run_and_get_level(&["--sarif-error-tokens", "100000"]),
        "warning"
    );

    // Exceeding --threshold escalates every result to error. This run exits
    // non-zero (ThresholdExceeded), so check the report without asserting
    // success — reporters run before the threshold check fails the build.
    let out = root.join("report");
    let _ = std::fs::remove_dir_all(&out);
    let output = Command::new(&bin)
        .args([
            "src",
            "--min-tokens",
            "10",
            "--reporters",
            "sarif",
            "--threshold",
            "0",
            "--output",
            out.to_str().unwrap(),
        ])
        .current_dir(&root)
        .output()
        .expect("failed to run cpd");
    assert!(
        !output.status.success(),
        "cpd must exit non-zero when duplication exceeds --threshold"
    );
    let sarif = std::fs::read_to_string(out.join("jscpd-report.sarif")).expect("sarif exists");
    let parsed: serde_json::Value = serde_json::from_str(&sarif).expect("valid JSON");
    assert_eq!(
        parsed["runs"][0]["results"][0]["level"], "error",
        "results must be errors when duplication exceeds --threshold"
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

#[test]
fn blame_populated_when_scan_root_differs_from_cwd() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };
    // Requires git on PATH; skip quietly if unavailable.
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }

    let root = std::env::temp_dir().join(format!("cpd-blame-subdir-{}", std::process::id()));
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

    // Init a git repo at `root` and commit, so `git blame` has data.
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git command failed to spawn");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Blame Tester"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);

    let out = root.join("report");

    // Scan the `pkg` subdirectory from the repo root (scan root != file dirs
    // relative to CWD after scan-root relativization).
    let output = Command::new(&bin)
        .args([
            "pkg",
            "--min-tokens",
            "10",
            "--blame",
            "--reporters",
            "json",
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

    let json = std::fs::read_to_string(out.join("jscpd-report.json")).expect("json report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let first_file = &parsed["duplicates"][0]["firstFile"];
    assert!(
        first_file.get("blame").is_some(),
        "blame data must be populated when scan root differs from CWD, got: {}",
        first_file
    );
    assert_eq!(
        first_file["blame"]["author"].as_str().unwrap_or(""),
        "Blame Tester",
        "blame author must come from git history"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn single_file_scan_preserves_filename() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    let root = std::env::temp_dir().join(format!("cpd-single-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create dir");

    let dup = "function greet(name) {\n  \
        const message = \"Hello, \" + name + \"!\";\n  \
        console.log(message);\n  \
        console.log(\"Welcome to the system\");\n  \
        console.log(\"Have a nice day now\");\n  \
        return message;\n}\n";
    std::fs::write(root.join("a.js"), dup).expect("write a.js");
    std::fs::write(root.join("b.js"), dup).expect("write b.js");

    let out = root.join("report");
    let a_path = root.join("a.js");
    let b_path = root.join("b.js");

    let output = Command::new(&bin)
        .args([
            a_path.to_str().unwrap(),
            b_path.to_str().unwrap(),
            "--min-tokens",
            "10",
            "--reporters",
            "json",
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

    let json = std::fs::read_to_string(out.join("jscpd-report.json")).expect("json report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let first_name = parsed["duplicates"][0]["firstFile"]["name"]
        .as_str()
        .unwrap_or("");
    assert!(
        first_name == "a.js" || first_name == "b.js",
        "source_id must be the filename (not empty), got: '{}'",
        first_name
    );

    let fragment = parsed["duplicates"][0]["fragment"].as_str().unwrap_or("");
    assert!(
        fragment.contains("Welcome to the system"),
        "snippet must be populated for single-file scan"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sarif_multi_root_distinct_artifact_indexes() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    let root = std::env::temp_dir().join(format!("cpd-multi-sarif-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir_a = root.join("alpha/src");
    let dir_b = root.join("beta/src");
    std::fs::create_dir_all(&dir_a).expect("create alpha");
    std::fs::create_dir_all(&dir_b).expect("create beta");

    let dup = "function greet(name) {\n  \
        const message = \"Hello, \" + name + \"!\";\n  \
        console.log(message);\n  \
        console.log(\"Welcome to the system\");\n  \
        console.log(\"Have a nice day now\");\n  \
        return message;\n}\n";
    // Same relative path under two different roots
    std::fs::write(dir_a.join("a.js"), dup).expect("write alpha/src/a.js");
    std::fs::write(dir_b.join("a.js"), dup).expect("write beta/src/a.js");

    let out = root.join("report");

    let output = Command::new(&bin)
        .args([
            "alpha",
            "beta",
            "--min-tokens",
            "10",
            "--reporters",
            "sarif",
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

    let sarif = std::fs::read_to_string(out.join("jscpd-report.sarif")).expect("sarif exists");
    let parsed: serde_json::Value = serde_json::from_str(&sarif).expect("valid JSON");
    let artifacts = parsed["runs"][0]["artifacts"]
        .as_array()
        .expect("artifacts array");

    assert!(
        artifacts.len() >= 2,
        "same relative path under different roots must produce distinct artifacts, got {}",
        artifacts.len()
    );

    let _ = std::fs::remove_dir_all(&root);
}

// --- cross-formats e2e -------------------------------------------------------

fn cross_formats_fixture_dir() -> PathBuf {
    fixtures_dir().join("cross_formats")
}

fn run_cross_formats(extra_args: &[&str]) -> Output {
    let dir = cross_formats_fixture_dir();
    let mut args: Vec<&str> = vec![
        "--min-tokens",
        "20",
        "--min-lines",
        "1",
        "--reporters",
        "console",
    ];
    args.extend_from_slice(extra_args);
    let dir_str = dir.to_str().unwrap();
    args.push(dir_str);
    run_cpd(args).expect("cpd binary must exist")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn cross_formats_off_no_ts_js_clone() {
    let output = run_cross_formats(&[]);
    let stdout = stdout_of(&output);
    assert!(
        !(stdout.contains("a.ts") && stdout.contains("b.js")),
        "without --cross-formats the TS/JS pair must not be reported, got: {stdout}"
    );
}

#[test]
fn cross_formats_flag_detects_ts_js_clone() {
    let output = run_cross_formats(&["--cross-formats", "javascript,typescript"]);
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("a.ts") && stdout.contains("b.js"),
        "--cross-formats must report the TS/JS pair, got: {stdout}"
    );
}

#[test]
fn cross_formats_preset_detects_ts_js_clone() {
    let output = run_cross_formats(&["--cross-formats", "js-ts"]);
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("a.ts") && stdout.contains("b.js"),
        "--cross-formats js-ts preset must report the TS/JS pair, got: {stdout}"
    );
}

#[test]
fn cross_formats_from_config_file() {
    let config_path = fixtures_dir().join("cross_formats.jscpd.json");
    let dir = cross_formats_fixture_dir();
    let output = run_cpd([
        "--config",
        config_path.to_str().unwrap(),
        "--reporters",
        "console",
        dir.to_str().unwrap(),
    ])
    .expect("cpd binary must exist");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("a.ts") && stdout.contains("b.js"),
        "crossFormats from config file must report the TS/JS pair, got: {stdout}"
    );
}

#[test]
fn cross_formats_shown_in_debug_output() {
    let output = run_cpd(["--cross-formats", "javascript,typescript", "--debug", "."])
        .expect("cpd binary must exist");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("cross_formats"),
        "--debug must include cross_formats in merged config, got: {stdout}"
    );
    assert!(
        stdout.contains("typescript"),
        "--debug must show the configured group, got: {stdout}"
    );
}
