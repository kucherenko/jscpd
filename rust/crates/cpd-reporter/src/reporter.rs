use crate::context::ReportContext;
use cpd_core::models::CpdClone;
use cpd_finder::blame::BlameMap;
use std::path::{Path, PathBuf};

/// Options passed to all reporters.
#[derive(Debug, Clone)]
pub struct ReporterOptions {
    pub output_dir: PathBuf,
    pub threshold: Option<f64>,
    pub blame: bool,
    pub no_colors: bool,
    pub blame_data: BlameMap,
    pub absolute: bool,
    /// Version stamped into reports (SARIF `tool.driver.version`, HTML footer).
    /// Binaries must set this to their own crate version — the default is
    /// cpd-reporter's version, which is not what `cpd --version` prints.
    pub tool_version: String,
    /// Clones with at least this many tokens are reported at SARIF level
    /// "error"; smaller clones (or all clones when None) stay "warning".
    pub sarif_error_tokens: Option<u32>,
    /// Base name for the report files each file reporter writes, without the
    /// extension. Aggregators that run several linters into one directory need
    /// this to avoid clobbering each other's `jscpd-report.json` (#1015).
    pub report_name: String,
}

/// Base name used for report files when `--report-name` is not given.
pub const DEFAULT_REPORT_NAME: &str = "jscpd-report";

/// The extensions of the reports `--report-name` names. A name that ends in
/// one would give `x.json.json`.
pub const REPORT_EXTENSIONS: &[&str] = &["json", "xml", "csv", "html", "md", "sarif", "edn"];

/// Every reporter [`create_reporter`] makes, by its main name.
pub const REPORTER_NAMES: &[&str] = &[
    "console",
    "console-full",
    "json",
    "sarif",
    "codeclimate",
    "ai",
    "xml",
    "csv",
    "edn",
    "html",
    "markdown",
    "badge",
    "openmetrics",
    "xcode",
    "threshold",
    "silent",
];

impl ReporterOptions {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            threshold: None,
            blame: false,
            no_colors: false,
            blame_data: BlameMap::new(),
            absolute: false,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            sarif_error_tokens: None,
            report_name: DEFAULT_REPORT_NAME.to_string(),
        }
    }

    /// The file a reporter of the `jscpd-report` family writes: the report
    /// name with `extension`. Every such reporter takes its file name here,
    /// so a new one follows `--report-name` too.
    pub fn report_file(&self, extension: &str) -> String {
        format!("{}.{extension}", self.report_name)
    }
}

/// Core reporter trait. Object-safe (no generic methods).
///
/// # Breaking Change (v0.8.0)
///
/// The `report` method signature has been changed to accept `&ReportContext`
/// instead of `&Statistics` to support timing data integration.
///
/// ## Migration Guide
///
/// **Old signature:**
/// ```ignore
/// fn report(&self, clones: &[CpdClone], stats: &Statistics, output_dir: &Path)
///     -> Result<(), ReporterError>
/// ```
///
/// **New signature:**
/// ```ignore
/// fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path)
///     -> Result<(), ReporterError>
/// ```
///
/// To migrate your reporter implementation:
/// 1. Change the second parameter from `stats: &Statistics` to `ctx: &ReportContext`
/// 2. Access statistics via `ctx.stats` instead of `stats`
/// 3. Access timing data via `ctx.duration`
///
/// Example:
/// ```ignore
/// // Old code:
/// fn report(&self, clones: &[CpdClone], stats: &Statistics, output_dir: &Path) -> Result<(), ReporterError> {
///     let total_lines = stats.total.lines;
///     // ...
/// }
///
/// // New code:
/// fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
///     let total_lines = ctx.stats.total.lines;
///     let detection_time = ctx.duration;
///     // ...
/// }
/// ```
pub trait Reporter: Send {
    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError>;

    /// Name of this reporter (for display/logging).
    fn name(&self) -> &str;
}

/// Errors that reporters can produce.
#[derive(Debug)]
pub enum ReporterError {
    Io(std::io::Error),
    Format(String),
    ThresholdExceeded { actual: f64, threshold: f64 },
}

impl std::fmt::Display for ReporterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error in reporter: {e}"),
            Self::Format(msg) => write!(f, "Format error in reporter: {msg}"),
            Self::ThresholdExceeded { actual, threshold } => {
                write!(
                    f,
                    "Duplication {actual:.1}% exceeds threshold {threshold:.1}%"
                )
            }
        }
    }
}

