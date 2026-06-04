use std::{collections::HashMap, path::PathBuf};
use cpd_reporter::reporter::{create_reporter, ReporterOptions};
use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, Statistics, StatRow};

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-blame-test-{}", suffix));
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn make_stats() -> Statistics {
    Statistics {
        total: StatRow {
            lines: 100, tokens: 500, sources: 5, clones: 1,
            duplicated_lines: 10, duplicated_tokens: 50,
            percentage: 10.0, percentage_tokens: 10.0,
        },
        formats: HashMap::new(),
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn make_clone_with_blame() -> CpdClone {
    let loc = Location { line: 5, column: 0, offset: 0 };
    let end_loc = Location { line: 15, column: 0, offset: 100 };
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
    let loc = Location { line: 1, column: 0, offset: 0 };
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

#[test]
fn json_reporter_includes_blame_sha() {
    let dir = tmp_dir("json");
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = true;
    let reporter = create_reporter("json", &opts).expect("json reporter must exist");
    reporter.report(&[make_clone_with_blame()], &make_stats(), &dir).unwrap();

    let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
    assert!(content.contains("deadbeef1234"), "JSON must include blame SHA");
    assert!(content.contains("Bob Smith"), "JSON must include blame author");
}

#[test]
fn json_reporter_blame_none_serializes_as_null() {
    let dir = tmp_dir("json-null");
    let opts = ReporterOptions::new(dir.clone());
    let reporter = create_reporter("json", &opts).expect("json reporter must exist");
    reporter.report(&[make_clone_no_blame()], &make_stats(), &dir).unwrap();

    let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
    assert!(
        content.contains("\"blame\": null") || content.contains("\"blame\":null"),
        "JSON fragment with no blame must have null blame field, got: {}",
        &content[..content.len().min(500)]
    );
}

#[test]
fn sarif_reporter_blame_in_properties() {
    let dir = tmp_dir("sarif");
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = true;
    let reporter = create_reporter("sarif", &opts).expect("sarif reporter must exist");
    reporter.report(&[make_clone_with_blame()], &make_stats(), &dir).unwrap();

    let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
    assert!(content.contains("deadbeef1234"), "SARIF must include blame SHA in properties");
}

#[test]
fn sarif_reporter_no_panic_on_none_blame() {
    let dir = tmp_dir("sarif-none");
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = true;
    let reporter = create_reporter("sarif", &opts).expect("sarif reporter must exist");
    let result = reporter.report(&[make_clone_no_blame()], &make_stats(), &dir);
    assert!(result.is_ok(), "SARIF reporter must handle None blame gracefully");
}

#[test]
fn console_full_reporter_no_panic_with_blame() {
    let dir = tmp_dir("console-full");
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = true;
    let reporter = create_reporter("console-full", &opts).expect("console-full must exist");
    let result = reporter.report(&[make_clone_with_blame()], &make_stats(), &dir);
    assert!(result.is_ok(), "console-full with blame must not panic");
}

#[test]
fn console_full_reporter_no_panic_no_blame() {
    let dir = tmp_dir("console-full-none");
    let mut opts = ReporterOptions::new(dir.clone());
    opts.blame = false;
    let reporter = create_reporter("console-full", &opts).expect("console-full must exist");
    let result = reporter.report(&[make_clone_no_blame()], &make_stats(), &dir);
    assert!(result.is_ok(), "console-full without blame must not panic");
}
