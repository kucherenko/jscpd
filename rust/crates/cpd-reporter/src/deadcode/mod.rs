//! Reporters for a dead-code run.
//!
//! A parallel set to the clone reporters, under the same names: `console`,
//! `json`, `sarif`, `html` and the rest mean the same thing on both sides of
//! jscpd, take the same [`ReporterOptions`], and write into the same output
//! directory. What changes is the subject — findings rather than clone pairs —
//! so the two cannot share a trait without one of them lying about its input.

pub mod ci;
pub mod console;
pub mod html;
pub mod structured;
pub mod text;

use crate::reporter::{ReporterError, ReporterOptions};
use cpd_core::deadcode::{Category, Finding, Stats};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Run-level context every dead-code reporter receives.
pub struct DeadCodeContext<'a> {
    pub stats: &'a Stats,
    /// Wall-clock time of the run, for the trailer line.
    pub duration: Duration,
    /// Scan roots the finding paths are relative to. A reporter that shows
    /// source needs them: the report is written to be read from anywhere, so
    /// it cannot assume the working directory is still the scan root.
    pub roots: &'a [std::path::PathBuf],
}

impl<'a> DeadCodeContext<'a> {
    pub fn new(stats: &'a Stats, duration: Duration) -> Self {
        Self {
            stats,
            duration,
            roots: &[],
        }
    }

    /// Attach the scan roots so source excerpts can be resolved.
    pub fn with_roots(mut self, roots: &'a [std::path::PathBuf]) -> Self {
        self.roots = roots;
        self
    }
}

/// Resolve a finding's display path to a file on disk, trying the path as
/// written and then each scan root. Returns `None` when the file has moved or
/// the report is being read on another machine.
pub fn resolve_path(path: &str, roots: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    let direct = std::path::Path::new(path);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    roots
        .iter()
        .map(|root| root.join(path))
        .find(|candidate| candidate.is_file())
}

/// Core dead-code reporter trait. Object-safe, mirroring [`crate::Reporter`].
pub trait DeadCodeReporter: Send {
    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError>;

    /// Name this reporter is selected by.
    fn name(&self) -> &str;
}

/// Factory: creates a boxed [`DeadCodeReporter`] by name.
///
/// The names match [`crate::create_reporter`] exactly, so `-r sarif` means
/// SARIF whether the run is looking for clones or for dead code.
pub fn create_dead_code_reporter(
    name: &str,
    options: &ReporterOptions,
) -> Option<Box<dyn DeadCodeReporter>> {
    match name {
        "console" => Some(Box::new(console::ConsoleReporter::new(options))),
        "console-full" | "consoleFull" | "full" => {
            Some(Box::new(console::ConsoleFullReporter::new(options)))
        }
        "json" => Some(Box::new(structured::JsonReporter::new(options))),
        "xml" => Some(Box::new(structured::XmlReporter::new(options))),
        "csv" => Some(Box::new(structured::CsvReporter::new(options))),
        "sarif" => Some(Box::new(ci::SarifReporter::new(options))),
        "codeclimate" | "gitlab" => Some(Box::new(ci::CodeClimateReporter::new(options))),
        "openmetrics" => Some(Box::new(ci::OpenMetricsReporter::new(options))),
        "badge" => Some(Box::new(ci::BadgeReporter::new(options))),
        "xcode" => Some(Box::new(ci::XcodeReporter::new(options))),
        "threshold" => Some(Box::new(ci::ThresholdReporter::new(options))),
        "markdown" => Some(Box::new(text::MarkdownReporter::new(options))),
        "ai" => Some(Box::new(text::AiReporter::new(options))),
        "html" => Some(Box::new(html::HtmlReporter::new(options))),
        "silent" => Some(Box::new(text::SilentReporter)),
        _ => None,
    }
}

/// Every reporter name a dead-code run accepts, for `--help` and diagnostics.
pub fn dead_code_reporter_names() -> &'static [&'static str] {
    &[
        "console",
        "console-full",
        "json",
        "xml",
        "csv",
        "sarif",
        "codeclimate",
        "openmetrics",
        "badge",
        "xcode",
        "threshold",
        "markdown",
        "ai",
        "html",
        "silent",
    ]
}

// ── helpers shared by the reporters ────────────────────────────────────────

/// Findings grouped by category, in the order [`Category::ALL`] declares —
/// whole files first, then the narrowing rules — so every format tells the
/// story in the same order.
pub fn group_by_category(findings: &[Finding]) -> Vec<(Category, Vec<&Finding>)> {
    let mut groups: BTreeMap<Category, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        groups.entry(finding.category).or_default().push(finding);
    }
    groups.into_iter().collect()
}

/// Findings grouped by file, paths in lexical order.
pub fn group_by_path(findings: &[Finding]) -> Vec<(&str, Vec<&Finding>)> {
    let mut groups: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        groups
            .entry(finding.path.as_str())
            .or_default()
            .push(finding);
    }
    groups.into_iter().collect()
}

/// `path:line:column`, one-based, the form an editor and a terminal both
/// turn into a link.
pub fn location(finding: &Finding) -> String {
    format!(
        "{}:{}:{}",
        finding.path,
        finding.start.line,
        finding.start.column + 1
    )
}

/// The severity a machine-readable format should carry a finding at.
///
/// Confidence is the only input: a category is not more serious than another,
/// it is more or less certain, and a CI gate should react to certainty.
pub fn severity(finding: &Finding) -> Severity {
    match finding.confidence {
        90..=u8::MAX => Severity::Error,
        60..=89 => Severity::Warning,
        _ => Severity::Note,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    /// SARIF `level`.
    pub fn sarif(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }

    /// Code Climate `severity`.
    pub fn codeclimate(self) -> &'static str {
        match self {
            Self::Error => "major",
            Self::Warning => "minor",
            Self::Note => "info",
        }
    }

    /// Xcode diagnostic keyword.
    pub fn xcode(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning | Self::Note => "warning",
        }
    }
}

