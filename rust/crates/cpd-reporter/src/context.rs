use cpd_core::models::Statistics;
use std::time::Duration;

/// Context for reporting that includes statistics and timing information.
///
/// The duration is formatted adaptively:
/// - Milliseconds (ms) when < 1000ms
/// - Seconds (s) when >= 1000ms
pub struct ReportContext<'a> {
    /// Reference to the clone detection statistics
    pub stats: &'a Statistics,
    /// Time taken for clone detection
    pub duration: Duration,
}

impl<'a> ReportContext<'a> {
    /// Creates a new ReportContext with the given statistics and duration.
    pub fn new(stats: &'a Statistics, duration: Duration) -> Self {
        Self { stats, duration }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{StatRow, Statistics};
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_report_context_new() {
        let stats = Statistics {
            total: StatRow {
                lines: 1000,
                tokens: 5000,
                sources: 10,
                clones: 5,
                duplicated_lines: 100,
                duplicated_tokens: 500,
                percentage: 15.5,
                percentage_tokens: 12.3,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2024-01-01".to_string(),
        };

        let duration = Duration::from_millis(1500);

        let context = ReportContext::new(&stats, duration);

        assert_eq!(context.duration, duration);
        // Statistics reference should point to the same data
        assert_eq!(context.stats.total.sources, 10);
        assert_eq!(context.stats.total.clones, 5);
    }

    #[test]
    fn test_report_context_fields_accessible() {
        let stats = Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 1,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2024-01-01".to_string(),
        };

        let duration = Duration::from_secs(2);
        let context = ReportContext::new(&stats, duration);

        // Verify we can access both fields
        let _stats_ref = context.stats;
        let _dur = context.duration;

        assert_eq!(context.duration.as_secs(), 2);
    }
}
