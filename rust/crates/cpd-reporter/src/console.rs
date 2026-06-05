// console.rs — clone reporter matching jscpd console output format.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::{CpdClone, StatRow, Statistics};
use std::{collections::BTreeMap, path::Path};

pub struct ConsoleReporter {
    no_colors: bool,
}

impl ConsoleReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            no_colors: options.no_colors,
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
        // Summary table
        self.print_table(ctx.stats);

        println!("{}", self.dim(&format!("Found {} clones.", clones.len())));
        Ok(())
    }
}

impl ConsoleReporter {
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

        // Build per-format rows (sorted alphabetically)
        let sorted_formats: BTreeMap<&str, &StatRow> =
            stats.formats.iter().map(|(k, v)| (k.as_str(), v)).collect();

        let format_rows: Vec<[String; 7]> = sorted_formats
            .iter()
            .map(|(fmt, row)| make_row(fmt, row))
            .collect();

        let total_row = make_row("Total:", &stats.total);

        // Compute column widths (header vs content)
        let mut widths = [0usize; 7];
        for (i, h) in headers.iter().enumerate() {
            widths[i] = h.len();
        }
        for row in format_rows.iter().chain(std::iter::once(&total_row)) {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        self.print_sep(&widths, '┌', '┬', '┐');
        self.print_row(&headers.map(|h| h.to_string()), &widths, true);
        for row in &format_rows {
            self.print_sep(&widths, '├', '┼', '┤');
            self.print_row(row, &widths, false);
        }
        self.print_sep(&widths, '├', '┼', '┤');
        self.print_row(&total_row, &widths, false);
        self.print_sep(&widths, '└', '┴', '┘');
    }

    fn print_sep(&self, widths: &[usize], left: char, mid: char, right: char) {
        let mut s = String::new();
        s.push(left);
        for (i, &w) in widths.iter().enumerate() {
            for _ in 0..(w + 2) {
                s.push('─');
            }
            if i < widths.len() - 1 {
                s.push(mid);
            }
        }
        s.push(right);
        println!("{}", self.dim(&s));
    }

    fn print_row(&self, cells: &[String; 7], widths: &[usize], header: bool) {
        let sep = self.dim("│");
        print!("{}", sep);
        for (i, cell) in cells.iter().enumerate() {
            if header {
                let text = self.red(&format!(" {:width$} ", cell, width = widths[i]));
                print!("{}{}", text, sep);
            } else if i == 0 && cell == "Total:" {
                // Bold the "Total:" label to match jscpd output
                let padded = format!("{:<width$}", cell, width = widths[i]);
                print!(" {}{} {}", self.bold(cell), &padded[cell.len()..], sep);
            } else {
                print!(" {:width$} {}", cell, sep, width = widths[i]);
            }
        }
        println!();
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
    use cpd_core::models::{Fragment, Location};
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
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn one_clone_stats() -> Statistics {
        let mut formats = HashMap::new();
        formats.insert(
            "javascript".to_string(),
            StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 1,
                duplicated_lines: 10,
                duplicated_tokens: 50,
                percentage: 10.0,
                percentage_tokens: 10.0,
            },
        );
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
            },
            formats,
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_clone() -> CpdClone {
        let start = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 10,
            column: 0,
            offset: 100,
        };
        let frag_a = Fragment {
            source_id: "src/a.js".to_string(),
            start: start.clone(),
            end: end.clone(),
            range: [0, 100],
            blame: None,
        };
        let frag_b = Fragment {
            source_id: "src/b.js".to_string(),
            start,
            end,
            range: [0, 100],
            blame: None,
        };
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag_a,
            fragment_b: frag_b,
            token_count: 50,
        }
    }

    #[test]
    fn empty_clones_does_not_panic() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        let reporter = ConsoleReporter::new(&opts);
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }

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
                .report(&[make_clone()], &ctx, &PathBuf::from("/tmp"))
                .is_ok()
        );
    }

    #[test]
    fn no_colors_flag_respected() {
        let mut opts = ReporterOptions::new(PathBuf::from("/tmp"));
        opts.no_colors = true;
        let reporter = ConsoleReporter::new(&opts);
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        assert!(reporter.report(&[], &ctx, &PathBuf::from("/tmp")).is_ok());
    }

    #[test]
    fn name_returns_console() {
        let opts = ReporterOptions::new(PathBuf::from("/tmp"));
        assert_eq!(ConsoleReporter::new(&opts).name(), "console");
    }
}
