// Integration tests for the file-writing reporters: each report is parsed
// the way its consumer would parse it (JSON, XML, CSV, SARIF, OpenMetrics)
// and checked for the facts it must carry: the two locations of each clone,
// its size, and the run statistics.
//
// What the stdout reporters print is checked in console_output.rs.

use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{ReporterError, ReporterOptions, create_reporter};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

// ============================================================================
// Fixture Data
// ============================================================================

fn make_test_statistics() -> Statistics {
    let row = StatRow {
        lines: 100,
        tokens: 500,
        sources: 5,
        clones: 2,
        duplicated_lines: 20,
        duplicated_tokens: 100,
        percentage: 20.0,
        percentage_tokens: 20.0,
        ..StatRow::default()
    };
    Statistics {
        total: row.clone(),
        formats: HashMap::from([("javascript".to_string(), row)]),
        detection_date: "2026-06-04T00:00:00Z".to_string(),
    }
}

/// A clone of `src/app.js:10-20` at `src/utils.js:30-40`, whose files are not
/// on disk.
fn make_test_clone() -> CpdClone {
    let fragment = |id: &str, start: u32| Fragment {
        source_id: id.to_string(),
        source_root: None,
        start: Location::new(start, 0, 100),
        end: Location::new(start + 10, 4, 200),
        range: [100, 200],
        blame: None,
    };
    CpdClone::exact(
        "javascript",
        fragment("src/app.js", 10),
        fragment("src/utils.js", 30),
        50,
    )
}

const SNIPPET_MARKER: &str = "test_snippet_marker_xyzzy";

/// The duplicated function, preceded in a.js by two lines that are not part
/// of the clone.
fn duplicated_code() -> String {
    format!("function hello() {{\n  console.log(\"{SNIPPET_MARKER}\");\n  return 42;\n}}\n")
}

/// A clone backed by real files: a.js lines 3-6 and b.js lines 1-4 hold the
/// same function.
fn make_test_clone_with_real_files(dir: &Path) -> CpdClone {
    let code = duplicated_code();
    let file_a = dir.join("a.js");
    let file_b = dir.join("b.js");
    std::fs::write(
        &file_a,
        format!("const unrelated = 1;\nlet other = 2;\n{code}"),
    )
    .unwrap();
    std::fs::write(&file_b, &code).unwrap();

    let fragment = |file: &Path, start: u32| {
        Fragment::new(
            file.to_string_lossy().into_owned(),
            Location::new(start, 0, 0),
            Location::new(start + 3, 1, 0),
            [0, 100],
        )
    };
    CpdClone::exact("javascript", fragment(&file_a, 3), fragment(&file_b, 1), 50)
}

// ============================================================================
// Helpers
// ============================================================================

fn create_test_output_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cpd-reporter-test-{}-{}",
        test_name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("Failed to create test output dir");
    dir
}

/// Run reporter `name` over `clones` into a fresh directory and return the
/// content of `filename`.
fn run_file_reporter_with(
    name: &str,
    filename: &str,
    clones: &[CpdClone],
    configure: impl FnOnce(&mut ReporterOptions),
) -> String {
    // Tests run in parallel: every run gets a directory of its own.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = create_test_output_dir(&format!("{name}-run{run}"));
    run_in(&dir, name, filename, clones, configure)
}

fn run_in(
    dir: &Path,
    name: &str,
    filename: &str,
    clones: &[CpdClone],
    configure: impl FnOnce(&mut ReporterOptions),
) -> String {
    let mut opts = ReporterOptions::new(dir.to_path_buf());
    opts.no_colors = true;
    configure(&mut opts);
    let reporter = create_reporter(name, &opts)
        .unwrap_or_else(|| panic!("{name} reporter should be available"));
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::from_millis(500));
    reporter
        .report(clones, &ctx, dir)
        .unwrap_or_else(|e| panic!("{name} reporter failed: {e}"));
    let path = dir.join(filename);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{name} wrote no {path:?}"))
}

fn run_file_reporter(name: &str, filename: &str) -> String {
    run_file_reporter_with(name, filename, &[make_test_clone()], |_| {})
}

