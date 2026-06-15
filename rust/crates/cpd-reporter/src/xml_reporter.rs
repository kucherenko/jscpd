// XML reporter — PMD CPD-compatible format matching TypeScript jscpd
// Produces: <output_dir>/jscpd-report.xml

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, fragment_text, write_report_file};
use cpd_core::models::CpdClone;
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, Event};
use std::collections::HashMap;
use std::{io::Cursor, path::Path};

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_cdata(s: &str) -> String {
    s.replace("]]>", "]]]]><![CDATA[>")
}

fn write_codefragment<W: std::io::Write>(
    writer: &mut Writer<W>,
    text: &str,
) -> Result<(), ReporterError> {
    writer
        .write_event(Event::Start(BytesStart::new("codefragment")))
        .map_err(|e| ReporterError::Format(e.to_string()))?;
    writer
        .write_event(Event::CData(BytesCData::new(escape_cdata(text))))
        .map_err(|e| ReporterError::Format(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("codefragment")))
        .map_err(|e| ReporterError::Format(e.to_string()))?;
    Ok(())
}

pub struct XmlReporter {
    style: Style,
}

impl XmlReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
        }
    }
}

impl Reporter for XmlReporter {
    fn name(&self) -> &str {
        "xml"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        _ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut file_cache: HashMap<String, String> = HashMap::new();

        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        let root_start = BytesStart::new("pmd-cpd");
        writer
            .write_event(Event::Start(root_start))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        for clone in clones {
            let lines = clone
                .fragment_a
                .end
                .line
                .saturating_sub(clone.fragment_a.start.line);
            let mut dup = BytesStart::new("duplication");
            dup.push_attribute(("lines", lines.to_string().as_str()));
            writer
                .write_event(Event::Start(dup))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            let frag_text_a = fragment_text(&mut file_cache, &clone.fragment_a);

            let path_a = escape_xml(&clone.fragment_a.source_id);
            let line_a = clone.fragment_a.start.line.to_string();
            let mut file_a = BytesStart::new("file");
            file_a.push_attribute(("path", path_a.as_str()));
            file_a.push_attribute(("line", line_a.as_str()));
            writer
                .write_event(Event::Start(file_a))
                .map_err(|e| ReporterError::Format(e.to_string()))?;
            write_codefragment(&mut writer, &frag_text_a)?;
            writer
                .write_event(Event::End(BytesEnd::new("file")))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            let frag_text_b = fragment_text(&mut file_cache, &clone.fragment_b);

            let path_b = escape_xml(&clone.fragment_b.source_id);
            let line_b = clone.fragment_b.start.line.to_string();
            let mut file_b = BytesStart::new("file");
            file_b.push_attribute(("path", path_b.as_str()));
            file_b.push_attribute(("line", line_b.as_str()));
            writer
                .write_event(Event::Start(file_b))
                .map_err(|e| ReporterError::Format(e.to_string()))?;
            write_codefragment(&mut writer, &frag_text_b)?;
            writer
                .write_event(Event::End(BytesEnd::new("file")))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            write_codefragment(&mut writer, &frag_text_a)?;

            writer
                .write_event(Event::End(BytesEnd::new("duplication")))
                .map_err(|e| ReporterError::Format(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("pmd-cpd")))
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        let xml_bytes = writer.into_inner().into_inner();
        write_report_file(
            output_dir,
            "jscpd-report.xml",
            &xml_bytes,
            &self.style,
            "XML",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_empty_report_ok;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, empty_stats, tmp_dir};
    use cpd_core::models::{CpdClone, Fragment, Location};
    use std::time::Duration;

    assert_empty_report_ok!(empty_clones_produces_valid_xml, XmlReporter);

    #[test]
    fn one_clone_produces_duplication_element() {
        let dir = tmp_dir("xml");
        let file_a = dir.join("a.js");
        std::fs::write(&file_a, "hello\nworld\nfoo\nbar\n").unwrap();
        let file_a_str = file_a.to_string_lossy().into_owned();
        let loc_start = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let loc_end = Location {
            line: 3,
            column: 0,
            offset: 0,
        };
        let frag_a = Fragment {
            source_id: file_a_str.clone(),
            start: loc_start,
            end: loc_end,
            range: [0, 15],
            blame: None,
        };
        let file_b = dir.join("b.js");
        std::fs::write(&file_b, "hello\nworld\nbaz\nqux\n").unwrap();
        let file_b_str = file_b.to_string_lossy().into_owned();
        let loc_b_start = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let loc_b_end = Location {
            line: 3,
            column: 0,
            offset: 0,
        };
        let frag_b = Fragment {
            source_id: file_b_str,
            start: loc_b_start,
            end: loc_b_end,
            range: [0, 15],
            blame: None,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag_a,
            fragment_b: frag_b,
            token_count: 50,
        };
        let opts = ReporterOptions::new(dir.clone());
        let reporter = XmlReporter::new(&opts);
        let ctx = empty_ctx();
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.xml")).unwrap();
        assert!(
            content.contains("<duplication"),
            "XML must contain duplication element"
        );
        assert!(content.contains("a.js"), "XML must contain file path");
        assert!(
            content.contains("<codefragment>"),
            "XML must contain codefragment element"
        );
        assert!(
            content.contains("<![CDATA["),
            "XML must contain CDATA section"
        );
        assert!(
            !content.contains("tokens="),
            "XML must not contain tokens attribute (TS compat)"
        );
        assert!(
            !content.contains("endline="),
            "XML must not contain endline attribute (TS compat)"
        );
    }

    #[test]
    fn xml_reporter_name() {
        let opts = ReporterOptions::new(std::env::temp_dir());
        assert_eq!(XmlReporter::new(&opts).name(), "xml");
    }
}
