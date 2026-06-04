use std::path::{Path, PathBuf};
use cpd_core::models::{CpdClone, Statistics};

/// Options passed to all reporters.
#[derive(Debug, Clone)]
pub struct ReporterOptions {
    pub output_dir: PathBuf,
    pub threshold: Option<f64>,
    pub blame: bool,
    pub no_colors: bool,
}

impl ReporterOptions {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            threshold: None,
            blame: false,
            no_colors: false,
        }
    }
}

/// Core reporter trait. Object-safe (no generic methods).
pub trait Reporter: Send {
    fn report(
        &self,
        clones: &[CpdClone],
        stats: &Statistics,
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
                write!(f, "Duplication {actual:.1}% exceeds threshold {threshold:.1}%")
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
        "console-full" => Some(Box::new(crate::console_full::ConsoleFullReporter::new(options))),
        "json" => Some(Box::new(crate::json_reporter::JsonReporter::new(options))),
        "sarif" => Some(Box::new(crate::sarif::SarifReporter::new(options))),
        "ai" => Some(Box::new(crate::ai::AiReporter::new(options))),
        "xml" => Some(Box::new(crate::xml_reporter::XmlReporter::new(options))),
        "csv" => Some(Box::new(crate::csv_reporter::CsvReporter::new(options))),
        "html" => Some(Box::new(crate::html::HtmlReporter::new(options))),
        "markdown" => Some(Box::new(crate::markdown_reporter::MarkdownReporter::new(options))),
        "badge" => Some(Box::new(crate::badge::BadgeReporter::new(options))),
        "xcode" => Some(Box::new(crate::xcode::XcodeReporter::new(options))),
        "threshold" => Some(Box::new(crate::threshold::ThresholdReporter::new(options))),
        "silent" => Some(Box::new(crate::silent::SilentReporter::new(options))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use cpd_core::models::{Statistics, StatRow};
    use std::collections::HashMap;

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 0, tokens: 0, sources: 0, clones: 0,
                duplicated_lines: 0, duplicated_tokens: 0,
                percentage: 0.0, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn create_reporter_console_returns_some() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert!(create_reporter("console", &opts).is_some());
    }

    #[test]
    fn create_reporter_unknown_returns_none() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert!(create_reporter("unknown_xyz_reporter", &opts).is_none());
    }

    #[test]
    fn reporter_is_object_safe() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter: Box<dyn Reporter> = create_reporter("console", &opts).unwrap();
        assert_eq!(reporter.name(), "console");
    }

    #[test]
    fn reporter_error_display_threshold() {
        let err = ReporterError::ThresholdExceeded { actual: 25.5, threshold: 10.0 };
        let msg = err.to_string();
        assert!(msg.contains("25.5"), "display must contain actual percentage");
        assert!(msg.contains("10.0"), "display must contain threshold");
    }

    #[test]
    fn reporter_error_display_format() {
        let err = ReporterError::Format("bad template".to_string());
        assert!(err.to_string().contains("bad template"));
    }

    #[test]
    fn reporter_error_implements_std_error() {
        let err = ReporterError::Format("x".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn console_reporter_on_empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = create_reporter("console", &opts).unwrap();
        let result = reporter.report(&[], &empty_stats(), &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }
}