/// `(element name, attributes)` of every start tag in `xml`, plus every
/// CDATA text. Panics on malformed XML.
type Tag = (String, HashMap<String, String>);
fn parse_xml(xml: &str) -> (Vec<Tag>, Vec<String>) {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut tags, mut cdata) = (Vec::new(), Vec::new());
    loop {
        match reader.read_event().expect("well-formed XML") {
            Event::Start(e) | Event::Empty(e) => {
                let attrs = e
                    .attributes()
                    .map(|a| {
                        let a = a.unwrap();
                        #[allow(deprecated)]
                        let value = a.unescape_value().unwrap().into_owned();
                        (a.key.as_ref().to_string(), value)
                    })
                    .collect();
                tags.push((e.name().as_ref().to_string(), attrs));
            }
            Event::CData(t) => cdata.push(t.into_inner().into_owned()),
            Event::Eof => break,
            _ => {}
        }
    }
    (tags, cdata)
}

// ============================================================================
// JSON
// ============================================================================

#[test]
fn json_report_holds_both_locations_and_the_statistics() {
    let report: Value =
        serde_json::from_str(&run_file_reporter("json", "jscpd-report.json")).unwrap();
    let dups = report["duplicates"].as_array().unwrap();
    assert_eq!(dups.len(), 1);
    let dup = &dups[0];
    assert_eq!(dup["format"], "javascript");
    assert_eq!(dup["tokens"], 50);
    assert_eq!(dup["firstFile"]["name"], "src/app.js");
    assert_eq!(dup["firstFile"]["start"], 10);
    assert_eq!(dup["firstFile"]["end"], 20);
    assert_eq!(dup["secondFile"]["name"], "src/utils.js");
    assert_eq!(dup["secondFile"]["start"], 30);
    assert_eq!(dup["secondFile"]["end"], 40);
    let total = &report["statistics"]["total"];
    assert_eq!(total["duplicatedLines"], 20);
    assert_eq!(total["percentage"], 20.0);
    assert_eq!(report["statistics"]["formats"]["javascript"]["sources"], 5);
}

#[test]
fn json_report_fragment_is_the_duplicated_code() {
    let dir = create_test_output_dir("json-snippet");
    let clone = make_test_clone_with_real_files(&dir);
    let report: Value =
        serde_json::from_str(&run_in(&dir, "json", "jscpd-report.json", &[clone], |_| {})).unwrap();
    assert_eq!(
        report["duplicates"][0]["fragment"].as_str().unwrap(),
        duplicated_code().trim_end(),
        "lines 3-6 of a.js, without the lines around them"
    );
}

// ============================================================================
// XML
// ============================================================================

#[test]
fn xml_report_is_pmd_cpd_with_both_files() {
    let (tags, _) = parse_xml(&run_file_reporter("xml", "jscpd-report.xml"));
    let names: Vec<&str> = tags.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "pmd-cpd",
            "duplication",
            "file",
            "codefragment",
            "file",
            "codefragment",
            "codefragment"
        ]
    );
    assert_eq!(tags[1].1["lines"], "10");
    assert_eq!(tags[2].1["path"], "src/app.js");
    assert_eq!(tags[2].1["line"], "10");
    assert_eq!(tags[4].1["path"], "src/utils.js");
    assert_eq!(tags[4].1["line"], "30");
}

#[test]
fn xml_report_code_fragments_hold_the_duplicated_code() {
    let dir = create_test_output_dir("xml-snippet");
    let clone = make_test_clone_with_real_files(&dir);
    let (_, cdata) = parse_xml(&run_in(&dir, "xml", "jscpd-report.xml", &[clone], |_| {}));
    let code = duplicated_code();
    assert_eq!(cdata, [code.trim_end(); 3]);
}

// ============================================================================
// CSV
// ============================================================================

