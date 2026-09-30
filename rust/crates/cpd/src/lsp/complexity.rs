//! The complexity analysis: functions and files whose cyclomatic estimate
//! (CX, the one of `--summary` and `--complexity`) reaches a limit.

use super::findings::{Finding, Text};
use super::position::Encoding;
use super::settings::Analysis;
use cpd_core::summary::{file_complexity, span_complexity};
use cpd_reporter::rules::{COMPLEX_FILE, COMPLEX_FUNCTION};
use cpd_tokenizer::tokenizer::{Mode, tokenize};
use lsp_types::DiagnosticSeverity;

/// What the analysis measures against.
pub struct Limits {
    /// A function above this gets a finding.
    pub function: u32,
    /// A file at or above this gets one.
    pub file: u64,
}

/// The complexity findings of `text`, a file of `format`: one per function
/// over the limit, on its first line, and one on the first line of a file at
/// or over the complex-file bar.
pub fn complexity_findings(
    text: &Text,
    format: &str,
    mode: Mode,
    limits: &Limits,
    encoding: Encoding,
) -> Vec<Finding> {
    let tokens = tokenize(format, &text.text, mode);
    let mut findings = Vec::new();
    let file = file_complexity(&tokens, format);
    if file >= limits.file {
        findings.push(finding(
            text,
            0,
            COMPLEX_FILE,
            format,
            encoding,
            format!(
                "File complexity {file}; the health score counts a file from {} as complex",
                limits.file
            ),
        ));
    }
    if cpd_semantic::units::supports_units(format) {
        for map in cpd_semantic::units::extract_units(&text.text, format) {
            for unit in map.units {
                let cx = span_complexity(&tokens, &map.format, unit.start.offset, unit.end.offset);
                if cx <= u64::from(limits.function) {
                    continue;
                }
                findings.push(finding(
                    text,
                    unit.start.offset as usize,
                    COMPLEX_FUNCTION,
                    &map.format,
                    encoding,
                    format!(
                        "Complexity {cx} in {}, over the limit of {}",
                        unit.name, limits.function
                    ),
                ));
            }
        }
    }
    findings
}

/// A finding from byte `start` to the end of its line.
fn finding(
    text: &Text,
    start: usize,
    rule: &'static str,
    format: &str,
    encoding: Encoding,
    message: String,
) -> Finding {
    let position = text.index.position(&text.text, start, encoding);
    let line_end = text
        .index
        .line_start(position.line as usize + 1)
        .map_or(text.text.len(), |next| next.saturating_sub(1));
    Finding {
        analysis: Analysis::Complexity,
        rule,
        range: text
            .index
            .range(&text.text, start, line_end.max(start), encoding),
        severity: DiagnosticSeverity::INFORMATION,
        message,
        targets: Vec::new(),
        format: format.to_string(),
        first_line: position.line,
        last_line: position.line,
        unnecessary: false,
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_function_over_the_limit_gets_a_finding_on_its_first_line() {
        let code = "function small(a) {\n  return a ? 1 : 2;\n}\n\nfunction big(a, b, c) {\n  if (a && b) { return 1; }\n  if (b || c) { return 2; }\n  for (const x of a) { if (x) { return 3; } }\n  return c ?? 4;\n}\n";
        let text = Text::new(code.to_string());
        let limits = Limits {
            function: 5,
            file: 50,
        };
        let findings =
            complexity_findings(&text, "javascript", Mode::Mild, &limits, Encoding::Utf16);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, COMPLEX_FUNCTION);
        assert_eq!(findings[0].range.start.line, 4);
        assert!(
            findings[0].message.contains("in big"),
            "{}",
            findings[0].message
        );
        // The same file against a bar of 5 for files.
        let limits = Limits {
            function: 100,
            file: 5,
        };
        let findings =
            complexity_findings(&text, "javascript", Mode::Mild, &limits, Encoding::Utf16);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, COMPLEX_FILE);
        assert_eq!(findings[0].range.start.line, 0);
    }
}
