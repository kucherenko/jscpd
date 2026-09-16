// rust/crates/cpd/tests/dead_code.rs — the `--dead-code` mode end to end.
//
// These drive the real binary over the demo fixtures, so they check the thing
// a user actually runs: flag parsing, the mode switch, reporter selection and
// the exit code, not just the engine underneath.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

/// The repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crates/cpd is three levels below the repository root")
        .to_path_buf()
}

fn demo(sub: &str) -> PathBuf {
    repo_root().join("fixtures/dead-code-demo").join(sub)
}

fn run(args: &[&str]) -> Output {
    Command::new(cpd_bin())
        .args(args)
        .output()
        .expect("failed to run cpd")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn dead_code_reports_every_category_on_the_typescript_demo() {
    let path = demo("typescript");
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]);
    let text = stdout(&out);
    for expected in [
        "Unused files",
        "src/legacy-export.ts",
        "Unused exports",
        "renderReceipt",
        "Unused symbols",
        "describeTotal",
        "Unused imports",
        "roundToCents",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    assert!(
        text.contains("Found 4 dead code findings"),
        "the demo is pinned at four findings:\n{text}"
    );
}

#[test]
fn dead_code_reports_the_python_demo() {
    let path = demo("python");
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]);
    let text = stdout(&out);
    for expected in ["shop/legacy.py", "refund_order", "_unused_rounding"] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    assert!(
        !text.contains("complete_order"),
        "a name __init__.py re-exports and __main__.py calls is not dead:\n{text}"
    );
}

#[test]
fn the_basta_alias_runs_the_same_mode() {
    let path = demo("typescript");
    let with_flag = stdout(&run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]));
    let with_alias = stdout(&run(&[
        "--basta",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]));
    assert_eq!(
        with_flag.lines().filter(|l| l.starts_with(" - ")).count(),
        with_alias.lines().filter(|l| l.starts_with(" - ")).count(),
    );
}

#[test]
fn without_the_flag_jscpd_still_looks_for_duplicates() {
    let path = demo("typescript");
    let text = stdout(&run(&[
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
        "-r",
        "console",
    ]));
    assert!(
        !text.contains("dead code findings"),
        "dead-code detection must be opt-in:\n{text}"
    );
}

#[test]
fn raising_the_confidence_floor_drops_the_uncertain_findings() {
    let path = demo("typescript");
    let strict = stdout(&run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--min-confidence",
        "90",
        "--no-colors",
        "--no-tips",
    ]));
    assert!(
        !strict.contains("renderReceipt"),
        "an export something outside the scan could call is not a 90 finding:\n{strict}"
    );
    assert!(strict.contains("describeTotal"), "{strict}");
}

#[test]
fn categories_can_be_narrowed() {
    let path = demo("typescript");
    let text = stdout(&run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--dead-code-categories",
        "unused-import",
        "--no-colors",
        "--no-tips",
    ]));
    assert!(text.contains("roundToCents"), "{text}");
    assert!(!text.contains("Unused files"), "{text}");
    assert!(text.contains("Found 1 dead code finding"), "{text}");
}

#[test]
fn an_unknown_category_fails_with_a_message_rather_than_scanning() {
    let path = demo("typescript");
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--dead-code-categories",
        "unused-everything",
        "--no-colors",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown category"), "{stderr}");
}

#[test]
fn an_entry_glob_makes_a_file_live() {
    let path = demo("typescript");
    let text = stdout(&run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--entry",
        "src/legacy-export.ts",
        "--no-colors",
        "--no-tips",
    ]));
    assert!(
        !text.contains("Unused files"),
        "a project says once that a file is an entry point:\n{text}"
    );
}

#[test]
fn file_reporters_write_a_dead_code_report() {
    let path = demo("python");
    let out_dir = std::env::temp_dir().join(format!("basta-cli-{}", std::process::id()));
    std::fs::remove_dir_all(&out_dir).ok();
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "-r",
        "json,sarif",
        "-o",
        out_dir.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let json = std::fs::read_to_string(out_dir.join("basta-report.json")).expect("json report");
    assert!(json.contains("\"unused-file\""), "{json}");
    assert!(out_dir.join("basta-report.sarif").is_file());
    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn exit_code_is_returned_when_something_is_found() {
    let path = demo("python");
    let found = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--exit-code",
        "7",
        "-r",
        "silent",
    ]);
    assert_eq!(found.status.code(), Some(7));
}

#[test]
fn a_format_dead_code_cannot_analyze_is_refused_rather_than_scanning_nothing() {
    let path = demo("typescript");
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--format",
        "java",
        "--no-colors",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("selected no format"), "{stderr}");
}