#[test]
fn csv_report_has_one_row_per_format_and_a_total() {
    let content = run_file_reporter("csv", "jscpd-report.csv");
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_string)
        .collect();
    assert_eq!(headers[0], "Format");
    let rows: Vec<Vec<String>> = reader
        .records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect();
    let first_cells: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(first_cells, ["javascript", "Total:"]);
    for row in &rows {
        assert_eq!(row.len(), headers.len(), "{row:?}");
        assert!(row.iter().any(|c| c == "5"), "5 sources: {row:?}");
        assert!(row.iter().any(|c| c.starts_with("20")), "20 lines: {row:?}");
    }
}

// ============================================================================
// HTML
// ============================================================================

#[test]
fn html_report_lists_the_clone_with_its_locations() {
    let content = run_file_reporter("html", "jscpd-report.html");
    assert!(content.contains("<html"), "an HTML page");
    assert!(
        content.contains("src/app.js (Line 10:1 - Line 20:5)"),
        "first location"
    );
    assert!(
        content.contains("src/utils.js (Line 30:1 - Line 40:5)"),
        "second location"
    );
    assert!(
        content.contains("20 (20.00%)"),
        "duplicated lines and share"
    );
}

#[test]
fn html_report_shows_the_duplicated_code() {
    let dir = create_test_output_dir("html-snippet");
    let clone = make_test_clone_with_real_files(&dir);
    let content = run_in(&dir, "html", "jscpd-report.html", &[clone], |_| {});
    // Quotes in the code are HTML-escaped; the marker is plain text.
    assert!(content.contains(SNIPPET_MARKER));
    assert!(content.contains("return 42;"));
    assert!(!content.contains("unrelated"), "only the clone's lines");
}

// ============================================================================
// Markdown
// ============================================================================

#[test]
fn markdown_report_summarises_and_tabulates_the_run() {
    let content = run_file_reporter("markdown", "jscpd-report.md");
    assert!(content.starts_with("# Copy/paste detection report"));
    assert!(
        content.contains(
            "Found 1 exact clones with 20(20.00%) duplicated lines in 5 (1 formats) files."
        ),
        "{content}"
    );
    let row = content
        .lines()
        .find(|l| l.starts_with('|') && l.contains("javascript"))
        .unwrap_or_else(|| panic!("no javascript row in\n{content}"));
    let cells: Vec<&str> = row
        .split('|')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect();
    assert_eq!(cells[0], "javascript");
    assert!(cells.contains(&"5"), "{cells:?}");
}

// ============================================================================
// SARIF
// ============================================================================

#[test]
fn sarif_report_points_at_both_fragments() {
    let sarif: Value =
        serde_json::from_str(&run_file_reporter("sarif", "jscpd-report.sarif")).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "jscpd");
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result["level"], "warning");
    let primary = &result["locations"][0]["physicalLocation"];
    assert_eq!(primary["artifactLocation"]["uri"], "src/app.js");
    assert_eq!(primary["region"]["startLine"], 10);
    assert_eq!(primary["region"]["endLine"], 20);
    let related = &result["relatedLocations"][0]["physicalLocation"];
    assert_eq!(related["artifactLocation"]["uri"], "src/utils.js");
    assert_eq!(related["region"]["startLine"], 30);
    // The rule the result names is one the driver declares.
    let rule_ids: Vec<&str> = run["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(rule_ids.contains(&result["ruleId"].as_str().unwrap()));
    // Each file is one artifact, indexed from the locations.
    let artifacts = run["artifacts"].as_array().unwrap();
    let index = primary["artifactLocation"]["index"].as_u64().unwrap() as usize;
    assert_eq!(artifacts[index]["location"]["uri"], "src/app.js");
}

#[test]
fn sarif_results_turn_into_errors_over_the_threshold() {
    let sarif: Value = serde_json::from_str(&run_file_reporter_with(
        "sarif",
        "jscpd-report.sarif",
        &[make_test_clone()],
        |opts| opts.threshold = Some(10.0),
    ))
    .unwrap();
    assert_eq!(sarif["runs"][0]["results"][0]["level"], "error");
}

// ============================================================================
// OpenMetrics, badge
// ============================================================================