/// The reasons behind a finding as one human-readable clause, empty when the
/// finding has nothing against it.
pub fn reasons_clause(finding: &Finding) -> String {
    finding
        .reasons
        .iter()
        .map(|r| r.explain())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Test fixtures shared by the reporter tests.
#[cfg(test)]
pub(crate) mod fixtures {
    use cpd_core::deadcode::{Category, CategoryCount, Finding, Reason, Stats, SymbolKind};
    use cpd_core::models::Location;

    /// A temporary directory no other test can collide with.
    ///
    /// Reporter tests write real files and delete them afterwards; two tests
    /// sharing a path is a race that only shows up when the suite runs in
    /// parallel, which is exactly when nobody is watching.
    pub fn unique_dir(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("basta-report-{}-{label}-{n}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    pub fn location(line: u32) -> Location {
        Location {
            line,
            column: 0,
            offset: 0,
        }
    }

    pub fn finding(category: Category, path: &str, name: &str, confidence: u8) -> Finding {
        Finding {
            category,
            path: path.to_string(),
            name: name.to_string(),
            exported_as: None,
            symbol_kind: Some(SymbolKind::Function),
            parent: None,
            language: "js".into(),
            start: location(10),
            end: location(14),
            lines: 5,
            confidence,
            reasons: if confidence < 90 {
                vec![Reason::DynamicAccess]
            } else {
                Vec::new()
            },
            message: format!("`{name}` is never used"),
        }
    }

    pub fn findings() -> Vec<Finding> {
        vec![
            Finding {
                name: String::new(),
                symbol_kind: None,
                lines: 24,
                message: "src/orphan.ts is never imported".into(),
                ..finding(Category::UnusedFile, "src/orphan.ts", "", 95)
            },
            finding(Category::UnusedExport, "src/api.ts", "neverImported", 85),
            finding(Category::UnusedImport, "src/main.ts", "unusedDep", 100),
        ]
    }

    pub fn stats() -> Stats {
        Stats {
            files: 12,
            unparsed: 0,
            unparsed_files: Vec::new(),
            reachable_files: 10,
            symbols: 140,
            entry_points: 2,
            by_category: vec![
                CategoryCount {
                    category: Category::UnusedFile,
                    count: 1,
                    lines: 24,
                },
                CategoryCount {
                    category: Category::UnusedExport,
                    count: 1,
                    lines: 5,
                },
                CategoryCount {
                    category: Category::UnusedImport,
                    count: 1,
                    lines: 5,
                },
            ],
            dead_lines: 34,
            total_lines: 1000,
            percentage: 3.4,
            detection_date: "2026-09-15T10:00:00.000Z".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::deadcode::Reason;

    #[test]
    fn every_advertised_name_resolves_to_a_reporter() {
        let options = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        for name in dead_code_reporter_names() {
            assert!(
                create_dead_code_reporter(name, &options).is_some(),
                "reporter '{name}' must resolve"
            );
        }
        assert!(create_dead_code_reporter("nonsense", &options).is_none());
    }

    #[test]
    fn dead_code_reporter_names_match_the_clone_reporter_names() {
        let options = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        for name in dead_code_reporter_names() {
            assert!(
                crate::create_reporter(name, &options).is_some(),
                "'{name}' must mean the same thing on both sides of jscpd"
            );
        }
    }

    #[test]
    fn aliases_resolve_to_their_canonical_reporter() {
        let options = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        for (alias, canonical) in [
            ("full", "console-full"),
            ("consoleFull", "console-full"),
            ("gitlab", "codeclimate"),
        ] {
            let reporter = create_dead_code_reporter(alias, &options).expect(alias);
            assert_eq!(reporter.name(), canonical);
        }
    }

    #[test]
    fn severity_follows_confidence_not_category() {
        let certain = fixtures::finding(Category::UnusedMember, "a.ts", "x", 95);
        let likely = fixtures::finding(Category::UnusedImport, "a.ts", "x", 70);
        let unsure = fixtures::finding(Category::UnusedImport, "a.ts", "x", 40);
        assert_eq!(severity(&certain), Severity::Error);
        assert_eq!(severity(&likely), Severity::Warning);
        assert_eq!(severity(&unsure), Severity::Note);
    }

    #[test]
    fn grouping_orders_files_before_the_narrower_rules() {
        let findings = fixtures::findings();
        let groups = group_by_category(&findings);
        let categories: Vec<Category> = groups.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            categories,
            vec![
                Category::UnusedFile,
                Category::UnusedExport,
                Category::UnusedImport
            ]
        );
    }

    #[test]
    fn locations_are_one_based_and_clickable() {
        let mut finding = fixtures::finding(Category::UnusedExport, "src/a.ts", "x", 90);
        finding.start.column = 4;
        assert_eq!(location(&finding), "src/a.ts:10:5");
    }

    #[test]
    fn reasons_read_as_one_clause() {
        let mut finding = fixtures::finding(Category::UnusedExport, "src/a.ts", "x", 50);
        finding.reasons = vec![Reason::DynamicAccess, Reason::InTestFile];
        assert_eq!(
            reasons_clause(&finding),
            "file resolves names at runtime; declared in a test file"
        );
        finding.reasons.clear();
        assert!(reasons_clause(&finding).is_empty());
    }
}
