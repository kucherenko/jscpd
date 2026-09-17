// dashboard.rs — one static screen with the whole picture (`--dashboard`):
// project size, duplication, complexity and dead code.

use crate::shared::Style;
use crate::summary_render::human_size;
use cpd_core::deadcode::Report as DeadCodeReport;
use cpd_core::models::{CloneKind, CpdClone, Statistics};
use cpd_core::summary::{FileSummary, Summary};
use std::time::Duration;

/// Everything the dashboard shows, computed by the caller from one clone run,
/// one full-length summary and, when available, one dead-code run.
pub struct Dashboard<'a> {
    pub statistics: &'a Statistics,
    pub clones: &'a [CpdClone],
    /// Every file, not a top-N: the dashboard ranks it two ways.
    pub summary: &'a Summary,
    /// `None` when no file is in a language dead-code analysis supports.
    pub dead_code: Option<&'a DeadCodeReport>,
    /// Rows per ranked list.
    pub top: usize,
    pub elapsed: Duration,
}

const WIDTH: usize = 60;

fn heading(title: &str, style: &Style) {
    let rule = "─".repeat(WIDTH.saturating_sub(title.chars().count() + 4));
    println!("{}", style.bold(&format!("── {title} {rule}")));
}

fn thousands(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// `1 file`, `2 files`.
fn plural(n: u64, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        _ => format!("{n} {noun}s"),
    }
}

