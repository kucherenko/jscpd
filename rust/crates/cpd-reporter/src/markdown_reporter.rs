// cpd-reporter: Markdown reporter — writes jscpd-report.md

use std::{fs, path::Path};
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct MarkdownReporter;

impl MarkdownReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for MarkdownReporter {
    fn name(&self) -> &str {
        "markdown"
    }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.md");

        let mut md = String::new();
        md.push_str("# CPD Duplication Report\n\n");
        md.push_str(&format!("Detection date: {}\n\n", ctx.stats.detection_date));
        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Total files | {} |\n", ctx.stats.total.sources));
        md.push_str(&format!("| Clones | {} |\n", clones.len()));
        md.push_str(&format!(
            "| Duplicated lines | {} ({:.1}%) |\n",
            ctx.stats.total.duplicated_lines, ctx.stats.total.percentage
        ));
        md.push_str(&format!(
            "| Duplicated tokens | {} ({:.1}%) |\n",
            ctx.stats.total.duplicated_tokens, ctx.stats.total.percentage_tokens
        ));

        if !clones.is_empty() {
            md.push_str("\n## Duplicates\n\n");
            md.push_str("| File A | Lines | File B | Lines | Tokens |\n");
            md.push_str("|--------|-------|--------|-------|--------|\n");
            for clone in clones {
                md.push_str(&format!(
                    "| {} | {}-{} | {} | {}-{} | {} |\n",
                    clone.fragment_a.source_id,
                    clone.fragment_a.start.line,
                    clone.fragment_a.end.line,
                    clone.fragment_b.source_id,
                    clone.fragment_b.start.line,
                    clone.fragment_b.end.line,
                    clone.token_count,
                ));
            }
        }

        fs::write(&path, md)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;

    use cpd_core::models::{Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-md-{}",
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
                lines: 0,
                tokens: 0,
                sources: 0,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn markdown_report_has_header_row() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        assert!(content.contains("| Metric | Value |"), "must have summary table header");
    }

    #[test]
    fn markdown_empty_clones_returns_ok() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[], &ctx, &dir);
        assert!(result.is_ok());
    }

    #[test]
    fn markdown_contains_detection_date() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        assert!(content.contains("2026-01-01T00:00:00Z"));
    }
}
