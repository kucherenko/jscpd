// ai.rs
// AI reporter: compact JSON optimized for LLM consumption.
// Output format v1: { "summary": {...}, "duplicates": [{ "format", "tokens", "a": {"file", "start", "end"}, "b": {...} }] }

use std::{fs, path::Path};
use serde_json::json;
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct AiReporter;

impl AiReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for AiReporter {
    fn name(&self) -> &str {
        "ai"
    }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report-ai.json");

        let duplicates: Vec<serde_json::Value> = clones.iter().map(|c| {
            json!({
                "format": c.format,
                "tokens": c.token_count,
                "a": {
                    "file": c.fragment_a.source_id,
                    "start": c.fragment_a.start.line,
                    "end": c.fragment_a.end.line,
                },
                "b": {
                    "file": c.fragment_b.source_id,
                    "start": c.fragment_b.start.line,
                    "end": c.fragment_b.end.line,
                },
            })
        }).collect();

        let value = json!({
            "summary": {
                "clones": ctx.stats.total.clones,
                "duplicatedLines": ctx.stats.total.duplicated_lines,
                "duplicatedTokens": ctx.stats.total.duplicated_tokens,
                "percentage": ctx.stats.total.percentage,
                "detectionDate": ctx.stats.detection_date,
            },
            "duplicates": duplicates,
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
    use std::time::Duration;
    use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-ai-test-{}",
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
                lines: 1000, tokens: 5000, sources: 10, clones: 2,
                duplicated_lines: 40, duplicated_tokens: 0,
                percentage: 12.5, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_clone() -> CpdClone {
        let loc = Location { line: 5, column: 0, offset: 0 };
        let end = Location { line: 15, column: 0, offset: 0 };
        CpdClone {
            format: "typescript".to_string(),
            fragment_a: Fragment {
                source_id: "src/a.ts".to_string(),
                start: loc.clone(),
                end: end.clone(),
                range: [0, 50],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "src/b.ts".to_string(),
                start: loc,
                end,
                range: [0, 50],
                blame: None,
            },
            token_count: 60,
        }
    }

    #[test]
    fn ai_output_is_valid_json() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = AiReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report-ai.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("summary").is_some());
        assert!(parsed.get("duplicates").is_some());
    }

    #[test]
    fn ai_output_summary_contains_stats() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = AiReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report-ai.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["summary"]["clones"], 2);
        assert_eq!(parsed["summary"]["duplicatedLines"], 40);
    }

    #[test]
    fn ai_output_duplicates_compact_format() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = AiReporter::new(&opts);
        let clone = make_clone();
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report-ai.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let dup = &parsed["duplicates"][0];
        assert_eq!(dup["format"], "typescript");
        assert_eq!(dup["tokens"], 60);
        assert_eq!(dup["a"]["file"], "src/a.ts");
        assert_eq!(dup["b"]["file"], "src/b.ts");
    }

    #[test]
    fn ai_reporter_name_is_ai() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(AiReporter::new(&opts).name(), "ai");
    }
}
