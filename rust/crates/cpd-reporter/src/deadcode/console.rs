//! Terminal output for a dead-code run.
//!
//! Findings are grouped by category rather than by file, because that is the
//! order a reader acts in: delete the unused files first and most of the rest
//! of the report disappears with them.

use super::{
    DeadCodeContext, DeadCodeReporter, group_by_category, location, reasons_clause, resolve_path,
};
use crate::reporter::{ReporterError, ReporterOptions};
use crate::shared::{Style, extract_lines};
use cpd_core::deadcode::{ConfidenceLevel, Finding};
use std::collections::HashMap;
use std::path::Path;

pub struct ConsoleReporter {
    style: Style,
}

impl ConsoleReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for ConsoleReporter {
    fn name(&self) -> &str {
        "console"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        print_groups(findings, &self.style, |_| {});
        print_trailer(findings, ctx, &self.style);
        Ok(())
    }
}

/// The console report with the source of each finding printed beneath it.
pub struct ConsoleFullReporter {
    style: Style,
}

impl ConsoleFullReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

/// How many unparsed files the trailer names before summarising the rest.
const UNPARSED_LISTED: usize = 10;

/// How many lines of a declaration the console shows before truncating. Long
/// enough to recognise what is being deleted, short enough that a report with
/// fifty findings still scrolls.
const SNIPPET_LINES: u32 = 10;

impl DeadCodeReporter for ConsoleFullReporter {
    fn name(&self) -> &str {
        "console-full"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut cache: HashMap<String, String> = HashMap::new();
        let style = &self.style;
        let roots = ctx.roots;
        print_groups(findings, style, |finding| {
            // A whole-file finding has no snippet worth printing: the file is
            // the snippet.
            if finding.name.is_empty() {
                return;
            }
            let Some(content) = read_source(&mut cache, &finding.path, roots) else {
                return;
            };
            let last = finding.end.line.min(finding.start.line + SNIPPET_LINES - 1);
            for (offset, line) in extract_lines(content, finding.start.line, last)
                .lines()
                .enumerate()
            {
                println!(
                    "     {} {}",
                    style.dim(&format!("{:>5}", finding.start.line as usize + offset)),
                    line
                );
            }
            if finding.end.line > last {
                println!("     {}", style.dim("  ...  "));
            }
        });
        print_trailer(findings, ctx, style);
        Ok(())
    }
}

/// Read a finding's file once per run. A file that has moved since the scan
/// yields nothing rather than failing the report.
fn read_source<'a>(
    cache: &'a mut HashMap<String, String>,
    path: &str,
    roots: &[std::path::PathBuf],
) -> Option<&'a str> {
    if !cache.contains_key(path) {
        let content = resolve_path(path, roots).and_then(|p| std::fs::read_to_string(p).ok())?;
        cache.insert(path.to_string(), content);
    }
    cache.get(path).map(String::as_str)
}

/// Print every category block, calling `detail` after each finding line.
fn print_groups(findings: &[Finding], style: &Style, mut detail: impl FnMut(&Finding)) {
    if findings.is_empty() {
        println!("{}", style.green_prefix("No dead code found."));
        return;
    }
    for (category, group) in group_by_category(findings) {
        println!();
        println!(
            "{} {}",
            style.bold(category.title()),
            style.dim(&format!("({})", group.len()))
        );
        for finding in group {
            print_finding(finding, style);
            detail(finding);
        }
    }
}

fn print_finding(finding: &Finding, style: &Style) {
    let subject = if finding.name.is_empty() {
        style.bold_green(&finding.path)
    } else {
        format!(
            "{} {}",
            style.dim(&location(finding)),
            style.bold_green(&display_name(finding)),
        )
    };
    let kind = finding
        .symbol_kind
        .map(|k| format!("{} ", k.noun()))
        .unwrap_or_default();
    println!(
        " - {}{}  {}  {}",
        kind,
        subject,
        confidence_badge(finding, style),
        style.dim(&format!("{} lines", finding.lines)),
    );
    let reasons = reasons_clause(finding);
    if !reasons.is_empty() {
        println!("   {}", style.dim(&format!("↳ {reasons}")));
    }
}

/// `name`, or `name → exportedAs` when the two differ.
fn display_name(finding: &Finding) -> String {
    match (&finding.parent, &finding.exported_as) {
        (Some(parent), _) => format!("{parent}.{}", finding.name),
        (None, Some(exported)) => format!("{} (exported as {exported})", finding.name),
        (None, None) => finding.name.clone(),
    }
}