/// `N label` joined with ` · `, skipping zero counts.
fn counts(parts: &[(u64, &str)]) -> String {
    parts
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, label)| format!("{n} {label}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The first `numeric` columns right-aligned, the rest left-aligned.
fn table(headers: &[&str], numeric: usize, rows: &[Vec<String>], style: &Style) {
    let last = headers.len() - 1;
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| match (i < numeric, i == last) {
                (true, _) => format!("{c:>w$}", w = widths[i]),
                (false, true) => c.clone(),
                (false, false) => format!("{c:<w$}", w = widths[i]),
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!(
        "    {}",
        style.dim(&line(headers.iter().map(|h| h.to_string()).collect()))
    );
    for row in rows {
        println!("    {}", line(row.clone()));
    }
}

/// Duplicated lines as shown: overlapping clones can count a line twice.
fn duplicated_lines(file: &FileSummary) -> u64 {
    file.duplicated_lines.min(file.lines)
}

fn dup_percent(file: &FileSummary) -> f64 {
    match file.lines {
        0 => 0.0,
        lines => (file.duplicated_lines as f64 / lines as f64 * 100.0).min(100.0),
    }
}

pub fn print_dashboard(d: &Dashboard, style: &Style) {
    let total = &d.statistics.total;

    heading("Project", style);
    println!(
        "  {} · {} lines · {} tokens · {}",
        plural(total.sources, "file"),
        thousands(total.lines),
        thousands(total.tokens),
        plural(d.statistics.formats.len() as u64, "format")
    );
    let mut formats: Vec<(&String, u64)> = d
        .statistics
        .formats
        .iter()
        .map(|(name, row)| (name, row.lines))
        .collect();
    formats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if !formats.is_empty() {
        let largest = formats
            .iter()
            .take(d.top)
            .map(|(name, lines)| format!("{name} {}", thousands(*lines)))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {} {largest} lines", style.dim("largest:"));
    }

    println!();
    heading("Duplication", style);
    let kind_count = |kind: CloneKind| d.clones.iter().filter(|c| c.kind == kind).count() as u64;
    let kinds = counts(&[
        (kind_count(CloneKind::Exact), "exact"),
        (kind_count(CloneKind::Renamed), "renamed"),
        (kind_count(CloneKind::Similar), "similar"),
    ]);
    println!(
        "  {} duplicated lines · {}{}",
        style.bold(&format!("{:.2}%", total.percentage)),
        plural(d.clones.len() as u64, "clone"),
        match kinds.is_empty() {
            true => String::new(),
            false => format!(" ({kinds})"),
        }
    );
    let mut duplicated: Vec<&FileSummary> = d
        .summary
        .files
        .iter()
        .filter(|f| f.duplicated_lines > 0)
        .collect();
    duplicated.sort_by(|a, b| {
        dup_percent(b)
            .total_cmp(&dup_percent(a))
            .then(duplicated_lines(b).cmp(&duplicated_lines(a)))
            .then(a.path.cmp(&b.path))
    });
    if !duplicated.is_empty() {
        println!("  {}", style.dim("Most duplicated files:"));
        let rows: Vec<Vec<String>> = duplicated
            .iter()
            .take(d.top)
            .map(|f| {
                vec![
                    format!("{:.1}", dup_percent(f)),
                    duplicated_lines(f).to_string(),
                    f.path.clone(),
                ]
            })
            .collect();
        table(&["DUP%", "LINES", "PATH"], 2, &rows, style);
    }

    println!();
    heading("Complexity", style);
    let code: Vec<&FileSummary> = d
        .summary
        .files
        .iter()
        .filter(|f| f.complexity > 0)
        .collect();
    let total_cx: u64 = code.iter().map(|f| f.complexity).sum();
    let mean = match code.len() {
        0 => 0.0,
        n => total_cx as f64 / n as f64,
    };
    println!(
        "  {} total · {:.1} mean per file",
        style.bold(&total_cx.to_string()),
        mean
    );
    let mut complex = code;
    complex.sort_by(|a, b| b.complexity.cmp(&a.complexity).then(a.path.cmp(&b.path)));
    if !complex.is_empty() {
        println!("  {}", style.dim("Most complex files:"));
        let rows: Vec<Vec<String>> = complex
            .iter()
            .take(d.top)
            .map(|f| {
                vec![
                    f.complexity.to_string(),
                    f.lines.to_string(),
                    human_size(f.bytes),
                    f.path.clone(),
                ]
            })
            .collect();
        table(&["CX", "LINES", "SIZE", "PATH"], 3, &rows, style);
    }

    println!();
    heading("Dead code (JavaScript, TypeScript, Python)", style);
    match d.dead_code {
        None => println!(
            "  {}",
            style.dim("no JavaScript, TypeScript or Python files")
        ),
        Some(report) => {
            let stats = &report.statistics;
            let by_category = stats
                .by_category
                .iter()
                .map(|c| format!("{} {}", c.count, c.category.as_str()))
                .collect::<Vec<_>>()
                .join(" · ");
            println!(
                "  {} unused lines · {} in {}",
                style.bold(&format!("{:.2}%", stats.percentage)),
                plural(report.findings.len() as u64, "finding"),
                plural(stats.files as u64, "file")
            );
            if !by_category.is_empty() {
                println!("  {by_category}");
            }
            let mut largest: Vec<_> = report.findings.iter().collect();
            largest.sort_by(|a, b| {
                b.lines
                    .cmp(&a.lines)
                    .then(a.path.cmp(&b.path))
                    .then(a.start.line.cmp(&b.start.line))
            });
            if !largest.is_empty() {
                println!("  {}", style.dim("Largest findings:"));
                let rows: Vec<Vec<String>> = largest
                    .iter()
                    .take(d.top)
                    .map(|f| {
                        let what = match f.name.is_empty() {
                            true => f.path.clone(),
                            false => format!("{}:{} {}", f.path, f.start.line, f.name),
                        };
                        vec![f.lines.to_string(), f.category.as_str().to_string(), what]
                    })
                    .collect();
                table(&["LINES", "CATEGORY", "WHERE"], 1, &rows, style);
            }
        }
    }
    println!();
    println!(
        "{}",
        style.dim(&format!("time: {:.3}ms", d.elapsed.as_secs_f64() * 1000.0))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_rounds_to_one_decimal() {
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(38_240), "38.2K");
        assert_eq!(thousands(2_500_000), "2.5M");
    }

    #[test]
    fn plural_only_for_one() {
        assert_eq!(plural(1, "clone"), "1 clone");
        assert_eq!(plural(0, "clone"), "0 clones");
        assert_eq!(plural(2, "format"), "2 formats");
    }

    #[test]
    fn counts_skip_zeroes() {
        assert_eq!(
            counts(&[(3, "exact"), (0, "renamed"), (1, "similar")]),
            "3 exact · 1 similar"
        );
        assert_eq!(counts(&[(0, "exact")]), "");
    }
}