impl std::error::Error for ReporterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ReporterError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Factory: creates a boxed Reporter by name, returns None for unknown names.
pub fn create_reporter(name: &str, options: &ReporterOptions) -> Option<Box<dyn Reporter>> {
    match name {
        "console" => Some(Box::new(crate::console::ConsoleReporter::new(options))),
        "console-full" | "consoleFull" | "full" => Some(Box::new(
            crate::console_full::ConsoleFullReporter::new(options),
        )),
        "json" => Some(Box::new(crate::json_reporter::JsonReporter::new(options))),
        "sarif" => Some(Box::new(crate::sarif::SarifReporter::new(options))),
        "codeclimate" | "gitlab" => Some(Box::new(crate::codeclimate::CodeClimateReporter::new(
            options,
        ))),
        "ai" => Some(Box::new(crate::ai::AiReporter::new(options))),
        "xml" => Some(Box::new(crate::xml_reporter::XmlReporter::new(options))),
        "csv" => Some(Box::new(crate::csv_reporter::CsvReporter::new(options))),
        "edn" => Some(Box::new(crate::edn::EdnReporter::new(options))),
        "html" => Some(Box::new(crate::html::HtmlReporter::new(options))),
        "markdown" => Some(Box::new(crate::markdown_reporter::MarkdownReporter::new(
            options,
        ))),
        "badge" => Some(Box::new(crate::badge::BadgeReporter::new(options))),
        "openmetrics" => Some(Box::new(crate::openmetrics::OpenMetricsReporter::new(
            options,
        ))),
        "xcode" => Some(Box::new(crate::xcode::XcodeReporter::new(options))),
        "threshold" => Some(Box::new(crate::threshold::ThresholdReporter::new(options))),
        "silent" => Some(Box::new(crate::silent::SilentReporter::new(options))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::shared::fixtures::empty_stats;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn create_reporter_unknown_returns_none() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert!(create_reporter("unknown_xyz_reporter", &opts).is_none());
    }

    #[test]
    fn create_reporter_console_full_alias_full() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let r = create_reporter("full", &opts);
        assert!(r.is_some());
        assert_eq!(r.unwrap().name(), "console-full");
    }

    #[test]
    fn create_reporter_codeclimate_alias_gitlab() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        for name in ["codeclimate", "gitlab"] {
            let r = create_reporter(name, &opts);
            assert!(r.is_some(), "reporter '{name}' must resolve");
            assert_eq!(r.unwrap().name(), "codeclimate");
        }
    }

    #[test]
    fn create_reporter_console_full_alias_consolefull() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let r = create_reporter("consoleFull", &opts);
        assert!(r.is_some());
        assert_eq!(r.unwrap().name(), "console-full");
    }

    #[test]
    fn reporter_error_display_threshold() {
        let err = ReporterError::ThresholdExceeded {
            actual: 25.5,
            threshold: 10.0,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("25.5"),
            "display must contain actual percentage"
        );
        assert!(msg.contains("10.0"), "display must contain threshold");
    }

    #[test]
    fn reporter_error_display_format() {
        let err = ReporterError::Format("bad template".to_string());
        assert!(err.to_string().contains("bad template"));
    }

    #[test]
    fn reporter_error_exposes_the_io_error_as_its_source() {
        use std::error::Error;
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only");
        let err = ReporterError::from(io);
        assert!(err.to_string().contains("read-only"), "{err}");
        let source = err.source().expect("an I/O error has a source");
        assert_eq!(source.to_string(), "read-only");
        assert!(ReporterError::Format("x".to_string()).source().is_none());
    }

    #[test]
    fn every_reporter_name_resolves_to_its_reporter() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        for (name, expected) in [
            ("console", "console"),
            ("json", "json"),
            ("sarif", "sarif"),
            ("ai", "ai"),
            ("xml", "xml"),
            ("csv", "csv"),
            ("html", "html"),
            ("markdown", "markdown"),
            ("badge", "badge"),
            ("openmetrics", "openmetrics"),
            ("xcode", "xcode"),
            ("threshold", "threshold"),
            ("silent", "silent"),
        ] {
            let reporter = create_reporter(name, &opts)
                .unwrap_or_else(|| panic!("reporter '{name}' must resolve"));
            assert_eq!(reporter.name(), expected);
        }
    }

    #[test]
    fn every_listed_reporter_name_resolves() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        for name in REPORTER_NAMES {
            assert!(create_reporter(name, &opts).is_some(), "{name}");
        }
    }

    #[test]
    fn a_failing_write_is_reported_as_an_io_error() {
        // The output directory is a regular file: nothing can be written
        // below it.
        let file =
            std::env::temp_dir().join(format!("cpd-reporter-not-a-dir-{}", std::process::id()));
        std::fs::write(&file, "x").unwrap();
        let stats = empty_stats();
        let ctx = ReportContext::new(&stats, Duration::ZERO);
        for name in ["json", "xml", "sarif", "html"] {
            let reporter = create_reporter(name, &ReporterOptions::new(file.clone())).unwrap();
            match reporter.report(&[], &ctx, &file) {
                Err(ReporterError::Io(_)) => {}
                other => panic!("{name}: {other:?}"),
            }
        }
        let _ = std::fs::remove_file(&file);
    }
}
