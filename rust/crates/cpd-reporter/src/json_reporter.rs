// json_reporter.rs
// JSON reporter: writes jscpd-report.json with full clone + stats payload.

use std::{fs, path::Path};
use serde_json::json;
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct JsonReporter {
    #[allow(dead_code)]
    blame: bool,
}

impl JsonReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self { blame: opts.blame }
    }
}

impl Reporter for JsonReporter {
    fn name(&self) -> &str { "json" }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.json");
        let value = json!({
            "statistics": ctx.stats,
            "duplicates": clones,
        });
        let content = serde_json::to_string_pretty(&value)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;
    use super::*;
    use std::time::Duration;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-json-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 0, tokens: 0, sources: 0, clones: 0,
                duplicated_lines: 0, duplicated_tokens: 0,
                percentage: 0.0, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_clone_with_blame() -> CpdClone {
        let loc = Location { line: 1, column: 0, offset: 0 };
        let blame = BlameEntry {
            commit_sha: "abc123".to_string(),
            author: "Alice".to_string(),
            timestamp: 1_700_000_000,
        };
        let frag_a = Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 10],
            blame: Some(blame),
        };
        let frag_b = Fragment {
            source_id: "b.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        };
        CpdClone { format: "javascript".to_string(), fragment_a: frag_a, fragment_b: frag_b, token_count: 50 }
    }

    #[test]
    fn json_output_is_valid_json() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("statistics").is_some());
        assert!(parsed.get("duplicates").is_some());
    }

    #[test]
    fn json_with_blame_includes_sha() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let clone = make_clone_with_blame();
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        assert!(content.contains("abc123"), "JSON output must contain blame SHA");
    }

    #[test]
    fn json_without_blame_has_null() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let loc = Location { line: 1, column: 0, offset: 0 };
        let frag = Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 5],
            blame: None,
        };
        let clone = CpdClone {
            format: "js".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 10,
        };
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        assert!(
            content.contains("\"blame\": null") || content.contains("\"blame\":null"),
            "Fragment with no blame must serialize as null"
        );
    }

    #[test]
    fn json_reporter_name_is_json() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(JsonReporter::new(&opts).name(), "json");
    }
}
