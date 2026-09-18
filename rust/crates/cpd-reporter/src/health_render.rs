// health_render.rs — the project health score as a console badge, a compact
// line for the `ai` reporter, and Markdown/HTML sections; the SVG badge
// lives in `badge.rs` alongside the other badge shapes.

use crate::dashboard::thousands;
use crate::shared::Style;
use cpd_core::health::{Dimension, Health};

const BAR: usize = 24;
const DIM_BAR: usize = 12;

/// One eighth-block per remainder, for a bar that moves in finer steps than
/// one full block per 100/width points.
const EIGHTHS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// Green for A and B, yellow for C and D, red for E; dim when unscored.
fn ansi_color(grade: Option<char>) -> u8 {
    match grade {
        Some('A' | 'B') => 32,
        Some('C' | 'D') => 33,
        Some(_) => 31,
        None => 90,
    }
}

/// `dead-code` reads better as `dead code` in a sentence. Also collapses a
/// newline to a space: an external metric's `id` is a user-supplied string
/// (from `--health-input`), and a raw newline in it would split this into
/// two console rows or, in Markdown, end the table row early.
fn label(id: &str) -> String {
    id.replace('-', " ").replace(['\n', '\r'], " ")
}

/// Up to 6 formats, most-lines first, then `+N more`; a blunt fallback when
/// every code file happened to be markup and there is nothing to list. A
/// format name can come from `--formats-names`/`--formats-exts`, so it is
/// sanitized here rather than trusted the way a built-in name would be —
/// this is the one path console output takes too, not just Markdown/HTML.
fn format_list(formats: &[String]) -> String {
    const MAX: usize = 6;
    if formats.is_empty() {
        return "no code format".to_string();
    }
    let formats: Vec<String> = formats
        .iter()
        .map(|f| f.replace(['\n', '\r'], " "))
        .collect();
    match formats.len() <= MAX {
        true => formats.join(", "),
        false => format!(
            "{} +{} more",
            formats[..MAX].join(", "),
            formats.len() - MAX
        ),
    }
}

/// What a sub-score was computed from, in the dimension's own terms.
fn measured(dimension: &Dimension) -> Option<String> {
    let value = dimension.value?;
    let share = match (dimension.source, dimension.id.as_str()) {
        ("jscpd", "complexity") => format!("{value:.1}% in complex files"),
        ("jscpd", "duplication") => {
            let formats = format_list(&dimension.formats);
            match dimension.excluded.is_empty() {
                true => format!("{value:.1}% in {formats}"),
                false => format!(
                    "{value:.1}% in {formats} (no {})",
                    dimension.excluded.join("/")
                ),
            }
        }
        ("jscpd", _) => format!("{value:.1}%"),
        _ => format!("{value}"),
    };
    Some(match dimension.coverage {
        Some(coverage) => format!("{share}, reads {coverage:.0}% of the code"),
        None => share,
    })
}

/// A 0-100 score split into a whole-block count and, when the score does not
/// land on a block boundary, the partial eighth-block that follows it.
fn ticks(score: f64, width: usize) -> (usize, Option<char>) {
    let eighths = (score.clamp(0.0, 100.0) / 100.0 * width as f64 * 8.0).round() as usize;
    let full = (eighths / 8).min(width);
    let remainder = if full < width { eighths % 8 } else { 0 };
    (full, (remainder > 0).then(|| EIGHTHS[remainder]))
}

/// A coloured gauge for one score: filled blocks, one partial block, dimmed
/// blocks for the rest.
fn bar(score: f64, width: usize, color: u8, style: &Style) -> String {
    let (full, partial) = ticks(score, width);
    let mut rendered = style.paint(&"█".repeat(full), color);
    let mut used = full;
    if let Some(ch) = partial {
        rendered.push_str(&style.paint(&ch.to_string(), color));
        used += 1;
    }
    rendered.push_str(&style.dim(&"░".repeat(width.saturating_sub(used))));
    rendered
}

