// console.rs — clone reporter matching jscpd console output format.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{BoxChars, Style, format_location, print_clone_header, report_console_style};
use cpd_core::models::CpdClone;
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

impl Reporter for ConsoleReporter {
    fn name(&self) -> &str {
        "console"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        report_console_style(clones, ctx.stats, &self.style, BoxChars::ascii(), |clone| {
            let fa = &clone.fragment_a;
            let lines = fa.end.line.saturating_sub(fa.start.line) + 1;
            print_clone_header(&self.style, &clone.format);
            println!(
                " - {} [{}:{} - {}:{}] ({} lines, {} tokens)",
                self.style.bold_green(&fa.source_id),
                fa.start.line,
                fa.start.column + 1,
                fa.end.line,
                fa.end.column + 1,
                lines,
                clone.token_count,
            );
            let fb = &clone.fragment_b;
            println!(
                "   {}",
                format_location(
                    &fb.source_id,
                    fb.start.line,
                    fb.start.column,
                    fb.end.line,
                    fb.end.column
                )
            );
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_empty_report_ok;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, make_clone, one_clone_stats};
    use std::path::PathBuf;
    use std::time::Duration;

    assert_empty_report_ok!(empty_clones_does_not_panic, ConsoleReporter);

    #[test]
    fn non_empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleReporter::new(&opts);
        let ctx = ReportContext {
            stats: &one_clone_stats(),
            duration: Duration::ZERO,
        };
        assert!(
            reporter
                .report(
                    &[make_clone("src/a.js", "src/b.js", 50)],
                    &ctx,
                    &PathBuf::from("/tmp")
                )
                .is_ok()
        );
    }

    #[test]
    fn no_colors_flag_respected() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.no_colors = true;
        let reporter = ConsoleReporter::new(&opts);
        let ctx = empty_ctx();
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }

    #[test]
    fn name_returns_console() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert_eq!(ConsoleReporter::new(&opts).name(), "console");
    }
}
