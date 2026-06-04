// xcode.rs — Xcode-compatible warning reporter for cpd-reporter

use std::path::Path;
use cpd_core::models::{CpdClone, Statistics};
use crate::reporter::{Reporter, ReporterError, ReporterOptions};

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

    fn report(&self, clones: &[CpdClone], _stats: &Statistics, _output_dir: &Path) -> Result<(), ReporterError> {
        for clone in clones {
            println!(
                "{}:{}: warning: [cpd] Duplicated code found. {} tokens duplicated.",
                clone.fragment_a.source_id,
                clone.fragment_a.start.line,
                clone.token_count,
            );
            println!(
                "{}:{}: warning: [cpd] Duplicated code found. {} tokens duplicated.",
                clone.fragment_b.source_id,
                clone.fragment_b.start.line,
                clone.token_count,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;

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
        let result = reporter.report(&[], &empty_stats(), &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn xcode_returns_ok_on_one_clone() {
        let loc = Location { line: 5, column: 0, offset: 0 };
        let frag = Fragment {
            source_id: "MyFile.swift".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        };
        let clone = CpdClone {
            format: "swift".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 30,
        };
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = XcodeReporter::new(&opts);
        let result = reporter.report(&[clone], &empty_stats(), &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }
}
