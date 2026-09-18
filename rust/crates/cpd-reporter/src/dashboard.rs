// dashboard.rs — one static screen with the whole picture (`--dashboard`):
// health, project size, duplication, complexity and dead code. The numbers
// are gathered once into a [`DashboardView`], which the console prints and
// the JSON reporter serializes, so the two can never disagree.

use crate::shared::Style;
use crate::summary_render::human_size;
use cpd_core::deadcode::Report as DeadCodeReport;
use cpd_core::health::Health;
use cpd_core::models::{CloneKind, CpdClone, Statistics};
use cpd_core::summary::{FileSummary, Summary};
use serde::Serialize;
use std::time::Duration;

/// Everything the dashboard shows, computed by the caller from one clone run,
/// one full-length summary and, when available, one dead-code run.
pub struct Dashboard<'a> {
    pub health: &'a Health,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardView {
    pub health: Health,
    pub project: ProjectView,
    pub duplication: DuplicationView,
    pub complexity: ComplexityView,
    /// `null` when no file is in a language dead-code analysis supports.
    pub dead_code: Option<DeadCodeView>,
}

#[derive(Debug, Serialize)]
pub struct ProjectView {
    pub files: u64,
    pub lines: u64,
    pub tokens: u64,
    /// Every format, largest first.
    pub formats: Vec<FormatLines>,
}

