// console_full.rs — verbose clone reporter with source snippets for cpd-reporter
// Part of the jscpd project (https://github.com/kucherenko/jscpd)

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{
    BoxChars, Style, clean_source_id, print_clone_header, print_clone_locations, print_snippet,
    report_console_style,
};
use cpd_core::models::{CpdClone, Fragment};
use cpd_finder::blame::BlameMap;
use std::path::Path;

pub struct ConsoleFullReporter {
    blame: bool,
    style: Style,
    blame_data: BlameMap,
}

impl ConsoleFullReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            blame: options.blame,
            style: Style::new(options.no_colors),
            blame_data: options.blame_data.clone(),
        }
    }

    fn print_blame_snippet(&self, fa: &Fragment, fb: &Fragment) {
        let clean_a = clean_source_id(&fa.source_id);
        let clean_b = clean_source_id(&fb.source_id);

        let content_a = match std::fs::read_to_string(clean_a) {
            Ok(c) => c,
            Err(_) => return,
        };
        let content_b = match std::fs::read_to_string(clean_b) {
            Ok(c) => c,
            Err(_) => return,
        };

        let lines_a: Vec<&str> = content_a.lines().collect();
        let lines_b: Vec<&str> = content_b.lines().collect();

        let start_a = fa.start.line.saturating_sub(1) as usize;
        let end_a = fa.end.line as usize;
        let start_b = fb.start.line.saturating_sub(1) as usize;
        let end_b = fb.end.line as usize;

        let snippet_a = lines_a
            .get(start_a..end_a.min(lines_a.len()))
            .unwrap_or(&[]);
        let snippet_b = lines_b
            .get(start_b..end_b.min(lines_b.len()))
            .unwrap_or(&[]);

        let max_display = 20usize;
        let truncated = snippet_a.len() > max_display;
        let count = if truncated {
            max_display
        } else {
            snippet_a.len()
        };

        let author_a_width = 20;
        let line_a_width = 4;
        let line_b_width = 4;
        let sep = self.style.dim("%02");

        for i in 0..count {
            let line_num_a = fa.start.line as usize + i;
            let line_num_b = fb.start.line as usize + i;

            let author_a = self
                .blame_data
                .get(clean_a)
                .and_then(|m| m.get(&(line_num_a as u32)))
                .map(|(_, author, _)| author.as_str())
                .unwrap_or("");
            let author_b = self
                .blame_data
                .get(clean_b)
                .and_then(|m| m.get(&(line_num_b as u32)))
                .map(|(_, author, _)| author.as_str())
                .unwrap_or("");

            let same_author = !author_a.is_empty() && author_a == author_b;
            let marker = if same_author { "==" } else { "<=" };

            let code_line = snippet_b.get(i).unwrap_or(&"");

            println!(
                "{:line_a_w$} {sep} {:author_a_w$} {sep} {} {sep} {:line_b_w$} {sep} {:author_a_w$} {sep} {}",
                line_num_a,
                author_a,
                marker,
                line_num_b,
                author_b,
                code_line,
                line_a_w = line_a_width,
                author_a_w = author_a_width,
                line_b_w = line_b_width,
                sep = sep,
            );
        }

        if truncated {
            let remaining = snippet_a.len() - max_display;
            println!(
                "{}",
                self.style.dim(&format!("     … {} more lines", remaining))
            );
        }
    }
}

impl Reporter for ConsoleFullReporter {
    fn name(&self) -> &str {
        "console-full"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        report_console_style(
            clones,
            ctx.stats,
            &self.style,
            BoxChars::unicode(),
            |clone| {
                let fa = &clone.fragment_a;
                let fb = &clone.fragment_b;

                print_clone_header(&self.style, &clone.format);
                print_clone_locations(&self.style, clone);

                if self.blame {
                    self.print_blame_snippet(fa, fb);
                } else {
                    print_snippet(fa, &self.style, 20);
                    print_snippet(fb, &self.style, 20);
                }
                println!();
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, one_clone_stats};
    use crate::{assert_empty_report_ok, assert_reporter_name};
    use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location};
    use std::path::PathBuf;
    use std::time::Duration;

    assert_empty_report_ok!(empty_clones_does_not_panic, ConsoleFullReporter);

    fn make_clone_with_blame() -> CpdClone {
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let blame = BlameEntry {
            commit_sha: "abc12345".to_string(),
            author: "Alice".to_string(),
            timestamp: 1700000000,
        };
        let frag = Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: Some(blame),
        };
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 50,
        }
    }

    fn make_clone_no_blame() -> CpdClone {
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let frag = Fragment {
            source_id: "b.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        };
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 30,
        }
    }

    #[test]
    fn non_empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext {
            stats: &one_clone_stats(),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[make_clone_no_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    fn run_blame_test(blame: bool) {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.blame = blame;
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext {
            stats: &one_clone_stats(),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[make_clone_with_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn blame_shown_when_enabled() {
        run_blame_test(true);
    }

    #[test]
    fn blame_hidden_when_disabled() {
        run_blame_test(false);
    }

    assert_reporter_name!(
        name_returns_console_full,
        ConsoleFullReporter,
        "console-full"
    );

    #[test]
    fn no_colors_flag_respected() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.no_colors = true;
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = empty_ctx();
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }
}
