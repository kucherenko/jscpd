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
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::empty_ctx;
    use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow::default(),
            formats: HashMap::new(),
            detection_date: "2026-01-01".to_string(),
        }
    }

    assert_empty_report_ok!(xcode_returns_ok_on_empty_clones, XcodeReporter);

    #[test]
    fn xcode_returns_ok_on_one_clone() {
        let loc = Location {
            line: 5,
            column: 3,
            offset: 0,
        };
        let end = Location {
            line: 15,
            column: 0,
            offset: 0,
        };
        let frag_a = Fragment {
            source_id: "MyFile.swift".to_string(),
            start: loc.clone(),
            end: end.clone(),
            range: [0, 200],
            blame: None,
        };
        let frag_b = Fragment {
            source_id: "OtherFile.swift".to_string(),
            start: Location {
                line: 10,
                column: 0,
                offset: 0,
            },
            end: Location {
                line: 20,
                column: 0,
                offset: 0,
            },
            range: [100, 300],
            blame: None,
        };
        let clone = CpdClone {
            format: "swift".to_string(),
            fragment_a: frag_a,
            fragment_b: frag_b,
            token_count: 30,
        };
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = XcodeReporter::new(&opts);
        let ctx = empty_ctx();
        let result = reporter.report(&[clone], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }
}