#[derive(Debug, Serialize)]
pub struct FormatLines {
    pub format: String,
    pub lines: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicationView {
    pub percentage: f64,
    pub clones: u64,
    pub exact: u64,
    pub renamed: u64,
    pub similar: u64,
    /// The most duplicated files, by share of their lines.
    pub files: Vec<DuplicatedFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatedFile {
    pub path: String,
    pub percentage: f64,
    pub duplicated_lines: u64,
}

#[derive(Debug, Serialize)]
pub struct ComplexityView {
    pub total: u64,
    /// Mean per code file; prose and data files are left out.
    pub mean: f64,
    /// The most complex files.
    pub files: Vec<ComplexFile>,
}

#[derive(Debug, Serialize)]
pub struct ComplexFile {
    pub path: String,
    pub complexity: u64,
    pub lines: u64,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadCodeView {
    pub percentage: f64,
    pub findings: u64,
    pub files: u64,
    pub by_category: Vec<CategoryFindings>,
    /// The largest findings, by lines.
    pub largest: Vec<LargeFinding>,
}

#[derive(Debug, Serialize)]
pub struct CategoryFindings {
    pub category: &'static str,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct LargeFinding {
    pub category: &'static str,
    pub path: String,
    /// Empty for a whole-file finding.
    pub name: String,
    pub line: u32,
    pub lines: u32,
}

const WIDTH: usize = 60;

fn heading(title: &str, style: &Style) {
    let rule = "─".repeat(WIDTH.saturating_sub(title.chars().count() + 4));
    println!("{}", style.bold(&format!("── {title} {rule}")));
}

pub(crate) fn thousands(n: u64) -> String {
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

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

impl Dashboard<'_> {
    /// Gather every number the dashboard shows.
    pub fn view(&self) -> DashboardView {
        let total = &self.statistics.total;
        let mut formats: Vec<FormatLines> = self
            .statistics
            .formats
            .iter()
            .map(|(format, row)| FormatLines {
                format: format.clone(),
                lines: row.lines,
            })
            .collect();
        formats.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.format.cmp(&b.format)));

        let kind_count =
            |kind: CloneKind| self.clones.iter().filter(|c| c.kind == kind).count() as u64;
        let mut duplicated: Vec<&FileSummary> = self
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

        let mut code: Vec<&FileSummary> = self
            .summary
            .files
            .iter()
            .filter(|f| f.complexity > 0)
            .collect();
        let total_complexity: u64 = code.iter().map(|f| f.complexity).sum();
        let mean = match code.len() {
            0 => 0.0,
            n => total_complexity as f64 / n as f64,
        };
        code.sort_by(|a, b| b.complexity.cmp(&a.complexity).then(a.path.cmp(&b.path)));

        DashboardView {
            health: self.health.clone(),
            project: ProjectView {
                files: total.sources,
                lines: total.lines,
                tokens: total.tokens,
                formats,
            },
            duplication: DuplicationView {
                percentage: total.percentage,
                clones: self.clones.len() as u64,
                exact: kind_count(CloneKind::Exact),
                renamed: kind_count(CloneKind::Renamed),
                similar: kind_count(CloneKind::Similar),
                files: duplicated
                    .iter()
                    .take(self.top)
                    .map(|f| DuplicatedFile {
                        path: f.path.clone(),
                        percentage: round1(dup_percent(f)),
                        duplicated_lines: duplicated_lines(f),
                    })
                    .collect(),
            },
            complexity: ComplexityView {
                total: total_complexity,
                mean: round1(mean),
                files: code
                    .iter()
                    .take(self.top)
                    .map(|f| ComplexFile {
                        path: f.path.clone(),
                        complexity: f.complexity,
                        lines: f.lines,
                        bytes: f.bytes,
                    })
                    .collect(),
            },
            dead_code: self.dead_code.map(|report| {
                let mut largest: Vec<_> = report.findings.iter().collect();
                largest.sort_by(|a, b| {
                    b.lines
                        .cmp(&a.lines)
                        .then(a.path.cmp(&b.path))
                        .then(a.start.line.cmp(&b.start.line))
                });
                DeadCodeView {
                    percentage: report.statistics.percentage,
                    findings: report.findings.len() as u64,
                    files: u64::from(report.statistics.files),
                    by_category: report
                        .statistics
                        .by_category
                        .iter()
                        .map(|c| CategoryFindings {
                            category: c.category.as_str(),
                            count: c.count,
                        })
                        .collect(),
                    largest: largest
                        .iter()
                        .take(self.top)
                        .map(|f| LargeFinding {
                            category: f.category.as_str(),
                            path: f.path.clone(),
                            name: f.name.clone(),
                            line: f.start.line,
                            lines: f.lines,
                        })
                        .collect(),
                }
            }),
        }
    }
}

/// Print the dashboard: the health badge on top, then one section per view.
pub fn print_dashboard(view: &DashboardView, top: usize, elapsed: Duration, style: &Style) {
    crate::health_render::print_badge(&view.health, style);
    println!();

    heading("Project", style);
    let project = &view.project;
    println!(
        "  {} · {} lines · {} tokens · {}",
        plural(project.files, "file"),
        thousands(project.lines),
        thousands(project.tokens),
        plural(project.formats.len() as u64, "format")
    );
    if !project.formats.is_empty() {
        let largest = project
            .formats
            .iter()
            .take(top)
            .map(|f| format!("{} {}", f.format, thousands(f.lines)))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {} {largest} lines", style.dim("largest:"));
    }

    println!();
    heading("Duplication", style);
    let duplication = &view.duplication;
    let kinds = counts(&[
        (duplication.exact, "exact"),
        (duplication.renamed, "renamed"),
        (duplication.similar, "similar"),
    ]);
    println!(
        "  {} duplicated lines · {}{}",
        style.bold(&format!("{:.2}%", duplication.percentage)),
        plural(duplication.clones, "clone"),
        match kinds.is_empty() {
            true => String::new(),
            false => format!(" ({kinds})"),
        }
    );
    if !duplication.files.is_empty() {
        println!("  {}", style.dim("Most duplicated files:"));
        let rows: Vec<Vec<String>> = duplication
            .files
            .iter()
            .map(|f| {
                vec![
                    format!("{:.1}", f.percentage),
                    f.duplicated_lines.to_string(),
                    f.path.clone(),
                ]
            })
            .collect();
        table(&["DUP%", "LINES", "PATH"], 2, &rows, style);
    }

    println!();
    heading("Complexity", style);
    let complexity = &view.complexity;
    println!(
        "  {} total · {:.1} mean per file",
        style.bold(&complexity.total.to_string()),
        complexity.mean
    );
    if !complexity.files.is_empty() {
        println!("  {}", style.dim("Most complex files:"));
        let rows: Vec<Vec<String>> = complexity
            .files
            .iter()
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
    match &view.dead_code {
        None => println!(
            "  {}",
            style.dim("no JavaScript, TypeScript or Python files")
        ),
        Some(dead) => {
            println!(
                "  {} unused lines · {} in {}",
                style.bold(&format!("{:.2}%", dead.percentage)),
                plural(dead.findings, "finding"),
                plural(dead.files, "file")
            );
            let by_category = dead
                .by_category
                .iter()
                .map(|c| format!("{} {}", c.count, c.category))
                .collect::<Vec<_>>()
                .join(" · ");
            if !by_category.is_empty() {
                println!("  {by_category}");
            }
            if !dead.largest.is_empty() {
                println!("  {}", style.dim("Largest findings:"));
                let rows: Vec<Vec<String>> = dead
                    .largest
                    .iter()
                    .map(|f| {
                        let place = match f.name.is_empty() {
                            true => f.path.clone(),
                            false => format!("{}:{} {}", f.path, f.line, f.name),
                        };
                        vec![f.lines.to_string(), f.category.to_string(), place]
                    })
                    .collect();
                table(&["LINES", "CATEGORY", "WHERE"], 1, &rows, style);
            }
        }
    }
    println!();
    println!(
        "{}",
        style.dim(&format!("time: {:.3}ms", elapsed.as_secs_f64() * 1000.0))
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
