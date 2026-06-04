// XML reporter — PMD CPD-compatible format
// Produces: <output_dir>/jscpd-report.xml

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use std::{fs, io::Cursor, path::Path};
use cpd_core::models::{CpdClone, Statistics};
use crate::reporter::{Reporter, ReporterError, ReporterOptions};

pub struct XmlReporter;

impl XmlReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for XmlReporter {
    fn name(&self) -> &str {
        "xml"
    }

    fn report(&self, clones: &[CpdClone], _stats: &Statistics, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.xml");

        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        let root_start = BytesStart::new("pmd-cpd");
        writer.write_event(Event::Start(root_start))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        for clone in clones {
            let lines = clone.fragment_a.end.line.saturating_sub(clone.fragment_a.start.line) + 1;
            let mut dup = BytesStart::new("duplication");
            dup.push_attribute(("lines", lines.to_string().as_str()));
            dup.push_attribute(("tokens", clone.token_count.to_string().as_str()));
            writer.write_event(Event::Start(dup))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            let mut file_a = BytesStart::new("file");
            file_a.push_attribute(("path", clone.fragment_a.source_id.as_str()));
            file_a.push_attribute(("line", clone.fragment_a.start.line.to_string().as_str()));
            file_a.push_attribute(("endline", clone.fragment_a.end.line.to_string().as_str()));
            writer.write_event(Event::Empty(file_a))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            let mut file_b = BytesStart::new("file");
            file_b.push_attribute(("path", clone.fragment_b.source_id.as_str()));
            file_b.push_attribute(("line", clone.fragment_b.start.line.to_string().as_str()));
            file_b.push_attribute(("endline", clone.fragment_b.end.line.to_string().as_str()));
            writer.write_event(Event::Empty(file_b))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            writer.write_event(Event::End(BytesEnd::new("duplication")))
                .map_err(|e| ReporterError::Format(e.to_string()))?;
        }

        writer.write_event(Event::End(BytesEnd::new("pmd-cpd")))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        let xml_bytes = writer.into_inner().into_inner();
        fs::write(&path, xml_bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-xml-{}",
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
    fn empty_clones_produces_valid_xml() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = XmlReporter::new(&opts);
        reporter.report(&[], &empty_stats(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.xml")).unwrap();
        assert!(content.contains("<pmd-cpd"), "XML must contain root element");
        assert!(
            content.contains("</pmd-cpd>") || content.contains("<pmd-cpd/>"),
            "XML must be well-formed"
        );
    }

    #[test]
    fn one_clone_produces_duplication_element() {
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
            fragment_b: frag,
            token_count: 50,
        };
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = XmlReporter::new(&opts);
        reporter.report(&[clone], &empty_stats(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.xml")).unwrap();
        assert!(content.contains("<duplication"), "XML must contain duplication element");
        assert!(content.contains("a.js"), "XML must contain file path");
        assert!(content.contains("tokens=\"50\""), "XML must contain token count");
    }

    #[test]
    fn xml_reporter_name() {
        let opts = ReporterOptions::new(std::env::temp_dir());
        assert_eq!(XmlReporter::new(&opts).name(), "xml");
    }
}