/// The badge: a grade chip, score and gauge on the size of the project, then
/// one row per sub-score with its own gauge and what it was measured from.
pub fn print_badge(health: &Health, style: &Style) {
    let color = ansi_color(health.grade);
    let headline = match (health.score, health.grade) {
        (Some(score), Some(grade)) => format!(
            "{} {}  {}",
            style.chip(&format!(" {grade} "), color),
            style.bold(&format!("{score:>3.0}/100")),
            bar(score, BAR, color, style),
        ),
        _ => style.dim("not scored: no code files and no external metrics"),
    };
    println!(
        "{} {headline}  {}",
        style.bold("Health"),
        style.dim(&format!(
            "{} lines of code ({})",
            thousands(health.size.lines),
            health.size.class
        ))
    );
    if health.dimensions.is_empty() && health.skipped.is_empty() {
        return;
    }
    let width = health
        .dimensions
        .iter()
        .map(|d| label(&d.id).chars().count())
        .chain(health.skipped.iter().map(|s| label(s.id).chars().count()))
        .max()
        .unwrap_or(0);
    for d in &health.dimensions {
        let score_color = ansi_color(Some(cpd_core::health::grade(d.score)));
        let from = match measured(d) {
            Some(text) => format!("  {}", style.dim(&text)),
            None => String::new(),
        };
        println!(
            "  {:<width$}  {}  {}{from}",
            label(&d.id),
            style.paint(&format!("{:>3.0}", d.score), score_color),
            bar(d.score, DIM_BAR, score_color, style),
        );
    }
    for s in &health.skipped {
        println!(
            "  {:<width$}  {}",
            label(s.id),
            style.dim(&format!("n/a  ({})", s.reason))
        );
    }
}

/// One line, no padding or colour: the `ai` reporter's form.
pub fn print_compact(health: &Health) {
    let dimensions = health
        .dimensions
        .iter()
        // A newline in an external metric's id would otherwise split this
        // into more than the "one line" the doc comment promises.
        .map(|d| format!("{} {:.0}", d.id.replace(['\n', '\r'], " "), d.score))
        .collect::<Vec<_>>()
        .join(", ");
    match (health.score, health.grade) {
        (Some(score), Some(grade)) => println!(
            "health {score:.0} {grade} ({dimensions}; {} code lines)",
            health.size.lines
        ),
        _ => println!("health n/a (no code files)"),
    }
}

/// Escape the few characters that matter inside an HTML text node or attribute.
pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A value safe to put inside a Markdown table cell or a line of text: HTML
/// is neutralized the same way [`escape`] does it (GitHub renders raw HTML
/// inside Markdown), `|` cannot break out of the cell, and a newline cannot
/// end the row and start a new block — a heading, another table — of its
/// own. File paths, format names and metric ids all reach here from data a
/// scan does not control (a repository's own file names, `--formats-names`,
/// a `--health-input` file), never from a fixed, known-safe string.
pub(crate) fn markdown_cell(text: &str) -> String {
    escape(text).replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// Shared CSS for the health/dashboard HTML reports: a plain, readable page
/// with a coloured grade chip and simple tables, no external stylesheet.
pub(crate) const CSS: &str = "body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;color:#1a1a1a}h1,h2{border-bottom:1px solid #eee;padding-bottom:.3rem}table{border-collapse:collapse;width:100%;margin:.5rem 0 1.5rem}th,td{padding:.4rem .6rem;text-align:left;border-bottom:1px solid #eee}th{color:#666;font-weight:600}.muted{color:#666}.grade{display:inline-block;min-width:1.6rem;text-align:center;border-radius:.3rem;padding:.1rem .5rem;color:#fff;font-weight:700}.grade-A{background:#27ae60}.grade-B{background:#7cb342}.grade-C{background:#f1c40f;color:#1a1a1a}.grade-D{background:#f39c12}.grade-E{background:#e74c3c}.grade-na{background:#9f9f9f}";

/// The health section as GitHub-flavoured Markdown: score line, a table of
/// sub-scores, then what could not be measured.
pub fn markdown_section(health: &Health) -> String {
    let mut md = String::new();
    match (health.score, health.grade) {
        (Some(score), Some(grade)) => md.push_str(&format!(
            "**Score:** {grade} ({score:.0}/100) — {} lines of code ({})\n\n",
            thousands(health.size.lines),
            health.size.class
        )),
        _ => md.push_str("**Score:** not scored (no code files and no external metrics)\n\n"),
    }
    if !health.dimensions.is_empty() {
        md.push_str("| Dimension | Score | Measured |\n|---|---:|---|\n");
        for d in &health.dimensions {
            let from = measured(d).unwrap_or_default();
            md.push_str(&format!(
                "| {} | {:.0} | {} |\n",
                markdown_cell(&label(&d.id)),
                d.score,
                markdown_cell(&from)
            ));
        }
        md.push('\n');
    }
    if !health.skipped.is_empty() {
        let names = health
            .skipped
            .iter()
            .map(|s| format!("{} ({})", label(s.id), s.reason))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!("Not measured: {names}\n\n"));
    }
    md
}

