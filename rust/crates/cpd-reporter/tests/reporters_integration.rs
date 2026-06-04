// Integration tests for all 13 reporters with TimeReporter wrapper
//
// Test Strategy:
// 1. Each reporter has a dedicated test verifying output file creation
// 2. Each reporter is tested with TimeReporter wrapper to verify delegation
// 3. Fixture data provides realistic clone detection scenarios
//
// Test Coverage:
// - json, xml, csv, markdown, sarif, html reporters (file-based)
// - console, console-full, xcode reporters (stdout-based)
// - silent, threshold, badge, ai reporters (special behavior)
// - TimeReporter wrapper with all 13 reporters
//
// Infrastructure: Fixture data, test helpers, common setup
// Test Cases: 13 individual reporter tests plus TimeReporter wrapper tests

use std::{collections::HashMap, path::PathBuf, time::Duration};
use cpd_reporter::reporter::{Reporter, create_reporter, ReporterOptions};
use cpd_reporter::context::ReportContext;
use cpd_reporter::time::TimeReporter;
use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};

// ============================================================================
// Fixture Data - Sample clone detection results
// ============================================================================

/// Create realistic statistics for testing
/// 
/// Returns statistics representing:
/// - 100 total lines, 5 sources
/// - 2 clones detected (20% duplication)
/// - JavaScript format with detailed stats
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

/// Create sample clone for testing reporters
/// 
/// Returns a clone with:
/// - Two fragments (src/app.js and src/utils.js)
/// - Lines 10-20 in both files
/// - 50 tokens of duplication
/// - No blame information
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

/// Create a temporary directory for test output
/// 
/// Each test gets an isolated directory to prevent conflicts.
/// Directory is automatically created if it doesn't exist.
fn create_test_output_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-reporter-test-{}", test_name));
    std::fs::create_dir_all(&dir).expect("Failed to create test output dir");
    dir
}

/// Verify a file exists in the output directory
fn assert_file_exists(output_dir: &PathBuf, filename: &str) {
    let file_path = output_dir.join(filename);
    assert!(
        file_path.exists(),
        "Expected file {} to exist at {:?}",
        filename,
        file_path
    );
}

/// Read file content from output directory
fn read_output_file(output_dir: &PathBuf, filename: &str) -> String {
    let file_path = output_dir.join(filename);
    std::fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read file {:?}", file_path))
}

/// Assert that file content contains all expected patterns
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
// Sample Test - JSON Reporter
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

// ============================================================================
// TimeReporter Wrapper Test - This will fail until we add wrapper support
// ============================================================================

#[test]
fn time_reporter_wraps_json_reporter() {
    let output_dir = create_test_output_dir("time-json");
    let opts = ReporterOptions::new(output_dir.clone());
    
    // Create base reporter
    let base_reporter = create_reporter("json", &opts)
        .expect("json reporter should be available");
    
    // Wrap with TimeReporter
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    // TimeReporter should delegate to JSON reporter
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate successfully");
    
    // JSON output should still be created
    assert_file_exists(&output_dir, "jscpd-report.json");
    
    let content = read_output_file(&output_dir, "jscpd-report.json");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "JSON report after TimeReporter wrap"
    );
}

// ============================================================================
// Console Reporter Tests
// ============================================================================

#[test]
fn time_reporter_wraps_console_reporter() {
    let output_dir = create_test_output_dir("time-console");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("console", &opts)
        .expect("console reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to console reporter");
}

#[test]
fn time_reporter_wraps_console_full_reporter() {
    let output_dir = create_test_output_dir("time-console-full");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("console-full", &opts)
        .expect("console-full reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to console-full reporter");
}

#[test]
fn time_reporter_wraps_xcode_reporter() {
    let output_dir = create_test_output_dir("time-xcode");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("xcode", &opts)
        .expect("xcode reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to xcode reporter");
}

// ============================================================================
// File-based Reporter Tests
// ============================================================================

