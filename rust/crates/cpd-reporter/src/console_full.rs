// console_full.rs — verbose clone reporter with source snippets for cpd-reporter
// Part of the jscpd project (https://github.com/kucherenko/jscpd)

use std::path::Path;
use cpd_core::models::{CpdClone, Fragment};
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct ConsoleFullReporter {
    blame: bool,
    no_colors: bool,
}

impl ConsoleFullReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self { blame: options.blame, no_colors: options.no_colors }
    }

    fn bold_green(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m\x1b[32m{}\x1b[39m\x1b[22m", text)
        }
    }

    fn dim(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[90m{}\x1b[39m", text)
        }
    }
}

impl Reporter for ConsoleFullReporter {
    fn name(&self) -> &str { "console-full" }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, _output_dir: &Path) -> Result<(), ReporterError> {
        if clones.is_empty() {
            println!("{}", self.dim("No duplicates found."));
            return Ok(());
        }

        for clone in clones {
            let fa = &clone.fragment_a;
            let fb = &clone.fragment_b;
            let lines = fa.end.line.saturating_sub(fa.start.line) + 1;

            println!("Clone found ({}):", clone.format);
            println!(
                " - {} [{}:{} - {}:{}] ({} lines, {} tokens)",
                self.bold_green(&fa.source_id),
                fa.start.line,
                fa.start.column + 1,
                fa.end.line,
                fa.end.column + 1,
                lines,
                clone.token_count,
            );
            println!(
                "   {} [{}:{} - {}:{}]",
                self.bold_green(&fb.source_id),
                fb.start.line,
                fb.start.column + 1,
                fb.end.line,
                fb.end.column + 1,
            );

            // Blame info if available and requested
            if self.blame {
                if let Some(b) = &fa.blame {
                    println!("   {} {} by {} ({})",
                        self.dim("blame:"),
                        &b.commit_sha[..b.commit_sha.len().min(8)],
                        b.author,
                        b.timestamp,
                    );
                }
            }

            // Source snippet for the first fragment
            print_snippet(fa, self.no_colors);
            println!();
        }

        println!("{}", self.dim(&format!(
            "Total: {} duplicated lines ({:.2}%)",
            ctx.stats.total.duplicated_lines,
            ctx.stats.total.percentage,
        )));
        Ok(())
    }
}

/// Read the source file and print the duplicated lines with line numbers.
fn print_snippet(fragment: &Fragment, no_colors: bool) {
    let path = std::path::Path::new(&fragment.source_id);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return, // skip snippet if file can't be read
    };

    let start = fragment.start.line.saturating_sub(1) as usize; // convert 1-based to 0-based index
    let end = fragment.end.line as usize;

    let lines: Vec<&str> = content.lines().collect();
    let snippet_lines = lines.get(start..end.min(lines.len())).unwrap_or(&[]);

    // Limit snippet to 20 lines to avoid flooding the terminal
    let max_display = 20usize;
    let truncated = snippet_lines.len() > max_display;
    let display_lines = if truncated { &snippet_lines[..max_display] } else { snippet_lines };

    for (i, line) in display_lines.iter().enumerate() {
        let line_num = fragment.start.line as usize + i;
        let prefix = format!("{:>4} │ ", line_num);
        if no_colors {
            println!("{}{}", prefix, line);
        } else {
            println!("\x1b[90m{}\x1b[39m{}", prefix, line);
        }
    }
    if truncated {
        let remaining = snippet_lines.len() - max_display;
        if no_colors {
            println!("     … {} more lines", remaining);
        } else {
            println!("\x1b[90m     … {} more lines\x1b[39m", remaining);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, path::PathBuf};
    use cpd_core::models::{BlameEntry, Fragment, Location, StatRow, Statistics};
    use crate::reporter::ReporterOptions;
    use std::time::Duration;
    use crate::context::ReportContext;

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100, tokens: 500, sources: 5, clones: 0,
                duplicated_lines: 0, duplicated_tokens: 0,
                percentage: 0.0, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn one_clone_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100, tokens: 500, sources: 5, clones: 1,
                duplicated_lines: 10, duplicated_tokens: 50,
                percentage: 10.0, percentage_tokens: 10.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_clone_with_blame() -> CpdClone {
        let loc = Location { line: 1, column: 0, offset: 0 };
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
        CpdClone { format: "javascript".to_string(), fragment_a: frag.clone(), fragment_b: frag, token_count: 50 }
    }

    fn make_clone_no_blame() -> CpdClone {
        let loc = Location { line: 1, column: 0, offset: 0 };
        let frag = Fragment {
            source_id: "b.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        };
        CpdClone { format: "javascript".to_string(), fragment_a: frag.clone(), fragment_b: frag, token_count: 30 }
    }

    #[test]
    fn empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn non_empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext { stats: &one_clone_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[make_clone_no_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn blame_shown_when_enabled() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.blame = true;
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext { stats: &one_clone_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[make_clone_with_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn blame_hidden_when_disabled() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext { stats: &one_clone_stats(), duration: Duration::ZERO };
        let result = reporter.report(&[make_clone_with_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn name_returns_console_full() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        assert_eq!(reporter.name(), "console-full");
    }

    #[test]
    fn no_colors_flag_respected() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.no_colors = true;
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }
}
