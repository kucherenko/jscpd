use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, for_each_sorted_format, write_report_file};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct CsvReporter {
    style: Style,
}

fn stat_row_to_csv(format: &str, row: &cpd_core::models::StatRow) -> String {
    format!(
        "{},{},{},{},{},{},{}",
        format,
        row.sources,
        row.lines,
        row.tokens,
        row.clones,
        row.duplicated_lines,
        row.duplicated_tokens,
    )
}

impl CsvReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
        }
    }
}

impl Reporter for CsvReporter {
    fn name(&self) -> &str {
        "csv"
    }

    fn report(
        &self,
        _clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut rows = vec![
            "Format,Files analyzed,Total lines,Total tokens,Clones found,Duplicated lines,Duplicated tokens".to_string(),
        ];

        for_each_sorted_format(&ctx.stats, |fmt, row| {
            rows.push(stat_row_to_csv(fmt, row));
        });

        let content = rows.join("\n");
        write_report_file(output_dir, "jscpd-report.csv", &content, &self.style, "CSV")?;
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

    fn run_csv_report() -> String {
        let dir = tmp_dir("csv");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CsvReporter::new(&opts);
        let stats = one_clone_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.csv")).unwrap()
    }

    #[test]
    fn csv_reporter_name() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert_eq!(CsvReporter::new(&opts).name(), "csv");
    }

    #[test]
    fn csv_has_header_and_total_row() {
        let content = run_csv_report();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].starts_with("Format,"));
        assert!(lines.iter().any(|l| l.starts_with("Total:,")));
    }

    #[test]
    fn csv_contains_format_row() {
        let content = run_csv_report();
        assert!(
            content.contains("javascript"),
            "CSV must contain format row"
        );
    }
}
