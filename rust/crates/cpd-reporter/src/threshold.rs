// threshold.rs — Threshold reporter for cpd-reporter

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct ThresholdReporter {
    threshold: Option<f64>,
}

impl ThresholdReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            threshold: opts.threshold,
        }
    }
}

impl Reporter for ThresholdReporter {
    fn name(&self) -> &str {
        "threshold"
    }

    fn report(
        &self,
        _clones: &[CpdClone],
        ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        if let Some(threshold) = self.threshold {
            let actual = ctx.stats.total.percentage;
            if actual > threshold {
                return Err(ReporterError::ThresholdExceeded { actual, threshold });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use cpd_core::models::{StatRow, Statistics};
    use std::collections::HashMap;

    fn stats_with_pct(pct: f64) -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 2,
                duplicated_lines: 10,
                duplicated_tokens: 50,
                percentage: pct,
                percentage_tokens: pct,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01".to_string(),
        }
    }

    #[test]
    fn threshold_ok_when_below() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.threshold = Some(20.0);
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(10.0),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok(), "10% < 20% threshold must return Ok");
    }

    #[test]
    fn threshold_err_when_exceeded() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.threshold = Some(20.0);
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(25.0),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_err(), "25% > 20% threshold must return Err");
        match result.unwrap_err() {
            ReporterError::ThresholdExceeded { actual, threshold } => {
                assert!((actual - 25.0).abs() < 0.01);
                assert!((threshold - 20.0).abs() < 0.01);
            }
            other => panic!("expected ThresholdExceeded, got {:?}", other),
        }
    }

    #[test]
    fn threshold_ok_when_equal() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.threshold = Some(20.0);
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(20.0),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok(), "equal to threshold must return Ok");
    }

    #[test]
    fn threshold_ok_with_no_threshold_set() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(99.9),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok(), "no threshold must always return Ok");
    }

    #[test]
    fn silent_always_ok() {
        use crate::silent::SilentReporter;
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = SilentReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(100.0),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok(), "silent reporter must always return Ok");
    }
}
