use std::path::Path;
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

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

    fn report(&self, clones: &[CpdClone], _ctx: &ReportContext, _output_dir: &Path) -> Result<(), ReporterError> {
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
    use std::time::Duration;
    use std::path::PathBuf;
    use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 0, tokens: 0, sources: 0, clones: 0,
                duplicated_lines: 0, duplicated_tokens: 0,
                percentage: 0.0, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01".to_string(),
        }
    }

    #[test]
    fn xcode_returns_ok_on_empty_clones() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = XcodeReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn xcode_returns_ok_on_one_clone() {
        let loc = Location { line: 5, column: 3, offset: 0 };
        let end = Location { line: 15, column: 0, offset: 0 };
        let frag_a = Fragment {
            source_id: "MyFile.swift".to_string(),
            start: loc.clone(),
            end: end.clone(),
            range: [0, 200],
            blame: None,
        };
        let frag_b = Fragment {
            source_id: "OtherFile.swift".to_string(),
            start: Location { line: 10, column: 0, offset: 0 },
            end: Location { line: 20, column: 0, offset: 0 },
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
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[clone], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }
}