/// The value of the first sample of `metric` whose labels contain `labels`.
fn metric(content: &str, metric: &str, labels: &str) -> f64 {
    content
        .lines()
        .filter(|l| !l.starts_with('#'))
        .find(|l| l.split(['{', ' ']).next() == Some(metric) && l.contains(labels))
        .and_then(|l| l.rsplit(' ').next())
        .unwrap_or_else(|| panic!("no {metric}{{{labels}}} sample in\n{content}"))
        .parse()
        .unwrap()
}

#[test]
fn openmetrics_report_exposes_the_statistics() {
    let content = run_file_reporter("openmetrics", "jscpd-metrics.txt");
    assert_eq!(
        content.lines().last(),
        Some("# EOF"),
        "OpenMetrics ends with # EOF"
    );
    assert_eq!(
        metric(&content, "jscpd_duplicated_lines", "format=\"javascript\""),
        20.0
    );
    assert_eq!(
        metric(&content, "jscpd_clones_found", "format=\"javascript\""),
        2.0
    );
    assert_eq!(
        metric(
            &content,
            "jscpd_duplicated_lines_percent",
            "format=\"javascript\""
        ),
        20.0
    );
}

#[test]
fn badge_shows_the_duplicated_share() {
    let svg = run_file_reporter("badge", "jscpd-badge.svg");
    assert!(svg.trim_start().starts_with("<svg"), "{svg}");
    assert!(svg.contains("20.0%"), "{svg}");
}

// ============================================================================
// Threshold
// ============================================================================

#[test]
fn threshold_fails_the_run_only_when_the_share_is_above_it() {
    let stats = make_test_statistics();
    let ctx = ReportContext::new(&stats, Duration::ZERO);
    let check = |threshold: f64| {
        let mut opts = ReporterOptions::new(std::env::temp_dir());
        opts.threshold = Some(threshold);
        create_reporter("threshold", &opts).unwrap().report(
            &[make_test_clone()],
            &ctx,
            &std::env::temp_dir(),
        )
    };
    match check(10.0) {
        Err(ReporterError::ThresholdExceeded { actual, threshold }) => {
            assert_eq!(actual, 20.0);
            assert_eq!(threshold, 10.0);
        }
        other => panic!("20% is over a 10% threshold: {other:?}"),
    }
    assert!(check(20.0).is_ok(), "equal to the threshold passes");
    assert!(check(30.0).is_ok());
}

// ============================================================================
// --report-name (#1015)
// ============================================================================

/// Every reporter that writes a `jscpd-report.*` file must honour the
/// configured base name, so linter aggregators running in parallel can point
/// each tool at its own file instead of racing on `jscpd-report.json`.
#[test]
fn report_name_renames_every_jscpd_report_file() {
    for (name, extension) in [
        ("json", "json"),
        ("xml", "xml"),
        ("csv", "csv"),
        ("html", "html"),
        ("markdown", "md"),
        ("sarif", "sarif"),
    ] {
        let output_dir = create_test_output_dir(&format!("report-name-{}", name));
        let mut opts = ReporterOptions::new(output_dir.clone());
        opts.report_name = "megalinter-jscpd".to_string();
        let reporter = create_reporter(name, &opts).unwrap();

        reporter
            .report(&[make_test_clone()], &make_test_ctx(), &output_dir)
            .unwrap_or_else(|e| panic!("{} reporter should succeed: {:?}", name, e));

        assert_file_exists(&output_dir, &format!("megalinter-jscpd.{}", extension));
        assert!(
            !output_dir
                .join(format!("jscpd-report.{}", extension))
                .exists(),
            "{} reporter still wrote the default name",
            name
        );
    }
}

/// The default has to stay byte-identical, since existing pipelines and the
/// aggregators we are trying to unblock all read `jscpd-report.*` today.
#[test]
fn report_name_defaults_to_jscpd_report() {
    let output_dir = create_test_output_dir("report-name-default");
    let opts = ReporterOptions::new(output_dir.clone());
    assert_eq!(opts.report_name, "jscpd-report");

    let reporter = create_reporter("json", &opts).unwrap();
    reporter
        .report(&[make_test_clone()], &make_test_ctx(), &output_dir)
        .expect("json reporter must succeed");

    assert_file_exists(&output_dir, "jscpd-report.json");
}
