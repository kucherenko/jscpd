use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use std::path::Path;

pub struct XcodeReporter;

impl XcodeReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for XcodeReporter {
    fn name(&self) -> &str {
        "xcode"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        _ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        for clone in clones {
            let fa = &clone.fragment_a;
            let fb = &clone.fragment_b;
            let line_count = fa.end.line.saturating_sub(fa.start.line);
            println!(
                "{}:{}:{}: warning: Found {} lines ({}-{}) duplicated on file {} ({}-{})",
                fa.source_id,
                fa.start.line,
                fa.start.column,
                line_count,
                fa.start.line,
                fa.end.line,
                fb.source_id,
                fb.start.line,
                fb.end.line,
            );
        }
        println!("Found {} clones.", clones.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_empty_report_ok;

    // The warning lines themselves are checked on real stdout in
    // tests/console_output.rs.

    assert_empty_report_ok!(xcode_returns_ok_on_empty_clones, XcodeReporter);
}
