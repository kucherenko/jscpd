use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct SilentReporter {
    no_colors: bool,
}

impl SilentReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            no_colors: opts.no_colors,
        }
    }

    fn bold(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m{}\x1b[22m", text)
        }
    }
}

impl Reporter for SilentReporter {
    fn name(&self) -> &str {
        "silent"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let total = &ctx.stats.total;
        let format_count = ctx.stats.formats.len();
        println!(
            "Duplications detection: Found {} exact clones with {}({}%) duplicated lines in {} ({} formats) files.",
            self.bold(&clones.len().to_string()),
            self.bold(&total.duplicated_lines.to_string()),
            self.bold(&format!("{:.2}", total.percentage)),
            self.bold(&total.sources.to_string()),
            format_count,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use cpd_core::models::{StatRow, Statistics};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    fn any_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 1000,
                tokens: 5000,
                sources: 10,
                clones: 5,
                duplicated_lines: 500,
                duplicated_tokens: 2500,
                percentage: 50.0,
                percentage_tokens: 50.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn silent_always_ok_with_high_duplication() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = SilentReporter::new(&opts);
        let ctx = ReportContext {
            stats: &any_stats(),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn silent_name_is_correct() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = SilentReporter::new(&opts);
        assert_eq!(reporter.name(), "silent");
    }

    #[test]
    fn silent_prints_summary() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.no_colors = true;
        let reporter = SilentReporter::new(&opts);
        let stats = Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 2,
                duplicated_lines: 20,
                duplicated_tokens: 100,
                percentage: 20.0,
                percentage_tokens: 20.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        };
        let ctx = ReportContext {
            stats: &stats,
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }
}
