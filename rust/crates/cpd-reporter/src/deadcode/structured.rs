//! Machine-readable dead-code reports: JSON, XML and CSV.

use super::{DeadCodeContext, DeadCodeReporter, reasons_clause, severity};
use crate::reporter::{ReporterError, ReporterOptions};
use crate::shared::{Style, write_report_file};
use cpd_core::deadcode::{Finding, Report, Stats};
use std::path::Path;

/// The full report as JSON — the format every other tool reads.
///
/// The shape is additive: new keys may appear, existing ones keep their
/// meaning, so a consumer that pins to `findings[].path` keeps working.
pub struct JsonReporter {
    style: Style,
}

impl JsonReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for JsonReporter {
    fn name(&self) -> &str {
        "json"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let report = Report {
            findings: findings.to_vec(),
            statistics: ctx.stats.clone(),
        };
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        write_report_file(
            output_dir,
            "basta-report.json",
            json,
            &self.style,
            "Dead code JSON",
        )?;
        Ok(())
    }
}

/// A Checkstyle-shaped XML report, which most CI systems already ingest.
pub struct XmlReporter {
    style: Style,
}

impl XmlReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for XmlReporter {
    fn name(&self) -> &str {
        "xml"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<checkstyle version=\"4.3\">\n");
        for (path, group) in super::group_by_path(findings) {
            xml.push_str(&format!("  <file name=\"{}\">\n", escape_xml(path)));
            for finding in group {
                xml.push_str(&format!(
                    "    <error line=\"{}\" column=\"{}\" severity=\"{}\" message=\"{}\" source=\"basta.{}\"/>\n",
                    finding.start.line,
                    finding.start.column + 1,
                    severity(finding).sarif(),
                    escape_xml(&finding.message),
                    finding.category.as_str(),
                ));
            }
            xml.push_str("  </file>\n");
        }
        xml.push_str("</checkstyle>\n");
        let _ = ctx;
        write_report_file(
            output_dir,
            "basta-report.xml",
            xml,
            &self.style,
            "Dead code XML",
        )?;
        Ok(())
    }
}

/// One row per finding, for spreadsheets and ad-hoc analysis.
pub struct CsvReporter {
    style: Style,
}

impl CsvReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for CsvReporter {
    fn name(&self) -> &str {
        "csv"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer
            .write_record([
                "category",
                "path",
                "line",
                "column",
                "name",
                "kind",
                "parent",
                "lines",
                "confidence",
                "level",
                "reasons",
                "message",
            ])
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        for finding in findings {
            writer
                .write_record([
                    finding.category.as_str(),
                    &finding.path,
                    &finding.start.line.to_string(),
                    &(finding.start.column + 1).to_string(),
                    &finding.name,
                    finding.symbol_kind.map(|k| k.noun()).unwrap_or("file"),
                    finding.parent.as_deref().unwrap_or(""),
                    &finding.lines.to_string(),
                    &finding.confidence.to_string(),
                    finding.level().as_str(),
                    &reasons_clause(finding),
                    &finding.message,
                ])
                .map_err(|e| ReporterError::Format(e.to_string()))?;
        }
        let bytes = writer
            .into_inner()
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        let _ = ctx;
        write_report_file(
            output_dir,
            "basta-report.csv",
            bytes,
            &self.style,
            "Dead code CSV",
        )?;
        Ok(())
    }
}

/// Summary counters every text format needs.
pub(crate) fn summary_line(stats: &Stats) -> String {
    format!(
        "{} findings across {} files ({:.1}% of {} lines)",
        stats.total_findings(),
        stats.files,
        stats.percentage,
        stats.total_lines
    )
}

pub(crate) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters are not legal in XML 1.0 at all, so they are
            // dropped rather than escaped into something a parser rejects.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;
    use cpd_core::deadcode::Category;
    use std::path::PathBuf;
    use std::time::Duration;

    fn output_dir(name: &str) -> PathBuf {
        fixtures::unique_dir(name)
    }

    fn write(name: &str, reporter: &dyn DeadCodeReporter) -> String {
        let dir = output_dir(name);
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::from_millis(5));
        reporter
            .report(&fixtures::findings(), &ctx, &dir)
            .expect("report");
        let file = std::fs::read_dir(&dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let content = std::fs::read_to_string(file).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        content
    }

    #[test]
    fn json_round_trips_back_into_a_report() {
        let options = ReporterOptions::new(PathBuf::from("/tmp"));
        let json = write("json", &JsonReporter::new(&options));
        let parsed: Report = serde_json::from_str(&json).expect("valid report JSON");
        assert_eq!(parsed.findings.len(), 3);
        assert_eq!(parsed.statistics.files, 12);
        assert_eq!(parsed.findings[0].category, Category::UnusedFile);
    }

    #[test]
    fn xml_is_well_formed_checkstyle() {
        let options = ReporterOptions::new(PathBuf::from("/tmp"));
        let xml = write("xml", &XmlReporter::new(&options));
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<checkstyle"));
        assert!(xml.contains("source=\"basta.unused-export\""), "{xml}");
        assert_eq!(
            xml.matches("<file ").count(),
            3,
            "one file element per path: {xml}"
        );
    }

    #[test]
    fn csv_has_one_header_and_one_row_per_finding() {
        let options = ReporterOptions::new(PathBuf::from("/tmp"));
        let csv = write("csv", &CsvReporter::new(&options));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 4, "{csv}");
        assert!(lines[0].starts_with("category,path,line,column,name"));
        assert!(lines.iter().skip(1).any(|l| l.contains("unused-import")));
    }

    #[test]
    fn xml_escaping_covers_the_characters_that_break_parsers() {
        assert_eq!(
            escape_xml("a & b < c > d \" e ' f"),
            "a &amp; b &lt; c &gt; d &quot; e &apos; f"
        );
        assert_eq!(
            escape_xml("keep\ttabs\nand\rnewlines\u{0}but not nulls"),
            "keep\ttabs\nand\rnewlinesbut not nulls"
        );
    }

    #[test]
    fn an_empty_run_still_writes_a_valid_file() {
        let options = ReporterOptions::new(PathBuf::from("/tmp"));
        let dir = output_dir("empty");
        let stats = cpd_core::deadcode::Stats::default();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        for reporter in [
            Box::new(JsonReporter::new(&options)) as Box<dyn DeadCodeReporter>,
            Box::new(XmlReporter::new(&options)),
            Box::new(CsvReporter::new(&options)),
        ] {
            assert!(reporter.report(&[], &ctx, &dir).is_ok());
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_summary_line_reads_as_a_sentence_fragment() {
        assert_eq!(
            summary_line(&fixtures::stats()),
            "3 findings across 12 files (3.4% of 1000 lines)"
        );
    }
}
