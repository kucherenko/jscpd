use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{Reporter, ReporterOptions, create_reporter};
use std::{collections::HashMap, path::PathBuf, process, time::Duration};

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-blame-test-{}-{}", process::id(), suffix));
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn make_stats() -> Statistics {
    Statistics {
        total: StatRow {
            lines: 100,
            tokens: 500,
            sources: 5,
            clones: 1,
            duplicated_lines: 10,
            duplicated_tokens: 50,
            percentage: 10.0,
            percentage_tokens: 10.0,
            ..StatRow::default()
        },
        formats: HashMap::new(),
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn make_clone_with_blame() -> CpdClone {
    let loc = Location {
        line: 5,
        column: 0,
        offset: 0,
    };
    let end_loc = Location {
        line: 15,
        column: 0,
        offset: 100,
    };
    let blame = BlameEntry {
        commit_sha: "deadbeef1234".to_string(),
        author: "Bob Smith".to_string(),
        timestamp: 1700000000,
    };
    CpdClone {
        format: "javascript".to_string(),
        fragment_a: Fragment {
            source_id: "src/foo.js".to_string(),
            start: loc.clone(),
            end: end_loc.clone(),
            range: [0, 100],
            blame: Some(blame.clone()),
        },
        fragment_b: Fragment {
            source_id: "src/bar.js".to_string(),
            start: loc,
            end: end_loc,
            range: [0, 100],
            blame: Some(blame),
        },
        token_count: 50,
    }
}

fn make_clone_no_blame() -> CpdClone {
    let loc = Location {
        line: 1,
        column: 0,
        offset: 0,
    };
    CpdClone {
        format: "javascript".to_string(),
        fragment_a: Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 10],
            blame: None,
        },
        fragment_b: Fragment {
            source_id: "b.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        },
        token_count: 10,
    }
}

fn run_blame_reporter(
    name: &str,
    suffix: &str,
    clone: CpdClone,
    blame: bool,
) -> (PathBuf, Box<dyn Reporter>) {
    let dir = tmp_dir(suffix);
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = blame;
    let reporter =
        create_reporter(name, &opts).unwrap_or_else(|| panic!("{} reporter must exist", name));
    let ctx = ReportContext {
        stats: &make_stats(),
        duration: Duration::ZERO,
    };
    reporter.report(&[clone], &ctx, &dir).unwrap();
    (dir, reporter)
}

#[test]
fn json_reporter_includes_blame_sha() {
    let (dir, _reporter) = run_blame_reporter("json", "json", make_clone_with_blame(), true);

    let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
    assert!(
        content.contains("deadbeef1234"),
        "JSON must include blame SHA"
    );
    assert!(
        content.contains("Bob Smith"),
        "JSON must include blame author"
    );
}

#[test]
fn json_reporter_blame_none_serializes_as_null() {
    let (dir, _reporter) = run_blame_reporter("json", "json-null", make_clone_no_blame(), true);

    let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let first_file = &parsed["duplicates"][0]["firstFile"];
    assert!(
        first_file.get("blame").is_none(),
        "JSON firstFile should not contain blame field when blame is None, got: {:?}",
        first_file
    );
}

#[test]
fn sarif_reporter_blame_in_properties() {
    let (dir, _reporter) = run_blame_reporter("sarif", "sarif", make_clone_with_blame(), true);

    let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
    assert!(
        content.contains("deadbeef1234"),
        "SARIF must include blame SHA in properties"
    );
}

#[test]
fn sarif_reporter_no_panic_on_none_blame() {
    let (_dir, _reporter) = run_blame_reporter("sarif", "sarif-none", make_clone_no_blame(), true);
    // reporter already invoked; just assert it didn't panic by reaching here
}

#[test]
fn console_full_reporter_no_panic_with_blame() {
    run_blame_reporter(
        "console-full",
        "console-full",
        make_clone_with_blame(),
        true,
    );
}

#[test]
fn console_full_reporter_no_panic_no_blame() {
    run_blame_reporter(
        "console-full",
        "console-full-none",
        make_clone_no_blame(),
        false,
    );
}
