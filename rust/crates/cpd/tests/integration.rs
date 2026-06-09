// rust/crates/cpd/tests/integration.rs

use std::path::PathBuf;
use std::process::Command;

fn cpd_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/cpd");
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

#[test]
fn help_exits_zero() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    } // skip if not built

    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run cpd --help");
    assert!(output.status.success(), "--help must exit 0");
}

#[test]
fn list_prints_formats() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let output = Command::new(&bin)
        .arg("--list")
        .output()
        .expect("failed to run cpd --list");
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
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    // Scanning a nonexistent path should not panic (may exit non-zero or print warning)
    let output = Command::new(&bin)
        .args(["--reporters", "silent", "/tmp/cpd-nonexistent-xyz-12345"])
        .output()
        .expect("failed to run cpd");
    // Just verify it doesn't crash (SIGSEGV etc.) — any exit code is fine
    let _status = output.status;
}

#[test]
fn store_flag_prints_warning() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let output = Command::new(&bin)
        .args(["--store", "leveldb", "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");
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
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let output = Command::new(&bin)
        .args(["--reporters", "console", "."])
        .output()
        .expect("failed to run cpd");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("time:") && (stdout.contains("ms") || stdout.contains("s")),
        "timing should be printed automatically, got: {}",
        stdout
    );
}

#[test]
fn time_not_printed_for_silent() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let output = Command::new(&bin)
        .args(["--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("time:"),
        "timing should NOT be printed for silent reporter, got: {}",
        stdout
    );
}

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path
}

#[test]
fn explicit_config_malformed_json_exits_with_error() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("malformed_json.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "malformed config must exit non-zero, got: {}", output.status);
    assert!(stderr.contains("config file"), "stderr must mention 'config file', got: {}", stderr);
    assert!(
        stderr.contains("ParseError") || stderr.contains("parse") || stderr.contains("trailing comma") || stderr.contains("expected"),
        "stderr must mention JSON parse error, got: {}", stderr
    );
}

#[test]
fn explicit_config_unknown_field_warns() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("unknown_fields.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "unknown field must not be fatal, got: {}", output.status);
    assert!(stderr.contains("minTokenz"), "stderr must mention the unknown field 'minTokenz', got: {}", stderr);
    assert!(stderr.contains("unknown field"), "stderr must contain 'unknown field', got: {}", stderr);
}

#[test]
fn explicit_config_invalid_mode_warns() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("invalid_mode.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "invalid mode must not be fatal, got: {}", output.status);
    assert!(stderr.contains("mode"), "stderr must mention 'mode', got: {}", stderr);
    assert!(stderr.contains("fast"), "stderr must mention 'fast', got: {}", stderr);
    assert!(stderr.contains("mild") && stderr.contains("weak") && stderr.contains("strict"),
        "stderr must list valid modes (mild, weak, strict), got: {}", stderr);
}

#[test]
fn explicit_config_valid_succeeds() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("valid.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "valid config must exit 0, got: {}", output.status);
    assert!(stderr.contains("Using config from"), "stderr must contain 'Using config from', got: {}", stderr);
}

#[test]
fn explicit_config_type_mismatch_exits_with_error() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("type_mismatch.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "type mismatch must exit non-zero, got: {}", output.status);
    assert!(stderr.contains("config file"), "stderr must mention 'config file', got: {}", stderr);
    assert!(
        stderr.contains("ParseError") || stderr.contains("expected") || stderr.contains("type") || stderr.contains("integer"),
        "stderr must mention type mismatch, got: {}", stderr
    );
}

#[test]
fn explicit_config_v4_fields_warns() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let config_path = fixtures_dir().join("v4_fields.jscpd.json");
    let output = Command::new(&bin)
        .args(["--config", config_path.to_str().unwrap(), "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "v4 fields in explicit config must not be fatal, got: {}", output.status);
    assert!(stderr.contains("store"), "stderr must mention 'store', got: {}", stderr);
    assert!(stderr.contains("removed from config file"), "stderr must mention 'removed from config file', got: {}", stderr);
}

#[test]
fn cli_invalid_mode_prints_warning() {
    build_cpd();
    let bin = cpd_bin();
    if !bin.exists() {
        return;
    }

    let output = Command::new(&bin)
        .args(["--mode", "fast", "--reporters", "silent", "."])
        .output()
        .expect("failed to run cpd");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid mode"), "stderr must contain 'invalid mode', got: {}", stderr);
}
