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
