use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, for_each_sorted_format, summary_line, write_report_file};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct MarkdownReporter {
    style: Style,
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
            style: Style::new(opts.no_colors),
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
        let total = &ctx.stats.total;
        let format_count = ctx.stats.formats.len();

        let mut md = String::new();
        md.push_str("# Copy/paste detection report\n\n");
        md.push_str("> ");
        md.push_str(&summary_line(clones.len(), total, format_count));
        md.push_str("\n\n");

        let header = "| Format | Files analyzed | Total lines | Total tokens | Clones found | Duplicated lines | Duplicated tokens |";
        let sep = "|--------|---------------|-------------|--------------|--------------|------------------|-------------------|";
        md.push_str(&format!("{}\n{}\n", header, sep));

        for_each_sorted_format(ctx.stats, |fmt, row| {
            md.push_str(&format!("{}\n", stat_row(fmt, row)));
        });
        md.push_str(&stat_row("**Total:**", total));
        md.push('\n');

        write_report_file(output_dir, "jscpd-report.md", &md, &self.style, "Markdown")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::shared::fixtures::{one_clone_stats, tmp_dir};
    use std::path::PathBuf;
    use std::time::Duration;

    fn run_markdown_report(clones: &[CpdClone]) -> (PathBuf, String) {
        let dir = tmp_dir("md");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = MarkdownReporter::new(&opts);
        let stats = one_clone_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(clones, &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.md")).unwrap();
        (dir, content)
    }

    #[test]
    fn markdown_empty_clones_returns_ok() {
        let (dir, _content) = run_markdown_report(&[]);
        assert!(dir.join("jscpd-report.md").exists());
    }

    #[test]
    fn markdown_contains_detection_date() {
        let (_dir, content) = run_markdown_report(&[]);
        assert!(content.contains("Duplications detection"));
    }

    #[test]
    fn markdown_report_has_table_header() {
        let (_dir, content) = run_markdown_report(&[]);
        assert!(content.contains("Format"));
        assert!(content.contains("Files analyzed"));
        assert!(content.contains("Clones found"));
    }

    #[test]
    fn markdown_contains_format_row() {
        let (_dir, content) = run_markdown_report(&[]);
        assert!(content.contains("javascript"));
    }
}