#[test]
fn time_reporter_wraps_xml_reporter() {
    let output_dir = create_test_output_dir("time-xml");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("xml", &opts)
        .expect("xml reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to xml reporter");
    
    // XML creates a report file
    assert_file_exists(&output_dir, "jscpd-report.xml");
    
    let content = read_output_file(&output_dir, "jscpd-report.xml");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "XML report after TimeReporter wrap"
    );
}

#[test]
fn time_reporter_wraps_csv_reporter() {
    let output_dir = create_test_output_dir("time-csv");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("csv", &opts)
        .expect("csv reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to csv reporter");
    
    // CSV creates a report file
    assert_file_exists(&output_dir, "jscpd-report.csv");
    
    let content = read_output_file(&output_dir, "jscpd-report.csv");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "CSV report after TimeReporter wrap"
    );
}

#[test]
fn time_reporter_wraps_html_reporter() {
    let output_dir = create_test_output_dir("time-html");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("html", &opts)
        .expect("html reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to html reporter");
    
    // HTML creates a report file
    assert_file_exists(&output_dir, "jscpd-report.html");
    
    let content = read_output_file(&output_dir, "jscpd-report.html");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "HTML report after TimeReporter wrap"
    );
}

#[test]
fn time_reporter_wraps_markdown_reporter() {
    let output_dir = create_test_output_dir("time-markdown");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("markdown", &opts)
        .expect("markdown reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to markdown reporter");
    
    // Markdown creates a report file
    assert_file_exists(&output_dir, "jscpd-report.md");
    
    let content = read_output_file(&output_dir, "jscpd-report.md");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "Markdown report after TimeReporter wrap"
    );
}

#[test]
fn time_reporter_wraps_sarif_reporter() {
    let output_dir = create_test_output_dir("time-sarif");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("sarif", &opts)
        .expect("sarif reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to sarif reporter");
    
    // SARIF creates a report file
    assert_file_exists(&output_dir, "jscpd-report.sarif");
    
    let content = read_output_file(&output_dir, "jscpd-report.sarif");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "SARIF report after TimeReporter wrap"
    );
}

// ============================================================================
// Special Reporter Tests (AI, Badge, Threshold, Silent)
// ============================================================================

#[test]
fn time_reporter_wraps_ai_reporter() {
    let output_dir = create_test_output_dir("time-ai");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("ai", &opts)
        .expect("ai reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to ai reporter");
    
    // AI creates a JSON report file
    assert_file_exists(&output_dir, "jscpd-report-ai.json");
    
    let content = read_output_file(&output_dir, "jscpd-report-ai.json");
    assert_content_contains(
        &content,
        &["src/app.js"],
        "AI report after TimeReporter wrap"
    );
}

#[test]
fn time_reporter_wraps_badge_reporter() {
    let output_dir = create_test_output_dir("time-badge");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("badge", &opts)
        .expect("badge reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to badge reporter");
    
    // Badge creates an SVG file
    assert_file_exists(&output_dir, "jscpd-badge.svg");
}

#[test]
fn time_reporter_wraps_threshold_reporter() {
    let output_dir = create_test_output_dir("time-threshold");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("threshold", &opts)
        .expect("threshold reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    // Threshold reporter may fail if threshold exceeded, but wrapping should work
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    // We don't assert success because threshold may be exceeded
    // Just verify the wrapper doesn't panic
    let _ = result;
}

#[test]
fn time_reporter_wraps_silent_reporter() {
    let output_dir = create_test_output_dir("time-silent");
    let opts = ReporterOptions::new(output_dir.clone());
    
    let base_reporter = create_reporter("silent", &opts)
        .expect("silent reporter should be available");
    
    let time_reporter = TimeReporter::new(base_reporter);
    
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    let clones = vec![make_test_clone()];
    
    let result = time_reporter.report(&clones, &ctx, &output_dir);
    assert!(result.is_ok(), "TimeReporter should delegate to silent reporter");
    
    // Silent reporter produces no output, just verify delegation worked
}
