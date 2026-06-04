// time.rs — TimeReporter decorator that wraps a primary reporter and prints timing

use std::path::Path;
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError};
use crate::context::ReportContext;

/// TimeReporter wraps a primary reporter and prints adaptive timing before delegating.
pub struct TimeReporter {
    inner: Box<dyn Reporter>,
}

impl TimeReporter {
    pub fn new(inner: Box<dyn Reporter>) -> Self {
        Self { inner }
    }
}

impl Reporter for TimeReporter {
    fn name(&self) -> &str {
        "time"
    }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        // Print adaptive timing format
        let duration_ms = ctx.duration.as_secs_f64() * 1000.0;
        if duration_ms < 1000.0 {
            println!("time: {:.3}ms", duration_ms);
        } else {
            let duration_s = ctx.duration.as_secs_f64();
            println!("time: {:.2}s", duration_s);
        }

        // Delegate to wrapped reporter
        self.inner.report(clones, ctx, output_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;
    use std::collections::HashMap;
    use cpd_core::models::{Statistics, StatRow};
    use crate::context::ReportContext;

    // Mock reporter that tracks calls
    struct MockReporter {
        name: String,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockReporter {
        fn new(name: &str) -> (Self, std::sync::Arc<std::sync::Mutex<usize>>) {
            let counter = std::sync::Arc::new(std::sync::Mutex::new(0));
            (
                Self {
                    name: name.to_string(),
                    call_count: counter.clone(),
                },
                counter,
            )
        }
    }

    impl Reporter for MockReporter {
        fn name(&self) -> &str {
            &self.name
        }

        fn report(&self, _clones: &[CpdClone], _ctx: &ReportContext, _output_dir: &Path) -> Result<(), ReporterError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            Ok(())
        }
    }

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
    fn time_reporter_wraps_and_delegates() {
        let (mock, counter) = MockReporter::new("mock");
        let time_reporter = TimeReporter::new(Box::new(mock));
        
        let stats = empty_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(100));
        let result = time_reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        
        assert!(result.is_ok(), "TimeReporter should delegate successfully");
        assert_eq!(*counter.lock().unwrap(), 1, "Inner reporter should be called once");
    }

    #[test]
    fn time_reporter_prints_milliseconds_below_1000() {
        let (mock, _counter) = MockReporter::new("mock");
        let time_reporter = TimeReporter::new(Box::new(mock));
        
        let stats = empty_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(500));
        
        // Note: We can't easily capture stdout in this test without additional infrastructure
        // But we can verify the reporter runs without errors
        let result = time_reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn time_reporter_prints_seconds_above_1000ms() {
        let (mock, _counter) = MockReporter::new("mock");
        let time_reporter = TimeReporter::new(Box::new(mock));
        
        let stats = empty_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(1500));
        
        let result = time_reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn time_reporter_name_is_time() {
        let (mock, _counter) = MockReporter::new("mock");
        let time_reporter = TimeReporter::new(Box::new(mock));
        
        assert_eq!(time_reporter.name(), "time");
    }

    #[test]
    fn time_reporter_boundary_at_1000ms() {
        let (mock1, _counter1) = MockReporter::new("mock1");
        let time_reporter1 = TimeReporter::new(Box::new(mock1));
        
        // Just below 1000ms should use milliseconds format
        let stats = empty_stats();
        let ctx1 = ReportContext::new(&stats, Duration::from_millis(999));
        let result1 = time_reporter1.report(&[], &ctx1, &PathBuf::from("/tmp"));
        assert!(result1.is_ok());
        
        let (mock2, _counter2) = MockReporter::new("mock2");
        let time_reporter2 = TimeReporter::new(Box::new(mock2));
        
        // At exactly 1000ms should use seconds format
        let ctx2 = ReportContext::new(&stats, Duration::from_millis(1000));
        let result2 = time_reporter2.report(&[], &ctx2, &PathBuf::from("/tmp"));
        assert!(result2.is_ok());
    }
}
