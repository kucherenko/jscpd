// silent.rs — Silent (no-op) reporter for cpd-reporter

use std::path::Path;
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct SilentReporter;

impl SilentReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for SilentReporter {
    fn name(&self) -> &str {
        "silent"
    }

    fn report(&self, _clones: &[CpdClone], _ctx: &ReportContext, _output_dir: &Path) -> Result<(), ReporterError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;

    use cpd_core::models::{Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;

    fn any_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 1000, tokens: 5000, sources: 10, clones: 5,
                duplicated_lines: 500, duplicated_tokens: 2500,
                percentage: 50.0, percentage_tokens: 50.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01".to_string(),
        }
    }

    #[test]
    fn silent_always_ok_with_high_duplication() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = SilentReporter::new(&opts);
        let ctx = ReportContext { stats: &any_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn silent_name_is_correct() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = SilentReporter::new(&opts);
        assert_eq!(reporter.name(), "silent");
    }
}
