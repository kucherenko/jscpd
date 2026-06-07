// console_full.rs — verbose clone reporter with source snippets for cpd-reporter
// Part of the jscpd project (https://github.com/kucherenko/jscpd)

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::{CpdClone, Fragment, StatRow, Statistics};
use cpd_finder::blame::BlameMap;
use std::{collections::BTreeMap, path::Path};

pub struct ConsoleFullReporter {
    blame: bool,
    no_colors: bool,
    blame_data: BlameMap,
}

impl ConsoleFullReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            blame: options.blame,
            no_colors: options.no_colors,
            blame_data: options.blame_data.clone(),
        }
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

    fn red(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[31m{}\x1b[39m", text)
        }
    }

    fn bold(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m{}\x1b[22m", text)
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
        let sep = self.dim("\u{2502}");

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
                "{:>line_a_w$} {sep} {:>author_a_w$} {sep} {} {sep} {:>line_b_w$} {sep} {:>author_a_w$} {sep} {}",
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
            if self.no_colors {
                println!("     … {} more lines", remaining);
            } else {
                println!("\x1b[90m     … {} more lines\x1b[39m", remaining);
            }
        }
    }

    fn print_table(&self, stats: &Statistics) {
        let headers = [
            "Format",
            "Files analyzed",
            "Total lines",
            "Total tokens",
            "Clones found",
            "Duplicated lines",
            "Duplicated tokens",
        ];

        let sorted_formats: BTreeMap<&str, &StatRow> =
            stats.formats.iter().map(|(k, v)| (k.as_str(), v)).collect();

        let format_rows: Vec<[String; 7]> = sorted_formats
            .iter()
            .map(|(fmt, row)| make_row(fmt, row))
            .collect();

        let total_row = make_row("Total:", &stats.total);

        let mut widths = [0usize; 7];
        for (i, h) in headers.iter().enumerate() {
            widths[i] = h.len();
        }
        for row in format_rows.iter().chain(std::iter::once(&total_row)) {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        self.print_sep(&widths, '\u{250c}', '\u{252c}', '\u{2510}');
        self.print_row(&headers.map(|h| h.to_string()), &widths, true);
        for row in &format_rows {
            self.print_sep(&widths, '\u{251c}', '\u{253c}', '\u{2524}');
            self.print_row(row, &widths, false);
        }
        self.print_sep(&widths, '\u{251c}', '\u{253c}', '\u{2524}');
        self.print_row(&total_row, &widths, false);
        self.print_sep(&widths, '\u{2514}', '\u{2534}', '\u{2518}');
    }

    fn print_sep(&self, widths: &[usize], left: char, mid: char, right: char) {
        let mut s = String::new();
        s.push(left);
        for (i, &w) in widths.iter().enumerate() {
            for _ in 0..(w + 2) {
                s.push('\u{2500}');
            }
            if i < widths.len() - 1 {
                s.push(mid);
            }
        }
        s.push(right);
        println!("{}", self.dim(&s));
    }

    fn print_row(&self, cells: &[String; 7], widths: &[usize], header: bool) {
        let sep = self.dim("\u{2502}");
        print!("{}", sep);
        for (i, cell) in cells.iter().enumerate() {
            if header {
                let text = self.red(&format!(" {:width$} ", cell, width = widths[i]));
                print!("{}{}", text, sep);
            } else if i == 0 && cell == "Total:" {
                let padded = format!("{:<width$}", cell, width = widths[i]);
                print!(" {}{} {}", self.bold(cell), &padded[cell.len()..], sep);
            } else {
                print!(" {:width$} {}", cell, sep, width = widths[i]);
            }
        }
        println!();
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
        if clones.is_empty() {
            println!("{}", self.dim("No duplicates found."));
        } else {
            for clone in clones {
                let fa = &clone.fragment_a;
                let fb = &clone.fragment_b;

                println!("{}", self.bold(&format!("Clone found ({})", clone.format)));
                println!(
                    " - {} [{}:{} - {}:{}]",
                    self.bold_green(&fa.source_id),
                    fa.start.line,
                    fa.start.column + 1,
                    fa.end.line,
                    fa.end.column + 1,
                );
                println!(
                    "   {} [{}:{} - {}:{}]",
                    self.bold_green(&fb.source_id),
                    fb.start.line,
                    fb.start.column + 1,
                    fb.end.line,
                    fb.end.column + 1,
                );

                if self.blame {
                    self.print_blame_snippet(fa, fb);
                } else {
                    print_snippet(fa, self.no_colors);
                    print_snippet(fb, self.no_colors);
                }
                println!();
            }
        }

        // Statistics table
        self.print_table(ctx.stats);

        if clones.is_empty() {
            println!("{}", self.dim("Found 0 clones."));
        } else {
            println!("{}", self.dim(&format!("Found {} clones.", clones.len())));
        }

        Ok(())
    }
}

fn clean_source_id(source_id: &str) -> &str {
    match source_id.rfind(':') {
        Some(pos) => &source_id[..pos],
        None => source_id,
    }
}

/// Read the source file and print the duplicated lines with line numbers.
/// For multi-format files (e.g. markdown), source_id may contain a `:format` suffix
/// (like "file.md:markdown") that must be stripped to form a valid file path.
fn print_snippet(fragment: &Fragment, no_colors: bool) {
    let clean_id = clean_source_id(&fragment.source_id);
    let path = std::path::Path::new(clean_id);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let start = fragment.start.line.saturating_sub(1) as usize;
    let end = fragment.end.line as usize;

    let lines: Vec<&str> = content.lines().collect();
    let snippet_lines = lines.get(start..end.min(lines.len())).unwrap_or(&[]);

    // Limit snippet to 20 lines to avoid flooding the terminal
    let max_display = 20usize;
    let truncated = snippet_lines.len() > max_display;
    let display_lines = if truncated {
        &snippet_lines[..max_display]
    } else {
        snippet_lines
    };

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

fn make_row(fmt: &str, row: &StatRow) -> [String; 7] {
    [
        fmt.to_string(),
        row.sources.to_string(),
        row.lines.to_string(),
        row.tokens.to_string(),
        row.clones.to_string(),
        format!("{} ({:.2}%)", row.duplicated_lines, row.percentage),
        format!("{} ({:.2}%)", row.duplicated_tokens, row.percentage_tokens),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use cpd_core::models::{BlameEntry, Fragment, Location, StatRow, Statistics};
    use std::time::Duration;
    use std::{collections::HashMap, path::PathBuf};

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn one_clone_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 1,
                duplicated_lines: 10,
                duplicated_tokens: 50,
                percentage: 10.0,
                percentage_tokens: 10.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

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
    fn empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
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

    #[test]
    fn blame_shown_when_enabled() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.blame = true;
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext {
            stats: &one_clone_stats(),
            duration: Duration::ZERO,
        };
        let result = reporter.report(&[make_clone_with_blame()], &ctx, &PathBuf::from("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn blame_hidden_when_disabled() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleFullReporter::new(&opts);
        let ctx = ReportContext {
            stats: &one_clone_stats(),
            duration: Duration::ZERO,
        };
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
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }
}
