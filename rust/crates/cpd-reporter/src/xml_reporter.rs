// XML reporter — PMD CPD-compatible format matching TypeScript jscpd
// Produces: <output_dir>/jscpd-report.xml

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, fragment_text, write_report_file};
use cpd_core::models::CpdClone;
use quick_xml::Writer;
use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, Event};
use std::borrow::Cow;
use std::collections::HashMap;
use std::{io::Cursor, path::Path};

/// True for characters XML 1.0 allows anywhere in a document (production
/// `Char`): tab, LF, CR, and everything from U+0020 up except the
/// non-characters U+FFFE / U+FFFF. Rust strings cannot hold surrogates, so
/// those need no check.
fn is_xml_char(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

/// Replace characters XML forbids outright with U+FFFD. No escaping exists
/// for them: a NUL, an ANSI escape or a form feed in a source snippet makes
/// the report unparseable even inside CDATA (issue #375).
fn sanitize_xml_text(s: &str) -> Cow<'_, str> {
    if s.chars().all(is_xml_char) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(
            s.chars()
                .map(|ch| if is_xml_char(ch) { ch } else { '\u{FFFD}' })
                .collect(),
        )
    }
}

/// `]]>` ends a CDATA section; split it across two sections.
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
        .write_event(Event::CData(BytesCData::new(escape_cdata(
            &sanitize_xml_text(text),
        ))))
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

            // quick-xml escapes attribute values itself; escaping here too
            // turned `&` in a path into `&amp;amp;`.
            let path_a = sanitize_xml_text(&clone.fragment_a.source_id);
            let line_a = clone.fragment_a.start.line.to_string();
            let mut file_a = BytesStart::new("file");
            file_a.push_attribute(("path", path_a.as_ref()));
            file_a.push_attribute(("line", line_a.as_str()));
            writer
                .write_event(Event::Start(file_a))
                .map_err(|e| ReporterError::Format(e.to_string()))?;
            write_codefragment(&mut writer, &frag_text_a)?;
            writer
                .write_event(Event::End(BytesEnd::new("file")))
                .map_err(|e| ReporterError::Format(e.to_string()))?;

            let frag_text_b = fragment_text(&mut file_cache, &clone.fragment_b);

            let path_b = sanitize_xml_text(&clone.fragment_b.source_id);
            let line_b = clone.fragment_b.start.line.to_string();
            let mut file_b = BytesStart::new("file");
            file_b.push_attribute(("path", path_b.as_ref()));
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

    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, tmp_dir};
    use cpd_core::models::{CpdClone, Fragment, Location};

    assert_empty_report_ok!(empty_clones_produces_valid_xml, XmlReporter);

    /// Write `text` to `name` under `dir` and return a fragment covering
    /// lines `start..=end` of it.
    fn file_fragment(dir: &Path, name: &str, text: &str, start: u32, end: u32) -> Fragment {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        Fragment::new(
            path.to_string_lossy().into_owned(),
            Location::new(start, 0, 0),
            Location::new(end, 0, 0),
            [0, text.len() as u32],
        )
    }

    fn report_xml(dir: &Path, clones: &[CpdClone]) -> String {
        let reporter = XmlReporter::new(&ReporterOptions::new(dir.to_path_buf()));
        reporter.report(clones, &empty_ctx(), dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.xml")).unwrap()
    }

    /// Parse `xml` to the end with quick-xml, returning every CDATA text and
    /// every `path` attribute value (unescaped). Panics on malformed input.
    #[allow(deprecated)] // normalized_value() needs a resolver; unescape_value is enough here
    fn parse_report(xml: &str) -> (Vec<String>, Vec<String>) {
        use quick_xml::events::Event;
        let mut reader = quick_xml::Reader::from_str(xml);
        let (mut cdata, mut paths) = (Vec::new(), Vec::new());
        loop {
            match reader.read_event().expect("report must be well-formed XML") {
                Event::Eof => break,
                Event::CData(c) => cdata.push(c.into_inner().into_owned()),
                Event::Start(e) if e.name().as_ref() == "file" => {
                    let attr = e.try_get_attribute("path").unwrap().unwrap();
                    paths.push(attr.unescape_value().unwrap().into_owned());
                }
                _ => {}
            }
        }
        (cdata, paths)
    }

    #[test]
    fn one_clone_produces_duplication_element() {
        let dir = tmp_dir("xml");
        let frag_a = file_fragment(&dir, "a.js", "hello\nworld\nfoo\nbar\n", 1, 3);
        let frag_b = file_fragment(&dir, "b.js", "hello\nworld\nbaz\nqux\n", 1, 3);
        let content = report_xml(&dir, &[CpdClone::exact("javascript", frag_a, frag_b, 50)]);
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
        parse_report(&content);
    }

    #[test]
    fn xml_illegal_control_characters_are_replaced() {
        let dir = tmp_dir("xml-ctl");
        // NUL, an ANSI escape sequence and a form feed: valid UTF-8, illegal XML.
        let text = "const RESET = \"\u{1b}[0m\";\nconst nul = \"\u{0}\";\n\u{c}\nconst ok = 1;\n";
        let frag_a = file_fragment(&dir, "a.js", text, 1, 4);
        let frag_b = file_fragment(&dir, "b.js", text, 1, 4);
        let content = report_xml(&dir, &[CpdClone::exact("javascript", frag_a, frag_b, 50)]);
        assert!(
            !content
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r')),
            "no control characters may survive: {content:?}"
        );
        let (cdata, _) = parse_report(&content);
        assert!(
            cdata[0].contains("const RESET = \"\u{FFFD}[0m\""),
            "illegal characters become U+FFFD: {:?}",
            cdata[0]
        );
        assert!(cdata[0].contains("const ok = 1;"));
    }

    #[test]
    fn cdata_terminator_in_source_survives_a_round_trip() {
        let dir = tmp_dir("xml-cdata");
        let text = "const end = \"]]>\";\nconst more = ']]>]]>';\nx();\n";
        let frag_a = file_fragment(&dir, "a.js", text, 1, 3);
        let frag_b = file_fragment(&dir, "b.js", text, 1, 3);
        let content = report_xml(&dir, &[CpdClone::exact("javascript", frag_a, frag_b, 50)]);
        let (cdata, _) = parse_report(&content);
        // quick-xml yields one CData event per section; joined they must
        // reproduce the source text exactly.
        let joined: String = cdata.concat();
        assert!(joined.contains("const end = \"]]>\";"), "{joined:?}");
        assert!(joined.contains("']]>]]>'"), "{joined:?}");
    }

    #[test]
    fn path_attributes_are_escaped_exactly_once() {
        let dir = tmp_dir("xml-path");
        let text = "one\ntwo\nthree\n";
        // `<`, `>` and `"` are not legal in Windows file names, so the file
        // on disk gets a plain name and only the recorded path carries them;
        // the attribute is built from the path, not from the file.
        let mut frag_a = file_fragment(&dir, "plain-a.js", text, 1, 3);
        frag_a.source_id = dir.join("r&d <\"x\">.js").to_string_lossy().into_owned();
        let frag_b = file_fragment(&dir, "plain.js", text, 1, 3);
        let expected = frag_a.source_id.clone();
        let content = report_xml(&dir, &[CpdClone::exact("javascript", frag_a, frag_b, 50)]);
        assert!(content.contains("r&amp;d"), "{content}");
        assert!(!content.contains("&amp;amp;"), "double-escaped: {content}");
        let (_, paths) = parse_report(&content);
        assert_eq!(paths[0], expected);
    }

    #[test]
    fn sanitize_keeps_legal_text_borrowed() {
        assert!(matches!(
            sanitize_xml_text("plain\ttext\n"),
            Cow::Borrowed(_)
        ));
        assert_eq!(sanitize_xml_text("a\u{0}b\u{FFFE}c"), "a\u{FFFD}b\u{FFFD}c");
        assert_eq!(
            sanitize_xml_text("emoji \u{1F600} ok"),
            "emoji \u{1F600} ok"
        );
    }

    #[test]
    fn xml_reporter_name() {
        let opts = ReporterOptions::new(std::env::temp_dir());
        assert_eq!(XmlReporter::new(&opts).name(), "xml");
    }
}
