use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use std::{fs, path::Path};

pub struct MarkdownReporter {
    no_colors: bool,
}

fn stat_row(format: &str, row: &cpd_core::models::StatRow) -> String {
    format!(
        "| {} | {} | {} | {} | {} | {} ({:.2}%) | {} ({:.2}%) |",
        format,
        row.sources,
        row.lines,
        row.tokens,
        row.clones,
        row.duplicated_lines,
        row.percentage,
        row.duplicated_tokens,
        row.percentage_tokens,
    )
}

impl MarkdownReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            no_colors: opts.no_colors,
        }
    }
}

impl Reporter for MarkdownReporter {
    fn name(&self) -> &str {
        "markdown"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.md");

        let total = &ctx.stats.total;
        let format_count = ctx.stats.formats.len();

        let mut md = String::new();
        md.push_str("# Copy/paste detection report\n\n");
        md.push_str(&format!(
            "> Duplications detection: Found {} exact clones with {}({:.2}%) duplicated lines in {} ({} formats) files.\n\n",
            clones.len(),
            total.duplicated_lines,
            total.percentage,
            total.sources,
            format_count,
        ));

        let header = "| Format | Files analyzed | Total lines | Total tokens | Clones found | Duplicated lines | Duplicated tokens |";
        let sep = "|--------|---------------|-------------|--------------|--------------|------------------|-------------------|";
        md.push_str(&format!("{}\n{}\n", header, sep));

        let mut format_names: Vec<&str> = ctx.stats.formats.keys().map(|s| s.as_str()).collect();
        format_names.sort();
        for fmt in &format_names {
            if let Some(row) = ctx.stats.formats.get(*fmt) {
                md.push_str(&format!("{}\n", stat_row(fmt, row)));
            }
        }
        md.push_str(&format!("{}\n", stat_row("**Total:**", total)));

        fs::write(&path, md)?;

        let path_display = path.display();
        if self.no_colors {
            println!("Markdown report saved to {}", path_display);
        } else {
            println!("\x1b[32mMarkdown report saved to {}\x1b[39m", path_display);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use cpd_core::models::{StatRow, Statistics};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

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

    fn make_stats() -> Statistics {
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
                new_duplicated_lines: 0,
                new_clones: 0,
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
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats,
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn markdown_empty_clones_returns_ok() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        let result = reporter.report(&[], &ctx, &dir);
        assert!(result.is_ok());
    }

    #[test]
    fn markdown_contains_detection_date() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        assert!(content.contains("Duplications detection"));
    }

    #[test]
    fn markdown_report_has_table_header() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        assert!(content.contains("Format"));
        assert!(content.contains("Files analyzed"));
        assert!(content.contains("Clones found"));
    }

    #[test]
    fn markdown_contains_format_row() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        assert!(content.contains("javascript"));
    }
}
