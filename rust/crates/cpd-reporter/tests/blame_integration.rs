use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{Reporter, ReporterOptions, create_reporter};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process,
    time::Duration,
};

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
    CpdClone::exact(
        "javascript",
        Fragment::new("src/foo.js", loc.clone(), end_loc.clone(), [0, 100])
            .with_blame(blame.clone()),
        Fragment::new("src/bar.js", loc, end_loc, [0, 100]).with_blame(blame),
        50,
    )
}

fn make_clone_no_blame() -> CpdClone {
    let loc = Location {
        line: 1,
        column: 0,
        offset: 0,
    };
    CpdClone::exact(
        "javascript",
        Fragment::new("a.js", loc.clone(), loc.clone(), [0, 10]),
        Fragment::new("b.js", loc.clone(), loc, [0, 10]),
        10,
    )
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
    let stats = make_stats();
    let ctx = ReportContext::new(&stats, Duration::ZERO);
    reporter.report(&[clone], &ctx, &dir).unwrap();
    (dir, reporter)
}

fn read_json(dir: &Path, file: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(file)).unwrap()).unwrap()
}

#[test]
fn json_reporter_attaches_blame_to_both_files() {
    let (dir, _reporter) = run_blame_reporter("json", "json", make_clone_with_blame(), true);
    let report = read_json(&dir, "jscpd-report.json");
    for side in ["firstFile", "secondFile"] {
        let blame = &report["duplicates"][0][side]["blame"];
        assert_eq!(blame["commitSha"], "deadbeef1234", "{side}");
        assert_eq!(blame["author"], "Bob Smith", "{side}");
    }
}

#[test]
fn json_reporter_leaves_blame_out_without_the_flag() {
    let (dir, _reporter) = run_blame_reporter("json", "json-off", make_clone_with_blame(), false);
    let report = read_json(&dir, "jscpd-report.json");
    let dup = &report["duplicates"][0];
    assert!(dup["firstFile"].get("blame").is_none(), "{dup}");
    assert!(dup["secondFile"].get("blame").is_none(), "{dup}");
}

#[test]
fn json_reporter_blame_none_serializes_as_absent() {
    let (dir, _reporter) = run_blame_reporter("json", "json-null", make_clone_no_blame(), true);
    let report = read_json(&dir, "jscpd-report.json");
    let first_file = &report["duplicates"][0]["firstFile"];
    assert!(
        first_file.get("blame").is_none(),
        "JSON firstFile should not contain blame field when blame is None, got: {:?}",
        first_file
    );
}

#[test]
fn sarif_reporter_blame_in_properties() {
    let (dir, _reporter) = run_blame_reporter("sarif", "sarif", make_clone_with_blame(), true);
    let sarif = read_json(&dir, "jscpd-report.sarif");
    let blame = &sarif["runs"][0]["results"][0]["properties"]["blame"];
    assert_eq!(blame["sha"], "deadbeef1234");
    assert_eq!(blame["author"], "Bob Smith");
    assert_eq!(blame["timestamp"], 1700000000);
}

#[test]
fn sarif_reporter_without_blame_data_has_no_blame_property() {
    let (dir, _reporter) = run_blame_reporter("sarif", "sarif-none", make_clone_no_blame(), true);
    let sarif = read_json(&dir, "jscpd-report.sarif");
    let result = &sarif["runs"][0]["results"][0];
    assert_eq!(result["properties"]["token_count"], 10);
    assert!(result["properties"].get("blame").is_none(), "{result}");
}

// What console-full prints with --blame is checked on real stdout in
// console_output.rs.
