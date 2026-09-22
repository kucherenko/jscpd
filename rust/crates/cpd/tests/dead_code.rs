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

/// `fixtures/dead-code-demo/config`: a project whose `.jscpd.json` carries a
/// dead-code section — an inline framework, an entry glob, a confidence floor
/// and a threshold of its own.
fn run_config_demo(extra: &[&str]) -> Output {
    let path = demo("config");
    let config = path.join(".jscpd.json");
    let mut args = vec![
        "--dead-code",
        path.to_str().unwrap(),
        "--config",
        config.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn the_config_files_dead_code_section_configures_the_run() {
    let out = run_config_demo(&[]);
    let text = stdout(&out);
    assert!(
        text.contains("Frameworks: job-runner"),
        "the section defines a framework inline:\n{text}"
    );
    assert!(
        !text.contains("nightly-reprint.job.js"),
        "which starts the job:\n{text}"
    );
    assert!(
        !text.contains("seed-labels.js"),
        "the section's `entry` roots the tool:\n{text}"
    );
    assert!(
        !text.contains("drainQueue"),
        "the section's minConfidence of 90 drops the 85% export:\n{text}"
    );
    assert!(text.contains("Found 1 dead code findings"), "{text}");
    assert!(
        out.status.success(),
        "14% dead is under the section's own threshold of 40, whatever the \
         clone threshold of 10 beside it says:\n{text}"
    );
}

#[test]
fn a_flag_overrides_the_section() {
    let text = stdout(&run_config_demo(&["--min-confidence", "60"]));
    assert!(text.contains("drainQueue"), "{text}");
    assert!(text.contains("Found 2 dead code findings"), "{text}");

    let out = run_config_demo(&["--threshold", "5"]);
    assert!(
        !out.status.success(),
        "--threshold beats the section's: 14% is over 5"
    );
}

#[test]
fn without_the_section_the_same_project_reads_as_mostly_dead() {
    let path = demo("config");
    let text = stdout(&run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]));
    assert!(text.contains("Found 4 dead code findings"), "{text}");
}

#[test]
fn an_unknown_framework_in_the_section_is_refused() {
    let dir = std::env::temp_dir().join(format!("jscpd-dead-code-section-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.json");
    std::fs::write(&config, r#"{"deadCode": {"framework": ["nextjs"]}}"#).unwrap();
    let path = demo("typescript");
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--config",
        config.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(out.status.code(), Some(1));
    let errors = String::from_utf8_lossy(&out.stderr);
    assert!(errors.contains("'nextjs' is not a framework"), "{errors}");
}

#[test]
fn rust_dead_code_comes_from_the_compilers_diagnostics_named_in_the_config() {
    // `fixtures/dead-code-demo/rust` commits the output of `cargo check
    // --message-format=json`, so this needs no Rust toolchain at test time.
    let path = demo("rust");
    let out = Command::new(cpd_bin())
        .current_dir(&path)
        .args(["--dead-code", ".", "--no-colors", "--no-tips"])
        .output()
        .expect("failed to run cpd");
    let text = stdout(&out);
    for expected in [
        "reprint",
        "render_return",
        "HashMap",
        "fits",
        "Found 6 dead code findings",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    assert!(
        !text.contains("labels_for"),
        "a public function of a library is not reported:\n{text}"
    );

    // Without the section there is nothing to read, and nothing to report.
    let out = run(&[
        "--dead-code",
        path.to_str().unwrap(),
        "--no-colors",
        "--no-tips",
    ]);
    assert!(
        stdout(&out).contains("No dead code found"),
        "{}",
        stdout(&out)
    );
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
