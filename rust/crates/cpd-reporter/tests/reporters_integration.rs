// Integration tests for all reporters
//
// Test Strategy:
// 1. Each reporter has a dedicated test verifying output file creation
// 2. Fixture data provides realistic clone detection scenarios
//
// Test Coverage:
// - json, xml, csv, markdown, sarif, html reporters (file-based)
// - console, console-full, xcode reporters (stdout-based)
// - silent, threshold, badge, ai reporters (special behavior)

use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{Reporter, ReporterOptions, create_reporter};
use std::{collections::HashMap, path::PathBuf, time::Duration};

// ============================================================================
// Fixture Data - Sample clone detection results
// ============================================================================

fn make_test_statistics() -> Statistics {
    let mut formats = HashMap::new();
    formats.insert(
        "javascript".to_string(),
        StatRow {
            lines: 100,
            tokens: 500,
            sources: 5,
            clones: 2,
            duplicated_lines: 20,
            duplicated_tokens: 100,
            percentage: 20.0,
            percentage_tokens: 20.0,
            ..StatRow::default()
        },
    );

    Statistics {
        total: StatRow {
            lines: 100,
            tokens: 500,
            sources: 5,
            clones: 2,
            duplicated_lines: 20,
            duplicated_tokens: 100,
            percentage: 20.0,
            percentage_tokens: 20.0,
            ..StatRow::default()
        },
        formats,
        detection_date: "2026-06-04T00:00:00Z".to_string(),
    }
}

fn make_test_clone() -> CpdClone {
    let start_loc = Location {
        line: 10,
        column: 0,
        offset: 100,
    };
    let end_loc = Location {
        line: 20,
        column: 0,
        offset: 200,
    };

    CpdClone {
        format: "javascript".to_string(),
        fragment_a: Fragment {
            source_id: "src/app.js".to_string(),
            start: start_loc.clone(),
            end: end_loc.clone(),
            range: [100, 200],
            blame: None,
        },
        fragment_b: Fragment {
            source_id: "src/utils.js".to_string(),
            start: start_loc,
            end: end_loc,
            range: [100, 200],
            blame: None,
        },
        token_count: 50,
    }
}

// ============================================================================
// Test Helper Functions
// ============================================================================

fn create_test_output_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-reporter-test-{}", test_name));
    std::fs::create_dir_all(&dir).expect("Failed to create test output dir");
    dir
}

fn assert_file_exists(output_dir: &PathBuf, filename: &str) {
    let file_path = output_dir.join(filename);
    assert!(
        file_path.exists(),
        "Expected file {} to exist at {:?}",
        filename,
        file_path
    );
}

fn read_output_file(output_dir: &PathBuf, filename: &str) -> String {
    let file_path = output_dir.join(filename);
    std::fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read file {:?}", file_path))
}

fn assert_content_contains(content: &str, patterns: &[&str], description: &str) {
    for pattern in patterns {
        assert!(
            content.contains(pattern),
            "{} should contain '{}', but it doesn't. Content snippet: {}",
            description,
            pattern,
            &content[..content.len().min(200)]
        );
    }
}

fn make_test_ctx() -> ReportContext<'static> {
    let stats = Box::leak(Box::new(make_test_statistics()));
    ReportContext::new(stats, Duration::from_millis(500))
}

fn make_reporter(name: &str, output_dir: PathBuf) -> (PathBuf, Box<dyn Reporter>) {
    let opts = ReporterOptions::new(output_dir.clone());
    let reporter = create_reporter(name, &opts)
        .unwrap_or_else(|| panic!("{} reporter should be available", name));
    (output_dir, reporter)
}

fn make_stdout_reporter(name: &str) -> Box<dyn Reporter> {
    let opts = ReporterOptions::new(PathBuf::from("/tmp"));
    create_reporter(name, &opts).unwrap_or_else(|| panic!("{} reporter should be available", name))
}

fn run_file_reporter(
    name: &str,
    test_name: &str,
    filename: &str,
    patterns: &[&str],
    description: &str,
) {
    let (output_dir, reporter) = make_reporter(name, create_test_output_dir(test_name));
    let ctx = make_test_ctx();
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "{} reporter should succeed", name);

    assert_file_exists(&output_dir, filename);
    let content = read_output_file(&output_dir, filename);
    assert_content_contains(&content, patterns, description);
}

fn run_stdout_reporter(name: &str) {
    let reporter = make_stdout_reporter(name);
    let ctx = make_test_ctx();
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &PathBuf::from("/tmp"));
    assert!(result.is_ok(), "{} reporter should succeed", name);
}

// ============================================================================
// Individual Reporter Tests
// ============================================================================

#[test]
fn json_reporter_creates_output_file() {
    run_file_reporter(
        "json",
        "json",
        "jscpd-report.json",
        &["src/app.js", "src/utils.js"],
        "JSON report",
    );
}

#[test]
fn silent_reporter_succeeds() {
    run_stdout_reporter("silent");
}

#[test]
fn console_reporter_succeeds() {
    run_stdout_reporter("console");
}

#[test]
fn console_full_reporter_succeeds() {
    run_stdout_reporter("console-full");
}

#[test]
fn xcode_reporter_succeeds() {
    run_stdout_reporter("xcode");
}

#[test]
fn xml_reporter_creates_output_file() {
    run_file_reporter(
        "xml",
        "xml",
        "jscpd-report.xml",
        &["src/app.js"],
        "XML report",
    );
}

#[test]
fn csv_reporter_creates_output_file() {
    run_file_reporter(
        "csv",
        "csv",
        "jscpd-report.csv",
        &["Format", "javascript"],
        "CSV report",
    );
}

#[test]
fn html_reporter_creates_output_file() {
    run_file_reporter(
        "html",
        "html",
        "jscpd-report.html",
        &["src/app.js"],
        "HTML report",
    );
}

#[test]
fn markdown_reporter_creates_output_file() {
    run_file_reporter(
        "markdown",
        "markdown",
        "jscpd-report.md",
        &["Duplications detection", "javascript"],
        "Markdown report",
    );
}

#[test]
fn sarif_reporter_creates_output_file() {
    run_file_reporter(
        "sarif",
        "sarif",
        "jscpd-report.sarif",
        &["src/app.js"],
        "SARIF report",
    );
}

#[test]
fn ai_reporter_succeeds() {
    run_stdout_reporter("ai");
}

#[test]
fn badge_reporter_creates_output_file() {
    let (output_dir, reporter) = make_reporter("badge", create_test_output_dir("badge"));
    let ctx = make_test_ctx();
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Badge reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-badge.svg");
}

#[test]
fn threshold_reporter_runs() {
    let reporter = make_stdout_reporter("threshold");
    let ctx = make_test_ctx();
    let clones = vec![make_test_clone()];

    let _ = reporter.report(&clones, &ctx, &PathBuf::from("/tmp"));
}
