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

use std::{collections::HashMap, path::PathBuf, time::Duration};
use cpd_reporter::reporter::{create_reporter, ReporterOptions};
use cpd_reporter::context::ReportContext;
use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};

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

// ============================================================================
// Individual Reporter Tests
// ============================================================================

#[test]
fn json_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("json");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("json", &opts)
        .expect("json reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(100));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "JSON reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.json");

    let content = read_output_file(&output_dir, "jscpd-report.json");
    assert_content_contains(
        &content,
        &["src/app.js", "src/utils.js"],
        "JSON report"
    );
}

#[test]
fn silent_reporter_succeeds() {
    let output_dir = create_test_output_dir("silent");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("silent", &opts)
        .expect("silent reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Silent reporter should succeed");
}

#[test]
fn console_reporter_succeeds() {
    let output_dir = create_test_output_dir("console");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("console", &opts)
        .expect("console reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Console reporter should succeed");
}

#[test]
fn console_full_reporter_succeeds() {
    let output_dir = create_test_output_dir("console-full");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("console-full", &opts)
        .expect("console-full reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Console-full reporter should succeed");
}

#[test]
fn xcode_reporter_succeeds() {
    let output_dir = create_test_output_dir("xcode");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("xcode", &opts)
        .expect("xcode reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Xcode reporter should succeed");
}

#[test]
fn xml_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("xml");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("xml", &opts)
        .expect("xml reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "XML reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.xml");

    let content = read_output_file(&output_dir, "jscpd-report.xml");
    assert_content_contains(&content, &["src/app.js"], "XML report");
}

#[test]
fn csv_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("csv");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("csv", &opts)
        .expect("csv reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "CSV reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.csv");

    let content = read_output_file(&output_dir, "jscpd-report.csv");
    assert_content_contains(&content, &["Format", "javascript"], "CSV report");
}

#[test]
fn html_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("html");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("html", &opts)
        .expect("html reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "HTML reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.html");

    let content = read_output_file(&output_dir, "jscpd-report.html");
    assert_content_contains(&content, &["src/app.js"], "HTML report");
}

#[test]
fn markdown_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("markdown");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("markdown", &opts)
        .expect("markdown reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Markdown reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.md");

    let content = read_output_file(&output_dir, "jscpd-report.md");
    assert_content_contains(&content, &["Duplications detection", "javascript"], "Markdown report");
}

#[test]
fn sarif_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("sarif");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("sarif", &opts)
        .expect("sarif reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "SARIF reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-report.sarif");

    let content = read_output_file(&output_dir, "jscpd-report.sarif");
    assert_content_contains(&content, &["src/app.js"], "SARIF report");
}

#[test]
fn ai_reporter_succeeds() {
    let output_dir = create_test_output_dir("ai");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("ai", &opts)
        .expect("ai reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "AI reporter should succeed");
}

#[test]
fn badge_reporter_creates_output_file() {
    let output_dir = create_test_output_dir("badge");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("badge", &opts)
        .expect("badge reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "Badge reporter should succeed");

    assert_file_exists(&output_dir, "jscpd-badge.svg");
}

#[test]
fn threshold_reporter_runs() {
    let output_dir = create_test_output_dir("threshold");
    let opts = ReporterOptions::new(output_dir.clone());

    let reporter = create_reporter("threshold", &opts)
        .expect("threshold reporter should be available");

    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];

    let result = reporter.report(&clones, &ctx, &output_dir);
    let _ = result;
}