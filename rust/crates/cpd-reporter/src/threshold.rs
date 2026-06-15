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
    use crate::shared::fixtures::stats_with_pct;

    fn run_threshold(threshold: Option<f64>, pct: f64) -> Result<(), ReporterError> {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.threshold = threshold;
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(pct, pct as u64),
            duration: Duration::ZERO,
        };
        reporter.report(&[], &ctx, &PathBuf::from("/tmp"))
    }

    #[test]
    fn threshold_ok_when_below() {
        assert!(
            run_threshold(Some(20.0), 10.0).is_ok(),
            "10% < 20% threshold must return Ok"
        );
    }

    #[test]
    fn threshold_err_when_exceeded() {
        let result = run_threshold(Some(20.0), 25.0);
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
        assert!(
            run_threshold(Some(20.0), 20.0).is_ok(),
            "equal to threshold must return Ok"
        );
    }

    #[test]
    fn threshold_ok_with_no_threshold_set() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ThresholdReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(99.9, 99),
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
            stats: &stats_with_pct(100.0, 100),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok(), "silent reporter must always return Ok");
    }
}
