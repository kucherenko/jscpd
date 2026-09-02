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

/// Run cpd in a given working directory (for config auto-discovery tests).
fn run_cpd_in_dir(dir: &std::path::Path) -> Option<Output> {
    let bin = maybe_bin()?;
    Some(
        Command::new(&bin)
            .args(["--reporters", "silent", "."])
            .current_dir(dir)
            .output()
            .expect("failed to run cpd"),
    )
}

#[test]
fn dot_config_subfolder_is_discovered() {
    let dir = std::env::temp_dir().join(format!("cpd-dotconfig-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".config")).unwrap();
    std::fs::write(dir.join(".config/jscpd.json"), r#"{"minTokens": 42}"#).unwrap();

    let output = run_cpd_in_dir(&dir).expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        stderr.contains("Using config from .config/jscpd.json"),
        "must discover .config/jscpd.json, got stderr: {}",
        stderr
    );
}

#[test]
fn root_config_wins_over_dot_config_subfolder() {
    let dir = std::env::temp_dir().join(format!("cpd-dotconfig-prec-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".config")).unwrap();
    std::fs::write(dir.join(".jscpd.json"), r#"{"minTokens": 42}"#).unwrap();
    std::fs::write(dir.join(".config/jscpd.json"), r#"{"minTokens": 99}"#).unwrap();

    let output = run_cpd_in_dir(&dir).expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        stderr.contains("Using config from .jscpd.json"),
        "root .jscpd.json must take precedence, got stderr: {}",
        stderr
    );
}

#[test]
fn unknown_format_prints_warning() {
    let output = run_cpd(["--format", "zzzznotalang", "--reporters", "silent", "."])
        .expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("'zzzznotalang' is not a supported format"),
        "unknown --format must print warning, got stderr: {}",
        stderr
    );
}

#[test]
fn known_format_prints_no_warning() {
    let output = run_cpd(["--format", "csharp", "--reporters", "silent", "."])
        .expect("cpd binary must exist");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("not a supported format"),
        "known --format must not warn, got stderr: {}",
        stderr
    );
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
fn codeclimate_reporter_writes_code_quality_report() {
    let bin = match maybe_bin() {
        Some(b) => b,
        None => return,
    };

    let root = std::env::temp_dir().join(format!("cpd-codeclimate-{}", std::process::id()));
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

    let out = root.join("report");
    let output = Command::new(&bin)
        .args([
            "src",
            "--min-tokens",
            "10",
            "--reporters",
            "codeclimate",
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

    let report = std::fs::read_to_string(out.join("gl-code-quality-report.json"))
        .expect("gl-code-quality-report.json exists");
    let parsed: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
    let issues = parsed.as_array().expect("report must be a JSON array");
    assert_eq!(
        issues.len(),
        2,
        "one clone must yield an issue per fragment"
    );

    for issue in issues {
        assert_eq!(issue["type"], "issue");
        assert_eq!(issue["check_name"], "jscpd/duplicate-code");
        assert_eq!(issue["severity"], "minor");
        let path = issue["location"]["path"].as_str().unwrap_or("");
        assert!(
            path == "a.js" || path == "b.js",
            "path must be scan-root-relative, got: {}",
            path
        );
        assert!(issue["location"]["lines"]["begin"].is_u64());
        let fp = issue["fingerprint"].as_str().unwrap_or("");
        assert!(
            fp.len() == 16 && fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be 16-char hex, got: {}",
            fp
        );
    }
    assert_ne!(
        issues[0]["fingerprint"], issues[1]["fingerprint"],
        "the two issues of one clone must have distinct fingerprints"
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

// --- --summary ---

fn run_summary(extra_args: &[&str]) -> Output {
    let dir = cross_formats_fixture_dir();
    let mut args: Vec<&str> = vec![
        "--min-tokens",
        "20",
        "--min-lines",
        "1",
        "--no-colors",
        "--no-tips",
    ];
    args.extend_from_slice(extra_args);
    let dir_str = dir.to_str().unwrap();
    args.push(dir_str);
    run_cpd(args).expect("cpd binary must exist")
}

#[test]
fn summary_flag_prints_summary_block() {
    let output = run_summary(&["--summary", "--reporters", "console"]);
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Summary") && stdout.contains("Top files:"),
        "--summary must print the summary block, got: {stdout}"
    );
    assert!(
        stdout.contains("Top folders:"),
        "--summary must print the folder rollup, got: {stdout}"
    );
    assert!(
        stdout.contains("a.ts"),
        "summary must list scanned files, got: {stdout}"
    );
}

#[test]
fn summary_absent_by_default() {
    let output = run_summary(&["--reporters", "console"]);
    let stdout = stdout_of(&output);
    assert!(
        !stdout.contains("Top files:"),
        "summary must be opt-in; default output changed: {stdout}"
    );
}

#[test]
fn summary_ai_reporter_prints_compact_block() {
    let output = run_summary(&["--summary", "--reporters", "ai"]);
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("files (tokens/lines/size/cx/dup%):"),
        "ai reporter must print the compact summary, got: {stdout}"
    );
}

#[test]
fn summary_json_key_present_only_when_enabled() {
    let out_on = std::env::temp_dir().join("cpd-summary-json-on");
    let _ = std::fs::remove_dir_all(&out_on);
    run_summary(&[
        "--summary",
        "--reporters",
        "json",
        "--output",
        out_on.to_str().unwrap(),
    ]);
    let report = std::fs::read_to_string(out_on.join("jscpd-report.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
    let summary = parsed
        .get("summary")
        .expect("--summary must add a summary key to the JSON report");
    assert!(summary.get("files").is_some(), "summary.files expected");
    assert!(summary.get("folders").is_some(), "summary.folders expected");
    assert!(
        summary.get("totalFiles").is_some(),
        "summary.totalFiles expected (camelCase)"
    );

    let out_off = std::env::temp_dir().join("cpd-summary-json-off");
    let _ = std::fs::remove_dir_all(&out_off);
    run_summary(&["--reporters", "json", "--output", out_off.to_str().unwrap()]);
    let report = std::fs::read_to_string(out_off.join("jscpd-report.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(
        parsed.get("summary").is_none(),
        "without --summary the JSON schema must be unchanged"
    );
}

#[test]
fn summary_by_invalid_value_warns_and_defaults() {
    let output = run_summary(&[
        "--summary",
        "--summary-by",
        "bogus",
        "--reporters",
        "console",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("summary-by") && stderr.contains("bogus"),
        "invalid --summary-by must warn, got: {stderr}"
    );
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("(by tokens;"),
        "invalid metric must fall back to tokens, got: {stdout}"
    );
}

#[test]
fn summary_top_limits_file_and_folder_lists() {
    let output = run_summary(&["--summary", "--summary-top", "1", "--reporters", "console"]);
    let stdout = stdout_of(&output);
    let rows_after = |heading: &str| {
        stdout
            .split(heading)
            .nth(1)
            .unwrap_or("")
            .lines()
            .skip(2) // empty remainder of the heading line, then the column header
            .take_while(|l| l.starts_with("  "))
            .count()
    };
    assert_eq!(
        rows_after("Top files:"),
        1,
        "--summary-top 1 must list exactly one file, got: {stdout}"
    );
    assert_eq!(
        rows_after("Top folders:"),
        1,
        "--summary-top 1 must list exactly one folder, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Baseline (--baseline / --update-baseline / --fail-on-new-clones, issue #944)
// ---------------------------------------------------------------------------

/// Unique scratch dir per call: parallel test threads must not share files.
fn baseline_tmp_dir(suffix: &str) -> PathBuf {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cpd-it-baseline-{}-{}-{}",
        suffix,
        std::process::id(),
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn dup_function(name: &str) -> String {
    format!(
        r#"function {name}(alpha, beta, gamma) {{
  const first = alpha * beta + gamma;
  const second = first - alpha / beta;
  const third = second + gamma * gamma;
  const fourth = third - first + alpha;
  const fifth = fourth * second - beta;
  console.log(first, second, third, fourth, fifth);
  return fifth + fourth + third + second + first;
}}
"#
    )
}

/// Scan dir with one duplicated function shared by a.js and b.js.
/// Returns (scan_dir, baseline_path); the baseline file lives outside the
/// scan dir so it is never scanned itself.
fn setup_baseline_scan(suffix: &str) -> (PathBuf, PathBuf) {
    let root = baseline_tmp_dir(suffix);
    let scan = root.join("src");
    std::fs::create_dir_all(&scan).unwrap();
    std::fs::write(scan.join("a.js"), dup_function("known")).unwrap();
    std::fs::write(scan.join("b.js"), dup_function("known")).unwrap();
    (scan, root.join("baseline.json"))
}

/// Add a second, distinct duplicated pair to the scan dir.
fn add_new_duplicate(scan: &std::path::Path) {
    let body = dup_function("fresh").replace("alpha", "omega");
    std::fs::write(scan.join("c.js"), &body).unwrap();
    std::fs::write(scan.join("d.js"), &body).unwrap();
}

fn run_baseline_cpd(scan: &std::path::Path, baseline: &std::path::Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "--min-tokens",
        "20",
        "--baseline",
        baseline.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    args.push(scan.to_str().unwrap());
    run_cpd(args).expect("cpd binary must exist")
}

#[test]
fn update_baseline_creates_file_and_prints_counts() {
    let (scan, baseline) = setup_baseline_scan("create");
    let output = run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    assert!(output.status.success(), "update run must exit 0");
    assert!(baseline.exists(), "baseline file must be created");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("1 fingerprints added, 0 removed (1 total)"),
        "update must print added/removed counts, got: {stderr}"
    );
    let content = std::fs::read_to_string(&baseline).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["version"], 1);
    assert_eq!(parsed["fingerprints"].as_object().unwrap().len(), 1);
}

#[test]
fn baseline_gate_passes_when_no_new_clones() {
    let (scan, baseline) = setup_baseline_scan("pass");
    run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    let output = run_baseline_cpd(
        &scan,
        &baseline,
        &["--fail-on-new-clones", "--reporters", "silent"],
    );
    assert!(
        output.status.success(),
        "no new clones must exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn baseline_gate_fails_on_new_clones() {
    let (scan, baseline) = setup_baseline_scan("fail");
    run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    add_new_duplicate(&scan);
    let output = run_baseline_cpd(
        &scan,
        &baseline,
        &["--fail-on-new-clones", "--reporters", "silent"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "new clones must exit 1, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("new clones not in the baseline"),
        "gate must explain the failure, got: {stderr}"
    );
}

#[test]
fn baseline_gate_allows_n_new_clones() {
    let (scan, baseline) = setup_baseline_scan("allow-n");
    run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    add_new_duplicate(&scan);
    let output = run_baseline_cpd(
        &scan,
        &baseline,
        &["--fail-on-new-clones", "1", "--reporters", "silent"],
    );
    assert!(
        output.status.success(),
        "one new clone within --fail-on-new-clones 1 must exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_baseline_file_errors_with_update_hint() {
    let (scan, baseline) = setup_baseline_scan("missing");
    let output = run_baseline_cpd(&scan, &baseline, &["--reporters", "silent"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") && stderr.contains("--update-baseline"),
        "missing baseline must hint at --update-baseline, got: {stderr}"
    );
}

#[test]
fn fail_on_new_clones_requires_baseline() {
    let output =
        run_cpd(["--fail-on-new-clones", "--reporters", "silent", "."]).expect("cpd binary");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--fail-on-new-clones requires --baseline"),
        "got: {stderr}"
    );
}

#[test]
fn update_baseline_requires_baseline() {
    let output = run_cpd(["--update-baseline", "--reporters", "silent", "."]).expect("cpd binary");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--update-baseline requires --baseline"),
        "got: {stderr}"
    );
}

#[test]
fn json_report_marks_new_clones_and_statistics() {
    let (scan, baseline) = setup_baseline_scan("json");
    run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    add_new_duplicate(&scan);
    let report_dir = scan.parent().unwrap().join("report");
    let output = run_baseline_cpd(
        &scan,
        &baseline,
        &[
            "--reporters",
            "json",
            "--output",
            report_dir.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = std::fs::read_to_string(report_dir.join("jscpd-report.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["statistics"]["total"]["newClones"], 1);
    let dups = parsed["duplicates"].as_array().unwrap();
    assert_eq!(dups.len(), 2);
    assert_eq!(
        dups.iter().filter(|d| d["isNew"] == true).count(),
        1,
        "exactly the added pair must be new: {content}"
    );
}

#[test]
fn console_marks_new_clones() {
    let (scan, baseline) = setup_baseline_scan("console");
    run_baseline_cpd(
        &scan,
        &baseline,
        &["--update-baseline", "--reporters", "silent"],
    );
    add_new_duplicate(&scan);
    let output = run_baseline_cpd(&scan, &baseline, &["--reporters", "console", "--no-colors"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[NEW]"),
        "console must flag new clones, got: {stdout}"
    );
    assert!(
        stdout.contains("Found 2 clones (1 new)."),
        "found-count must include new count, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Ephemeral baseline from a git ref (--baseline-from-ref, issue #944 phase 2)
// ---------------------------------------------------------------------------

/// Run git in `dir` with identity/signing config that works on any machine.
fn git_in(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.email=cpd-test@example.com",
            "-c",
            "user.name=cpd-test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("failed to run git")
}

/// Git repo whose HEAD commit contains src/a.js and src/b.js sharing one
/// duplicated function. Returns the repo root.
fn setup_git_baseline_repo(suffix: &str) -> PathBuf {
    let root = baseline_tmp_dir(suffix);
    let scan = root.join("src");
    std::fs::create_dir_all(&scan).unwrap();
    std::fs::write(scan.join("a.js"), dup_function("known")).unwrap();
    std::fs::write(scan.join("b.js"), dup_function("known")).unwrap();
    assert!(git_in(&root, &["init", "-q"]).status.success());
    assert!(git_in(&root, &["add", "-A"]).status.success());
    let commit = git_in(&root, &["commit", "-q", "-m", "base"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    root
}

fn run_from_ref_cpd(root: &std::path::Path, git_ref: &str, extra: &[&str]) -> Output {
    let scan = root.join("src");
    let mut args = vec!["--min-tokens", "20", "--baseline-from-ref", git_ref];
    args.extend_from_slice(extra);
    args.push(scan.to_str().unwrap());
    run_cpd(args).expect("cpd binary must exist")
}

#[test]
fn baseline_from_ref_passes_when_no_new_clones() {
    let root = setup_git_baseline_repo("ref-pass");
    let output = run_from_ref_cpd(
        &root,
        "HEAD",
        &["--fail-on-new-clones", "--reporters", "silent"],
    );
    assert!(
        output.status.success(),
        "clones present in the base ref must not fail the gate, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The temporary worktree must be cleaned up and unregistered.
    let list = git_in(&root, &["worktree", "list"]);
    let worktrees = String::from_utf8_lossy(&list.stdout);
    assert_eq!(
        worktrees.lines().count(),
        1,
        "no leftover worktrees expected, got: {worktrees}"
    );
}

#[test]
fn baseline_from_ref_fails_on_new_uncommitted_pair() {
    let root = setup_git_baseline_repo("ref-fail");
    add_new_duplicate(&root.join("src"));
    let output = run_from_ref_cpd(
        &root,
        "HEAD",
        &["--fail-on-new-clones", "--reporters", "silent"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "a clone absent from the base ref must fail the gate, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("1 new clones not in the baseline"),
        "got: {stderr}"
    );
}

#[test]
fn baseline_from_ref_unknown_ref_errors_with_fetch_hint() {
    let root = setup_git_baseline_repo("ref-missing");
    let output = run_from_ref_cpd(&root, "no-such-ref", &["--reporters", "silent"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") && stderr.contains("fetch"),
        "missing ref must mention fetching in shallow checkouts, got: {stderr}"
    );
}

#[test]
fn baseline_from_ref_conflicts_with_baseline_flag() {
    let output = run_cpd([
        "--baseline",
        "b.json",
        "--baseline-from-ref",
        "HEAD",
        "--reporters",
        "silent",
        ".",
    ])
    .expect("cpd binary must exist");
    assert!(
        !output.status.success(),
        "--baseline and --baseline-from-ref must conflict"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("not both"),
        "got: {stderr}"
    );
}

#[test]
fn baseline_from_ref_marks_new_clone_in_console() {
    let root = setup_git_baseline_repo("ref-console");
    add_new_duplicate(&root.join("src"));
    let output = run_from_ref_cpd(&root, "HEAD", &["--reporters", "console", "--no-colors"]);
    assert!(output.status.success(), "no gate requested, must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Found 2 clones (1 new)."),
        "known pair stays known, added pair is new, got: {stdout}"
    );
}

// ── Binary name follows the invoked executable ──────────────────────────────
//
// `cpd` and `jscpd` are two bin targets built from the same main.rs. The clap
// command name used to be the literal "cpd", so `jscpd --version` printed
// `cpd 5.x.y`. The name is now taken from argv[0] at runtime.

fn run_named_bin(bin: &str, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn version_output_uses_invoked_binary_name() {
    let jscpd = run_named_bin(env!("CARGO_BIN_EXE_jscpd"), &["--version"]);
    let stdout = String::from_utf8_lossy(&jscpd.stdout);
    assert!(
        stdout.starts_with("jscpd "),
        "jscpd --version should start with 'jscpd ', got: {stdout:?}"
    );
    assert!(
        stdout.trim().ends_with(env!("CARGO_PKG_VERSION")),
        "jscpd --version should end with the crate version, got: {stdout:?}"
    );

    let cpd = run_named_bin(env!("CARGO_BIN_EXE_cpd"), &["--version"]);
    let stdout = String::from_utf8_lossy(&cpd.stdout);
    assert!(
        stdout.starts_with("cpd "),
        "cpd --version should start with 'cpd ', got: {stdout:?}"
    );
}

#[test]
fn help_usage_line_uses_invoked_binary_name() {
    let jscpd = run_named_bin(env!("CARGO_BIN_EXE_jscpd"), &["--help"]);
    let stdout = String::from_utf8_lossy(&jscpd.stdout);
    assert!(
        stdout.contains("Usage: jscpd"),
        "jscpd --help usage line should name jscpd, got: {stdout}"
    );

    let cpd = run_named_bin(env!("CARGO_BIN_EXE_cpd"), &["--help"]);
    let stdout = String::from_utf8_lossy(&cpd.stdout);
    assert!(
        stdout.contains("Usage: cpd"),
        "cpd --help usage line should name cpd, got: {stdout}"
    );
}
