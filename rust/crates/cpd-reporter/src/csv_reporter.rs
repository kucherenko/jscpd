use std::{fs, path::Path};
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct CsvReporter {
    no_colors: bool,
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
        Self { no_colors: opts.no_colors }
    }
}

impl Reporter for CsvReporter {
    fn name(&self) -> &str {
        "csv"
    }

    fn report(&self, _clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.csv");

        let mut rows = vec![
            "Format,Files analyzed,Total lines,Total tokens,Clones found,Duplicated lines,Duplicated tokens".to_string(),
        ];

        let mut format_names: Vec<&str> = ctx.stats.formats.keys().map(|s| s.as_str()).collect();
        format_names.sort();

        for fmt in &format_names {
            if let Some(row) = ctx.stats.formats.get(*fmt) {
                rows.push(stat_row_to_csv(fmt, row));
            }
        }

        rows.push(stat_row_to_csv("Total:", &ctx.stats.total));

        let content = rows.join("\n");
        fs::write(&path, content)?;

        let path_display = path.display();
        if self.no_colors {
            println!("CSV report saved to {}", path_display);
        } else {
            println!("\x1b[32mCSV report saved to {}\x1b[39m", path_display);
        }

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
    use crate::context::ReportContext;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-csv-{}",
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
        formats.insert("javascript".to_string(), StatRow {
            lines: 100, tokens: 500, sources: 5, clones: 2,
            duplicated_lines: 20, duplicated_tokens: 100,
            percentage: 20.0, percentage_tokens: 20.0,
        });
        Statistics {
            total: StatRow {
                lines: 100, tokens: 500, sources: 5, clones: 2,
                duplicated_lines: 20, duplicated_tokens: 100,
                percentage: 20.0, percentage_tokens: 20.0,
            },
            formats,
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn csv_reporter_name() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert_eq!(CsvReporter::new(&opts).name(), "csv");
    }

    #[test]
    fn csv_has_header_and_total_row() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CsvReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].starts_with("Format,"));
        assert!(lines.iter().any(|l| l.starts_with("Total:,")));
    }

    #[test]
    fn csv_contains_format_row() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CsvReporter::new(&opts);
        let stats = make_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.csv")).unwrap();
        assert!(content.contains("javascript"), "CSV must contain format row");
    }
}