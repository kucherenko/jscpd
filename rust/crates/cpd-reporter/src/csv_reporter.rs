// CSV reporter — one row per clone pair
// Produces: <output_dir>/jscpd-report.csv
// Columns: format,lines,tokens,sourceId_a,startLine_a,endLine_a,sourceId_b,startLine_b,endLine_b

use std::{fs, path::Path};
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct CsvReporter;

impl CsvReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for CsvReporter {
    fn name(&self) -> &str {
        "csv"
    }

    fn report(&self, clones: &[CpdClone], _ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.csv");
        let file = fs::File::create(&path)?;

        let mut writer = csv::Writer::from_writer(file);

        writer.write_record([
            "format", "lines", "tokens",
            "sourceId_a", "startLine_a", "endLine_a",
            "sourceId_b", "startLine_b", "endLine_b",
        ]).map_err(|e| ReporterError::Format(e.to_string()))?;

        for clone in clones {
            let lines = clone.fragment_a.end.line.saturating_sub(clone.fragment_a.start.line) + 1;
            writer.write_record([
                &clone.format,
                &lines.to_string(),
                &clone.token_count.to_string(),
                &clone.fragment_a.source_id,
                &clone.fragment_a.start.line.to_string(),
                &clone.fragment_a.end.line.to_string(),
                &clone.fragment_b.source_id,
                &clone.fragment_b.start.line.to_string(),
                &clone.fragment_b.end.line.to_string(),
            ]).map_err(|e| ReporterError::Format(e.to_string()))?;
        }

        writer.flush().map_err(|e| ReporterError::Format(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;

    use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
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
    fn empty_clones_produces_header_only() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CsvReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "empty clones should produce header row only");
        assert_eq!(
            lines[0],
            "format,lines,tokens,sourceId_a,startLine_a,endLine_a,sourceId_b,startLine_b,endLine_b"
        );
    }

    #[test]
    fn one_clone_produces_two_rows() {
        let loc = Location { line: 1, column: 0, offset: 0 };
        let frag = Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 5],
            blame: None,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag.clone(),
            fragment_b: Fragment { source_id: "b.js".to_string(), ..frag },
            token_count: 42,
        };
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CsvReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "one clone should produce header + 1 data row");
        assert!(lines[1].contains("javascript"), "row must contain format");
        assert!(lines[1].contains("42"), "row must contain token count");
        assert!(lines[1].contains("a.js"), "row must contain fragment_a source");
        assert!(lines[1].contains("b.js"), "row must contain fragment_b source");
    }

    #[test]
    fn csv_reporter_name() {
        let opts = ReporterOptions::new(std::env::temp_dir());
        assert_eq!(CsvReporter::new(&opts).name(), "csv");
    }
}