/// `--health -r markdown`: the health section alone, as a standalone document.
pub fn render_markdown(health: &Health) -> String {
    format!("# Project health\n\n{}", markdown_section(health))
}

/// The health section as an HTML fragment: a grade chip, a table of
/// sub-scores, then what could not be measured.
pub fn html_section(health: &Health) -> String {
    let mut html = String::new();
    let (chip, headline) = match (health.score, health.grade) {
        (Some(score), Some(grade)) => (
            format!(
                "<span class=\"grade grade-{grade}\">{grade}</span> <strong>{score:.0}/100</strong>"
            ),
            format!(
                "{} lines of code ({})",
                thousands(health.size.lines),
                health.size.class
            ),
        ),
        _ => (
            "<span class=\"grade grade-na\">n/a</span>".to_string(),
            "not scored: no code files and no external metrics".to_string(),
        ),
    };
    html.push_str(&format!(
        "<p class=\"health-score\">{chip} <span class=\"muted\">{headline}</span></p>\n"
    ));
    if !health.dimensions.is_empty() {
        html.push_str(
            "<table>\n<thead><tr><th>Dimension</th><th>Score</th><th>Measured</th></tr></thead>\n<tbody>\n",
        );
        for d in &health.dimensions {
            let from = measured(d).unwrap_or_default();
            html.push_str(&format!(
                "<tr><td>{}</td><td>{:.0}</td><td>{}</td></tr>\n",
                escape(&label(&d.id)),
                d.score,
                escape(&from)
            ));
        }
        html.push_str("</tbody>\n</table>\n");
    }
    if !health.skipped.is_empty() {
        let names = health
            .skipped
            .iter()
            .map(|s| format!("{} ({})", label(s.id), s.reason))
            .collect::<Vec<_>>()
            .join(", ");
        html.push_str(&format!(
            "<p class=\"muted\">Not measured: {}</p>\n",
            escape(&names)
        ));
    }
    html
}