fn confidence_badge(finding: &Finding, style: &Style) -> String {
    let text = format!("{} {}%", finding.level().as_str(), finding.confidence);
    match finding.level() {
        ConfidenceLevel::Certain | ConfidenceLevel::High => style.red(&text),
        ConfidenceLevel::Medium => style.bold(&text),
        ConfidenceLevel::Low => style.dim(&text),
    }
}

fn print_trailer(findings: &[Finding], ctx: &DeadCodeContext<'_>, style: &Style) {
    let stats = ctx.stats;
    println!();
    if findings.is_empty() {
        println!(
            "{}",
            style.dim(&format!(
                "Analyzed {} files, {} declarations, {} entry points.",
                stats.files, stats.symbols, stats.entry_points
            ))
        );
    } else {
        println!(
            "Found {} in {} files ({:.1}% of {} lines).",
            style.bold(&format!("{} dead code findings", findings.len())),
            stats.files,
            stats.percentage,
            stats.total_lines,
        );
    }
    if stats.unparsed > 0 {
        println!(
            "{}",
            style.dim(&format!(
                "{} file(s) could not be parsed; their references are unknown:",
                stats.unparsed
            ))
        );
        // Enough to recognise a pattern (a fixtures directory, a generated
        // tree), not so many that they crowd out the findings.
        for path in stats.unparsed_files.iter().take(UNPARSED_LISTED) {
            println!("{}", style.dim(&format!("   - {path}")));
        }
        if stats.unparsed_files.len() > UNPARSED_LISTED {
            println!(
                "{}",
                style.dim(&format!(
                    "   ... and {} more (see the JSON report)",
                    stats.unparsed_files.len() - UNPARSED_LISTED
                ))
            );
        }
    }
    println!("{}", style.dim(&format_duration(ctx.duration)));
}

fn format_duration(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        format!("Done in {millis}ms")
    } else {
        format!("Done in {:.2}s", duration.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;
    use cpd_core::deadcode::{Category, Stats};
    use std::path::PathBuf;
    use std::time::Duration;

    fn options(no_colors: bool) -> ReporterOptions {
        let mut options = ReporterOptions::new(PathBuf::from("/tmp"));
        options.no_colors = no_colors;
        options
    }

    #[test]
    fn an_empty_report_does_not_panic() {
        let stats = Stats::default();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        for name in ["console", "console-full"] {
            let reporter = super::super::create_dead_code_reporter(name, &options(true)).unwrap();
            assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
        }
    }

    #[test]
    fn findings_are_rendered_without_panicking() {
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::from_millis(12));
        let reporter = ConsoleReporter::new(&options(true));
        assert!(
            reporter
                .report(&fixtures::findings(), &ctx, &PathBuf::from("/tmp"))
                .is_ok()
        );
    }

    #[test]
    fn console_full_survives_a_finding_whose_file_is_gone() {
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        let reporter = ConsoleFullReporter::new(&options(true));
        assert!(
            reporter
                .report(&fixtures::findings(), &ctx, &PathBuf::from("/tmp"))
                .is_ok(),
            "a deleted or unreadable file must not fail the report"
        );
    }

    #[test]
    fn a_member_is_shown_with_the_class_that_owns_it() {
        let mut finding = fixtures::finding(Category::UnusedMember, "src/a.ts", "render", 70);
        finding.parent = Some("Widget".into());
        assert_eq!(display_name(&finding), "Widget.render");
    }

    #[test]
    fn a_renamed_export_shows_both_names() {
        let mut finding = fixtures::finding(Category::UnusedExport, "src/a.ts", "inner", 85);
        finding.exported_as = Some("outer".into());
        assert_eq!(display_name(&finding), "inner (exported as outer)");
    }

    #[test]
    fn durations_switch_units_at_one_second() {
        assert_eq!(format_duration(Duration::from_millis(12)), "Done in 12ms");
        assert_eq!(format_duration(Duration::from_millis(999)), "Done in 999ms");
        assert_eq!(
            format_duration(Duration::from_millis(1500)),
            "Done in 1.50s"
        );
    }

    #[test]
    fn no_colors_produces_no_escape_sequences() {
        let style = Style::new(true);
        let finding = fixtures::finding(Category::UnusedExport, "src/a.ts", "x", 85);
        assert!(!confidence_badge(&finding, &style).contains('\x1b'));
    }
}
