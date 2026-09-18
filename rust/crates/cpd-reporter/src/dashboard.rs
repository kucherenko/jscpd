// dashboard.rs — one static screen with the whole picture (`--dashboard`):
// health, project size, duplication, complexity and dead code. The numbers
// are gathered once into a [`DashboardView`], which the console prints and
// the JSON reporter serializes, so the two can never disagree.

use crate::health_render;
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
    /// Every format with at least one duplicated line, most duplicated first.
    pub formats: Vec<FormatDuplication>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatDuplication {
    pub format: String,
    pub percentage: f64,
    pub duplicated_lines: u64,
    pub clones: u64,
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
    // A cell can be a file path, a format name or a finding name — all from
    // the repository being scanned, not literal jscpd text. A newline in
    // one would print as an extra row with none of the other columns'
    // data, forging a table row that never came from a real scan.
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.replace(['\n', '\r'], " "))
                .collect()
        })
        .collect();
    let last = headers.len() - 1;
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
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
    for row in &rows {
        println!("    {}", line(row.clone()));
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Matches the two decimal places the console already prints this value
/// with (`{:.2}%`), so JSON carries the same number, not the raw
/// `f64` division noise (`49.583333333333336`) behind that formatted text.
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
        // Prose/data formats (markdown, JSON, YAML, ...) and markup formats
        // (HTML, CSS, templates, ...) never count toward the health score's
        // duplication share; a row for them here would describe a number
        // the score does not have.
        let mut duplicated_formats: Vec<FormatDuplication> = self
            .statistics
            .formats
            .iter()
            .filter(|(format, row)| {
                row.duplicated_lines > 0
                    && cpd_core::summary::has_control_flow(format)
                    && !cpd_core::health::is_markup(format)
            })
            .map(|(format, row)| FormatDuplication {
                format: format.clone(),
                percentage: round1(row.percentage),
                duplicated_lines: row.duplicated_lines,
                clones: row.clones,
            })
            .collect();
        duplicated_formats.sort_by(|a, b| {
            b.percentage
                .total_cmp(&a.percentage)
                .then(b.duplicated_lines.cmp(&a.duplicated_lines))
                .then(a.format.cmp(&b.format))
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
                percentage: round2(total.percentage),
                clones: self.clones.len() as u64,
                exact: kind_count(CloneKind::Exact),
                renamed: kind_count(CloneKind::Renamed),
                similar: kind_count(CloneKind::Similar),
                formats: duplicated_formats.into_iter().take(self.top).collect(),
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
                    percentage: round2(report.statistics.percentage),
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
            .map(|f| {
                format!(
                    "{} {}",
                    f.format.replace(['\n', '\r'], " "),
                    thousands(f.lines)
                )
            })
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
    if !duplication.formats.is_empty() {
        println!("  {}", style.dim("By format:"));
        let rows: Vec<Vec<String>> = duplication
            .formats
            .iter()
            .map(|f| {
                vec![
                    format!("{:.1}", f.percentage),
                    f.duplicated_lines.to_string(),
                    f.clones.to_string(),
                    f.format.clone(),
                ]
            })
            .collect();
        table(&["DUP%", "LINES", "CLONES", "FORMAT"], 3, &rows, style);
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

/// A GitHub-flavoured Markdown render of the dashboard: the health section,
/// then one table per section, in the same order the console prints them.
pub fn render_markdown(view: &DashboardView) -> String {
    let mut md = String::new();
    md.push_str("# Project dashboard\n\n## Health\n\n");
    md.push_str(&health_render::markdown_section(&view.health));

    md.push_str("## Project\n\n");
    let project = &view.project;
    md.push_str(&format!(
        "{} · {} lines · {} tokens · {}\n\n",
        plural(project.files, "file"),
        thousands(project.lines),
        thousands(project.tokens),
        plural(project.formats.len() as u64, "format")
    ));
    if !project.formats.is_empty() {
        md.push_str("| Format | Lines |\n|---|---:|\n");
        for f in &project.formats {
            md.push_str(&format!(
                "| {} | {} |\n",
                health_render::markdown_cell(&f.format),
                f.lines
            ));
        }
        md.push('\n');
    }

    md.push_str("## Duplication\n\n");
    let duplication = &view.duplication;
    md.push_str(&format!(
        "**{:.2}%** duplicated lines · {} (exact {}, renamed {}, similar {})\n\n",
        duplication.percentage,
        plural(duplication.clones, "clone"),
        duplication.exact,
        duplication.renamed,
        duplication.similar
    ));
    if !duplication.formats.is_empty() {
        md.push_str("| Dup% | Lines | Clones | Format |\n|---:|---:|---:|---|\n");
        for f in &duplication.formats {
            md.push_str(&format!(
                "| {:.1} | {} | {} | {} |\n",
                f.percentage,
                f.duplicated_lines,
                f.clones,
                health_render::markdown_cell(&f.format)
            ));
        }
        md.push('\n');
    }

    md.push_str("## Complexity\n\n");
    let complexity = &view.complexity;
    md.push_str(&format!(
        "{} total · {:.1} mean per file\n\n",
        complexity.total, complexity.mean
    ));
    if !complexity.files.is_empty() {
        md.push_str("| CX | Lines | Size | Path |\n|---:|---:|---:|---|\n");
        for f in &complexity.files {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                f.complexity,
                f.lines,
                human_size(f.bytes),
                health_render::markdown_cell(&f.path)
            ));
        }
        md.push('\n');
    }

    md.push_str("## Dead code (JavaScript, TypeScript, Python)\n\n");
    match &view.dead_code {
        None => md.push_str("no JavaScript, TypeScript or Python files\n"),
        Some(dead) => {
            md.push_str(&format!(
                "**{:.2}%** unused lines · {} in {}\n\n",
                dead.percentage,
                plural(dead.findings, "finding"),
                plural(dead.files, "file")
            ));
            let by_category = dead
                .by_category
                .iter()
                .map(|c| format!("{} {}", c.count, c.category))
                .collect::<Vec<_>>()
                .join(" · ");
            if !by_category.is_empty() {
                md.push_str(&format!("{by_category}\n\n"));
            }
            if !dead.largest.is_empty() {
                md.push_str("| Lines | Category | Where |\n|---:|---|---|\n");
                for f in &dead.largest {
                    let place = match f.name.is_empty() {
                        true => f.path.clone(),
                        false => format!("{}:{} {}", f.path, f.line, f.name),
                    };
                    md.push_str(&format!(
                        "| {} | {} | {} |\n",
                        f.lines,
                        f.category,
                        health_render::markdown_cell(&place)
                    ));
                }
                md.push('\n');
            }
        }
    }
    md
}

/// The dashboard as a standalone HTML page: the health section, then one
/// table per section, in the same order the console prints them.
pub fn render_html(view: &DashboardView) -> String {
    let mut body = String::new();
    body.push_str("<h1>Project dashboard</h1>\n<h2>Health</h2>\n");
    body.push_str(&health_render::html_section(&view.health));

    body.push_str("<h2>Project</h2>\n");
    let project = &view.project;
    body.push_str(&format!(
        "<p>{} · {} lines · {} tokens · {}</p>\n",
        plural(project.files, "file"),
        thousands(project.lines),
        thousands(project.tokens),
        plural(project.formats.len() as u64, "format")
    ));
    if !project.formats.is_empty() {
        body.push_str("<table>\n<thead><tr><th>Format</th><th>Lines</th></tr></thead>\n<tbody>\n");
        for f in &project.formats {
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>\n",
                health_render::escape(&f.format),
                f.lines
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }

    body.push_str("<h2>Duplication</h2>\n");
    let duplication = &view.duplication;
    body.push_str(&format!(
        "<p><strong>{:.2}%</strong> duplicated lines · {} (exact {}, renamed {}, similar {})</p>\n",
        duplication.percentage,
        plural(duplication.clones, "clone"),
        duplication.exact,
        duplication.renamed,
        duplication.similar
    ));
    if !duplication.formats.is_empty() {
        body.push_str(
            "<table>\n<thead><tr><th>Dup%</th><th>Lines</th><th>Clones</th><th>Format</th></tr></thead>\n<tbody>\n",
        );
        for f in &duplication.formats {
            body.push_str(&format!(
                "<tr><td>{:.1}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                f.percentage,
                f.duplicated_lines,
                f.clones,
                health_render::escape(&f.format)
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }

    body.push_str("<h2>Complexity</h2>\n");
    let complexity = &view.complexity;
    body.push_str(&format!(
        "<p>{} total · {:.1} mean per file</p>\n",
        complexity.total, complexity.mean
    ));
    if !complexity.files.is_empty() {
        body.push_str(
            "<table>\n<thead><tr><th>CX</th><th>Lines</th><th>Size</th><th>Path</th></tr></thead>\n<tbody>\n",
        );
        for f in &complexity.files {
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                f.complexity,
                f.lines,
                human_size(f.bytes),
                health_render::escape(&f.path)
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }

    body.push_str("<h2>Dead code (JavaScript, TypeScript, Python)</h2>\n");
    match &view.dead_code {
        None => body.push_str("<p class=\"muted\">no JavaScript, TypeScript or Python files</p>\n"),
        Some(dead) => {
            body.push_str(&format!(
                "<p><strong>{:.2}%</strong> unused lines · {} in {}</p>\n",
                dead.percentage,
                plural(dead.findings, "finding"),
                plural(dead.files, "file")
            ));
            let by_category = dead
                .by_category
                .iter()
                .map(|c| format!("{} {}", c.count, c.category))
                .collect::<Vec<_>>()
                .join(" · ");
            if !by_category.is_empty() {
                body.push_str(&format!("<p>{}</p>\n", health_render::escape(&by_category)));
            }
            if !dead.largest.is_empty() {
                body.push_str(
                    "<table>\n<thead><tr><th>Lines</th><th>Category</th><th>Where</th></tr></thead>\n<tbody>\n",
                );
                for f in &dead.largest {
                    let place = match f.name.is_empty() {
                        true => f.path.clone(),
                        false => format!("{}:{} {}", f.path, f.line, f.name),
                    };
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                        f.lines,
                        f.category,
                        health_render::escape(&place)
                    ));
                }
                body.push_str("</tbody>\n</table>\n");
            }
        }
    }

    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Project dashboard</title><style>{}</style></head><body>\n{body}</body></html>\n",
        health_render::CSS
    )
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

    /// Matches the console's own `{:.2}%` formatting: JSON must carry the
    /// same two-decimal number, not the raw division noise behind it.
    #[test]
    fn round2_matches_the_consoles_two_decimal_display() {
        let value = 100.0 / 3.0; // 33.333333333333336
        assert_eq!(round2(value), 33.33);
        assert_eq!(format!("{:.2}", value), format!("{:.2}", round2(value)));
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

    #[test]
    fn duplication_by_format_excludes_prose_data_and_markup() {
        use cpd_core::health::{Health, Size};
        use cpd_core::models::StatRow;
        use cpd_core::summary::SummaryMetric;
        use std::collections::HashMap;

        let mut formats = HashMap::new();
        formats.insert(
            "javascript".to_string(),
            StatRow {
                duplicated_lines: 40,
                percentage: 10.0,
                clones: 2,
                ..StatRow::default()
            },
        );
        // json has complexity 0 in the health model: a row here would
        // describe a number the health score does not have.
        formats.insert(
            "json".to_string(),
            StatRow {
                duplicated_lines: 20,
                percentage: 5.0,
                clones: 1,
                ..StatRow::default()
            },
        );
        // markup — the tokenizer's format name for HTML, XML, SVG, … — is
        // code, but its duplication does not count toward the health score
        // either: same reasoning, a different exclusion.
        formats.insert(
            "markup".to_string(),
            StatRow {
                duplicated_lines: 15,
                percentage: 20.0,
                clones: 1,
                ..StatRow::default()
            },
        );
        let statistics = Statistics {
            total: StatRow::default(),
            formats,
            detection_date: "2024-01-01".to_string(),
        };
        let health = Health {
            score: None,
            grade: None,
            size: Size {
                lines: 0,
                files: 0,
                class: "XS",
            },
            dimensions: vec![],
            skipped: vec![],
        };
        let summary = Summary {
            by: SummaryMetric::Complexity,
            total_files: 0,
            total_folders: 0,
            files: vec![],
            folders: vec![],
        };
        let view = Dashboard {
            health: &health,
            statistics: &statistics,
            clones: &[],
            summary: &summary,
            dead_code: None,
            top: 5,
            elapsed: Duration::ZERO,
        }
        .view();
        let names: Vec<&str> = view
            .duplication
            .formats
            .iter()
            .map(|f| f.format.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["javascript"],
            "json is prose/data, html is markup: neither counts toward the health score"
        );
    }

    fn sample_view() -> DashboardView {
        use cpd_core::health::{Health, Size};
        DashboardView {
            health: Health {
                score: None,
                grade: None,
                size: Size {
                    lines: 0,
                    files: 0,
                    class: "XS",
                },
                dimensions: vec![],
                skipped: vec![],
            },
            project: ProjectView {
                files: 1,
                lines: 10,
                tokens: 20,
                formats: vec![FormatLines {
                    format: "javascript".to_string(),
                    lines: 10,
                }],
            },
            duplication: DuplicationView {
                percentage: 0.0,
                clones: 0,
                exact: 0,
                renamed: 0,
                similar: 0,
                formats: vec![],
            },
            complexity: ComplexityView {
                total: 0,
                mean: 0.0,
                files: vec![],
            },
            dead_code: None,
        }
    }

    /// A format name can come from `--formats-names`/`--formats-exts`, and a
    /// complexity file path is whatever the scanned repository named the
    /// file: both are untrusted text that must not break out of a Markdown
    /// table cell or survive as raw HTML.
    #[test]
    fn markdown_escapes_hostile_format_names_and_paths() {
        let mut view = sample_view();
        let hostile = "fmt|`<img src=x onerror=alert(1)>&\ninjected";
        view.project.formats[0].format = hostile.to_string();
        view.duplication.formats.push(FormatDuplication {
            format: hostile.to_string(),
            percentage: 50.0,
            duplicated_lines: 5,
            clones: 1,
        });
        view.complexity.files.push(ComplexFile {
            path: hostile.to_string(),
            complexity: 10,
            lines: 5,
            bytes: 100,
        });

        let md = render_markdown(&view);
        for line in md.lines() {
            assert!(!line.contains("<img"), "raw HTML survived: {line}");
        }
        assert!(md.contains("fmt\\|`&lt;img"), "{md}");
        // The embedded newline did not start a new line of its own: every
        // occurrence of the hostile text keeps "injected" on the same line.
        for line in md.lines().filter(|l| l.contains("fmt")) {
            assert!(line.contains("injected"), "{line}");
        }
    }

    #[test]
    fn markdown_and_html_include_every_section() {
        let view = sample_view();
        let md = render_markdown(&view);
        for heading in [
            "## Health",
            "## Project",
            "## Duplication",
            "## Complexity",
            "## Dead code",
        ] {
            assert!(md.contains(heading), "{md}");
        }
        assert!(md.contains("not scored"), "{md}");

        let html = render_html(&view);
        for heading in [
            "<h2>Health</h2>",
            "<h2>Project</h2>",
            "<h2>Duplication</h2>",
            "<h2>Complexity</h2>",
            "<h2>Dead code",
        ] {
            assert!(html.contains(heading), "{html}");
        }
        assert!(html.contains("<html"), "{html}");
    }
}