/// `--health -r html`: the health section alone, as a standalone page.
pub fn render_html(health: &Health) -> String {
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Project health</title><style>{CSS}</style></head><body>\n<h1>Project health</h1>\n{}</body></html>\n",
        html_section(health)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::health::{Size, Skipped};

    fn sample() -> Health {
        Health {
            score: Some(73.4),
            grade: Some('B'),
            size: Size {
                lines: 43_390,
                files: 105,
                class: "M",
            },
            dimensions: vec![Dimension {
                id: "dead-code".to_string(),
                source: "jscpd",
                value: Some(1.2),
                adjusted: Some(1.3),
                lines: Some(520),
                half_life: Some(7.3),
                coverage: None,
                formats: vec![],
                excluded: vec![],
                weight: 1.0,
                score: 88.4,
            }],
            skipped: vec![Skipped {
                id: "complexity",
                reason: "no code files",
            }],
        }
    }

    #[test]
    fn an_unscored_project_prints_without_panicking() {
        let health = Health {
            score: None,
            grade: None,
            ..sample()
        };
        print_badge(&health, &Style::new(true));
        print_compact(&health);
        assert!(render_markdown(&health).contains("not scored"));
        assert!(render_html(&health).contains("grade-na"));
    }

    #[test]
    fn a_built_in_share_is_shown_as_a_percentage() {
        assert_eq!(measured(&sample().dimensions[0]).unwrap(), "1.2%");
        assert_eq!(label("dead-code"), "dead code");
        print_badge(&sample(), &Style::new(false));
        print_compact(&sample());
    }

    #[test]
    fn duplication_names_what_it_was_measured_over() {
        let mut duplication = sample().dimensions[0].clone();
        duplication.id = "duplication".to_string();
        duplication.formats = vec!["javascript".to_string(), "typescript".to_string()];
        duplication.excluded = vec!["markup", "text", "data"];
        assert_eq!(
            measured(&duplication).unwrap(),
            "1.2% in javascript, typescript (no markup/text/data)"
        );
    }

    #[test]
    fn duplication_only_names_categories_this_project_has() {
        let mut duplication = sample().dimensions[0].clone();
        duplication.id = "duplication".to_string();
        duplication.formats = vec!["javascript".to_string()];
        duplication.excluded = vec!["data"];
        assert_eq!(
            measured(&duplication).unwrap(),
            "1.2% in javascript (no data)"
        );

        duplication.excluded = vec![];
        assert_eq!(measured(&duplication).unwrap(), "1.2% in javascript");
    }

    #[test]
    fn format_list_truncates_long_lists() {
        let many: Vec<String> = (0..8).map(|i| format!("lang{i}")).collect();
        assert_eq!(
            format_list(&many),
            "lang0, lang1, lang2, lang3, lang4, lang5 +2 more"
        );
        assert_eq!(format_list(&[]), "no code format");
    }

    /// `--formats-names`/`--formats-exts` let a user name a format anything,
    /// and the console renderer (`print_badge`/`print_compact`) never
    /// passes this through `markdown_cell` the way Markdown/HTML do: it has
    /// to be sanitized here, at the source, to keep a newline from forging
    /// an extra console row on its own (Copilot review, PR #1076).
    #[test]
    fn format_list_collapses_a_newline_in_a_custom_format_name() {
        let hostile = vec!["typescript".to_string(), "evil\nFORGED ROW".to_string()];
        let rendered = format_list(&hostile);
        assert_eq!(rendered.lines().count(), 1, "{rendered}");
        assert!(rendered.contains("evil FORGED ROW"), "{rendered}");
    }

    #[test]
    fn markdown_has_score_and_skipped_dimension() {
        let md = render_markdown(&sample());
        assert!(md.contains("B (73/100)"), "{md}");
        assert!(md.contains("dead code"), "{md}");
        assert!(md.contains("complexity (no code files)"), "{md}");
    }

    #[test]
    fn html_carries_the_grade_and_a_table_row() {
        let html = render_html(&sample());
        assert!(html.contains("grade-B") && html.contains(">B<"), "{html}");
        assert!(html.contains("dead code"), "{html}");
    }

    /// `--health-input` ids are a user's own file, and a duplication
    /// dimension's `formats` can carry a `--formats-names` value: both are
    /// untrusted text a Markdown table must not let break out of a cell.
    #[test]
    fn markdown_escapes_a_hostile_metric_id() {
        let mut health = sample();
        health.dimensions[0].id = "a|b`c<img src=x onerror=alert(1)>&\ninjected".to_string();
        let md = markdown_section(&health);
        // The row stays one row: the embedded newline did not start a new
        // line, so nothing after it can read as its own Markdown block.
        let row = md
            .lines()
            .find(|l| l.contains("b`c"))
            .expect("the id must appear somewhere in one row");
        assert!(!row.contains('\n'), "{row}");
        // `|` cannot introduce a new cell, and raw HTML cannot survive,
        // since GitHub renders HTML embedded in Markdown.
        assert!(row.contains("a\\|b`c"), "{row}");
        assert!(!row.contains("<img"), "{row}");
        assert!(row.contains("&lt;img"), "{row}");
    }

    #[test]
    fn the_bar_uses_a_partial_block_between_full_ones() {
        let (full, partial) = ticks(60.0, 8);
        assert_eq!(full, 4);
        assert_eq!(partial, Some('▊'));
        let (full, partial) = ticks(100.0, 8);
        assert_eq!(full, 8);
        assert_eq!(partial, None);
    }
}
