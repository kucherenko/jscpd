// Reports over files whose names and contents are hostile: markup
// characters, quotes, control characters and ANSI escapes. A file name is
// whatever the scanned repository chose, and a snippet is its code, so
// neither may break the report's syntax or smuggle live markup into it.

use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{ReporterOptions, create_reporter};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Markup, both quote kinds, a bell and an ANSI colour escape.
const HOSTILE_NAME: &str = "we<ird>&\"q'uote\u{7}\u{1b}[31m.js";

/// Code that closes the HTML `<pre>`, opens a script, ends a CDATA section
/// and carries a form feed and an ANSI escape.
const HOSTILE_CODE: &str = "let a = \"</pre><script>alert(1)</script>\";\nlet b = ']]>';\nlet c = \"\u{c}\u{1b}[2J\";\nlet d = a + b + c;\n";

fn stats() -> Statistics {
    let row = StatRow {
        lines: 8,
        tokens: 40,
        sources: 2,
        clones: 1,
        duplicated_lines: 4,
        duplicated_tokens: 20,
        percentage: 50.0,
        percentage_tokens: 50.0,
        ..StatRow::default()
    };
    Statistics {
        total: row.clone(),
        formats: HashMap::from([("javascript".to_string(), row)]),
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

struct Scan {
    dir: PathBuf,
    clone: CpdClone,
    name_a: String,
}

/// A clone between a hostile-named file and `b.js`, both holding
/// `HOSTILE_CODE`. Source ids are relative to `source_root`, as in a scan.
fn hostile_scan(label: &str) -> Scan {
    let dir = std::env::temp_dir().join(format!("cpd-hostile-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join(HOSTILE_NAME), HOSTILE_CODE).unwrap();
    std::fs::write(src.join("b.js"), HOSTILE_CODE).unwrap();
    let fragment = |name: &str| {
        let mut f = Fragment::new(
            name,
            Location::new(1, 0, 0),
            Location::new(4, 20, 0),
            [0, 20],
        );
        f.source_root = Some(src.to_string_lossy().into_owned());
        f
    };
    let clone = CpdClone::exact("javascript", fragment(HOSTILE_NAME), fragment("b.js"), 20);
    Scan {
        dir,
        clone,
        name_a: HOSTILE_NAME.to_string(),
    }
}

fn report(scan: &Scan, reporter: &str, file: &str) -> String {
    let out = scan.dir.join("report");
    let mut opts = ReporterOptions::new(out.clone());
    opts.no_colors = true;
    let stats = stats();
    let ctx = ReportContext::new(&stats, Duration::ZERO);
    create_reporter(reporter, &opts)
        .unwrap()
        .report(std::slice::from_ref(&scan.clone), &ctx, &out)
        .unwrap();
    std::fs::read_to_string(out.join(file)).unwrap()
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

// ============================================================================
// JSON and SARIF: JSON escaping keeps every character
// ============================================================================

#[test]
fn json_round_trips_hostile_names_and_code() {
    let scan = hostile_scan("json");
    let report: Value = serde_json::from_str(&report(&scan, "json", "jscpd-report.json"))
        .expect("valid JSON despite the hostile input");
    let dup = &report["duplicates"][0];
    assert_eq!(dup["firstFile"]["name"], scan.name_a.as_str());
    assert_eq!(dup["secondFile"]["name"], "b.js");
    assert_eq!(dup["fragment"], HOSTILE_CODE.trim_end());
    cleanup(&scan.dir);
}

#[test]
fn sarif_is_valid_json_naming_the_hostile_file() {
    let scan = hostile_scan("sarif");
    let sarif: Value = serde_json::from_str(&report(&scan, "sarif", "jscpd-report.sarif"))
        .expect("valid JSON despite the hostile input");
    let result = &sarif["runs"][0]["results"][0];
    let uri = result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap();
    assert!(!uri.is_empty());
    assert!(
        result["partialFingerprints"]["jscpdCloneHash/v1"].is_string(),
        "the hostile file was read for its fingerprint: {result}"
    );
    cleanup(&scan.dir);
}

/// RFC 3986: what may appear in a URI reference without percent-encoding.
fn is_uri_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c)
}

#[test]
#[ignore = "known bug: sarif.rs writes artifactLocation.uri as the raw path, without percent-encoding, so names with spaces, <, \" or control characters make an invalid SARIF uri"]
fn sarif_artifact_uris_are_valid_uri_references() {
    let scan = hostile_scan("sarif-uri");
    let sarif: Value = serde_json::from_str(&report(&scan, "sarif", "jscpd-report.sarif")).unwrap();
    let run = &sarif["runs"][0];
    let mut uris: Vec<&str> = run["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["location"]["uri"].as_str().unwrap())
        .collect();
    uris.push(
        run["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .unwrap(),
    );
    for uri in uris {
        assert!(uri.chars().all(is_uri_char), "not a valid URI: {uri:?}");
    }
    cleanup(&scan.dir);
}

#[test]
fn codeclimate_round_trips_hostile_paths() {
    let scan = hostile_scan("codeclimate");
    let issues: Value =
        serde_json::from_str(&report(&scan, "codeclimate", "gl-code-quality-report.json"))
            .expect("valid JSON despite the hostile input");
    let paths: Vec<&str> = issues
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["location"]["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&scan.name_a.as_str()), "{paths:?}");
    cleanup(&scan.dir);
}

// ============================================================================
// XML
// ============================================================================

#[test]
fn xml_stays_well_formed_with_hostile_names_and_code() {
    use quick_xml::events::Event;
    let scan = hostile_scan("xml");
    let xml = report(&scan, "xml", "jscpd-report.xml");
    let mut reader = quick_xml::Reader::from_str(&xml);
    let (mut paths, mut code) = (Vec::new(), Vec::new());
    loop {
        match reader.read_event().expect("well-formed XML") {
            Event::Eof => break,
            Event::Start(e) if e.name().as_ref() == "file" => {
                let attr = e.try_get_attribute("path").unwrap().unwrap();
                #[allow(deprecated)]
                paths.push(attr.unescape_value().unwrap().into_owned());
            }
            Event::CData(c) => code.push(c.into_inner().into_owned()),
            _ => {}
        }
    }
    // XML 1.0 has no way to write a bell or an escape character, so they are
    // replaced; the markup characters and quotes survive exactly.
    let expected_name = scan.name_a.replace(['\u{7}', '\u{1b}'], "\u{FFFD}");
    assert_eq!(paths, [expected_name.as_str(), "b.js"]);
    let expected_code = HOSTILE_CODE
        .trim_end()
        .replace(['\u{c}', '\u{1b}'], "\u{FFFD}");
    // `]]>` is split across two CDATA sections; joined back it is intact.
    assert_eq!(code.concat(), expected_code.repeat(3));
    cleanup(&scan.dir);
}

// ============================================================================
// HTML
// ============================================================================

#[test]
fn html_escapes_hostile_names_and_code() {
    let scan = hostile_scan("html");
    let html = report(&scan, "html", "jscpd-report.html");
    assert!(!html.contains("<script>alert"), "live script from the code");
    assert!(!html.contains("<ird>"), "live markup from the file name");
    assert!(html.contains("we&#60;ird&#62;&#38;") || html.contains("we&lt;ird&gt;&amp;"));
    assert!(
        html.contains("&#60;/pre&#62;&#60;script&#62;")
            || html.contains("&lt;/pre&gt;&lt;script&gt;"),
        "the code is shown, escaped"
    );
    cleanup(&scan.dir);
}
