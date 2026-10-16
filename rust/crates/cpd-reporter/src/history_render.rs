// history_render.rs — rendering for the opt-in `--history` series: a console
// block, a compact form for the `ai` reporter, and Markdown/HTML sections for
// the health and dashboard reports.
//
// The console block charts the duplication percentage and the health score
// across the series, then prints a table with one row per commit (plus the
// working tree) and the change from the previous point, colored by
// direction, and three highlights: the duplication trend, the health trend
// with the score of each dimension at both ends, and the room left under
// `--threshold`. That last line is what an automatic ratchet would apply;
// jscpd reports it and leaves the decision to the reader.

use crate::badge::health_color;
use crate::health_render::{CSS, ansi_color, escape, label, markdown_cell, markdown_text};
use crate::shared::Style;
use cpd_core::history::{DimensionGap, History, HistoryPoint, sparkline};

const SUBJECT_WIDTH: usize = 48;

/// Rows of the console chart; each row holds eight vertical steps.
const CHART_ROWS: usize = 8;
const AXIS_WIDTH: usize = 7;
const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// How a chart reads its values.
#[derive(Clone, Copy)]
pub(crate) struct Axis {
    /// The y-axis label of a value, `AXIS_WIDTH` characters wide.
    label: fn(f64) -> String,
    /// The smallest span the axis covers, so that a flat series keeps one
    /// step of height.
    min_span: f64,
}

/// Duplication percentages, to a tenth of a point.
const PERCENT: Axis = Axis {
    label: |v| format!("{v:>width$.1}%", width = AXIS_WIDTH - 1),
    min_span: 0.1,
};

/// Health scores, in whole points.
const SCORE: Axis = Axis {
    label: |v| format!("{:>AXIS_WIDTH$}", whole(v)),
    min_span: 1.0,
};

/// A score in whole points, rounded as the health badge prints it (`{:.0}`
/// rounds half to even), so the last point of a series always reads the
/// same as the badge of the same run.
fn whole(score: f64) -> i64 {
    score.round_ties_even() as i64
}

/// Vertical bar chart of the series, top row first, followed by the x-axis
/// and, when columns are wide enough, the point numbers that match the `#`
/// column of the table. The y-axis spans min..max of the series (a series
/// that moves between 2.0% and 2.4% would be a flat line on a 0-based axis)
/// and every point keeps at least one step so the lowest one stays visible.
pub fn render_chart(values: &[f64], rows: usize) -> Vec<String> {
    let values: Vec<Option<f64>> = values.iter().copied().map(Some).collect();
    render_chart_on(&values, rows, PERCENT)
}

/// [`render_chart`] with the labels and span of `axis`. A point with no
/// value keeps its place on the x-axis with an empty column.
pub(crate) fn render_chart_on(values: &[Option<f64>], rows: usize, axis: Axis) -> Vec<String> {
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    if present.is_empty() || rows == 0 {
        return Vec::new();
    }
    let low = present.iter().cloned().fold(f64::INFINITY, f64::min);
    let high = present
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(low + axis.min_span);
    let steps = rows * 8;
    let heights: Vec<usize> = values
        .iter()
        .map(|v| match v {
            None => 0,
            Some(v) => {
                let h = ((v - low) / (high - low) * steps as f64).round() as usize;
                h.clamp(1, steps)
            }
        })
        .collect();
    // Two-character columns with a gap read best; past 20 points switch to
    // one character per point so 30 points still fit an 80-column terminal.
    let (col_width, gap) = if values.len() <= 20 { (2, 1) } else { (1, 0) };
    let blank = " ".repeat(AXIS_WIDTH);

    let mut lines = Vec::with_capacity(rows + 2);
    for row in (0..rows).rev() {
        let base = row * 8;
        let axis_label = if row == rows - 1 {
            format!("{} ┤", (axis.label)(high))
        } else if row == 0 {
            format!("{} ┤", (axis.label)(low))
        } else if row == rows / 2 {
            format!(
                "{} ┤",
                (axis.label)(low + (high - low) * base as f64 / steps as f64)
            )
        } else {
            format!("{blank} │")
        };
        let mut line = axis_label;
        line.push(' ');
        for h in &heights {
            let filled = h.saturating_sub(base).min(8);
            line.extend(std::iter::repeat_n(EIGHTHS[filled], col_width));
            line.extend(std::iter::repeat_n(' ', gap));
        }
        lines.push(line.trim_end().to_string());
    }
    let width = values.len() * (col_width + gap);
    lines.push(format!("{blank} └{}", "─".repeat(width + 1)));
    if col_width == 2 {
        let mut line = format!("{blank}   ");
        for i in 0..values.len() {
            line.push_str(&format!("{:<3}", i + 1));
        }
        lines.push(line.trim_end().to_string());
    }
    lines
}

fn truncate(text: &str, width: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(width).collect();
    if chars.next().is_some() {
        let mut shortened: String = head.chars().take(width.saturating_sub(1)).collect();
        shortened.push('…');
        shortened
    } else {
        head
    }
}

fn format_change(change: Option<f64>) -> String {
    match change {
        None => String::new(),
        Some(c) if c.abs() < 0.05 => "=".to_string(),
        Some(c) => format!("{c:+.1}"),
    }
}

/// More duplication is red, less is green.
fn color_change(text: &str, change: Option<f64>, style: &Style) -> String {
    match change {
        Some(c) if c >= 0.05 => style.red(text),
        Some(c) if c <= -0.05 => style.green_prefix(text),
        Some(_) => style.dim(text),
        None => text.to_string(),
    }
}

/// Change of the health score from the previous point, in the whole points
/// the table prints, so that it always equals the difference of two rows.
fn score_change(history: &History, index: usize) -> Option<i64> {
    let previous = history.points.get(index.checked_sub(1)?)?.health_score()?;
    let current = history.points.get(index)?.health_score()?;
    Some(whole(current) - whole(previous))
}

fn format_score_change(change: Option<i64>) -> String {
    match change {
        None => String::new(),
        Some(0) => "=".to_string(),
        Some(c) => format!("{c:+}"),
    }
}

/// A higher score is green, a lower one red: the other way round from
/// duplication.
fn color_score_change(text: &str, change: Option<i64>, style: &Style) -> String {
    match change {
        Some(c) if c > 0 => style.green_prefix(text),
        Some(c) if c < 0 => style.red(text),
        Some(_) => style.dim(text),
        None => text.to_string(),
    }
}

/// `74 B`, `n/a` for a point with no code to score, or empty for one that
/// was not scored at all.
fn health_text(point: &HistoryPoint) -> String {
    match &point.health {
        None => String::new(),
        Some(health) => match (health.score, health.grade) {
            (Some(score), Some(grade)) => format!("{} {grade}", whole(score)),
            _ => "n/a".to_string(),
        },
    }
}

/// [`health_text`], marked `*` when the point was scored from other
/// dimensions than the latest one (see [`gap_note`]).
fn health_cell(history: &History, index: usize) -> String {
    let text = health_text(&history.points[index]);
    match history.dimension_gap(index).is_empty() {
        true => text,
        false => format!("{text}*"),
    }
}

/// The note that explains the `*` of [`health_cell`]: which dimensions
/// were scored at the marked points and not at the latest one, or the
/// other way round. `None` when no point is marked.
fn gap_note(history: &History) -> Option<String> {
    let gaps: Vec<DimensionGap> = (0..history.points.len())
        .map(|i| history.dimension_gap(i))
        .filter(|gap| !gap.is_empty())
        .collect();
    if gaps.is_empty() {
        return None;
    }
    let names = |ids: Vec<&String>| {
        let mut seen: Vec<&String> = Vec::new();
        for id in ids {
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
        seen.iter()
            .map(|id| label(id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let missing: Vec<&String> = gaps.iter().flat_map(|g| &g.missing).collect();
    let extra: Vec<&String> = gaps.iter().flat_map(|g| &g.extra).collect();
    Some(match (missing.is_empty(), extra.is_empty()) {
        (false, true) => format!(
            "* not scored on {}, unlike the latest point",
            names(missing)
        ),
        (true, false) => format!("* scored on {} too, unlike the latest point", names(extra)),
        _ => format!(
            "* scored on other dimensions than the latest point ({})",
            names(missing.into_iter().chain(extra).collect())
        ),
    })
}

/// The health trend from the first scored point to the latest, with the
/// score of each dimension at both ends: `-11 since 8a9421d (2020-01-07),
/// 62 C → 51 D: duplication 63 → 29, complexity 52 → 48`. `arrow` joins
/// the two ends. Also returns the change, in whole points.
fn health_trend_text(history: &History, arrow: &str) -> Option<(String, i64)> {
    let (first, last) = history.health_trend()?;
    let (first_health, last_health) = (first.health.as_ref()?, last.health.as_ref()?);
    let change = whole(last_health.score?) - whole(first_health.score?);
    let mut ids: Vec<&str> = last_health
        .dimensions
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    for dimension in &first_health.dimensions {
        if !ids.contains(&dimension.id.as_str()) {
            ids.push(&dimension.id);
        }
    }
    let score = |health: &cpd_core::history::PointHealth, id: &str| {
        health
            .dimension(id)
            .map_or_else(|| "n/a".to_string(), |d| whole(d.score).to_string())
    };
    let dimensions = ids
        .iter()
        .map(|id| {
            format!(
                "{} {} {arrow} {}",
                label(id),
                score(first_health, id),
                score(last_health, id)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let moved = match change {
        0 => "unchanged".to_string(),
        change => format_score_change(Some(change)),
    };
    let text = format!(
        "{moved} since {} ({}), {} {arrow} {}: {dimensions}",
        first.short,
        first.date,
        health_text(first),
        health_text(last),
    );
    Some((text, change))
}

/// The health scores of the series in the whole points the table prints,
/// so that a chart never shows a drift that the rows call `=`.
fn whole_scores(history: &History) -> Vec<Option<f64>> {
    history
        .health_scores()
        .into_iter()
        .map(|score| score.map(|score| whole(score) as f64))
        .collect()
}

/// A sparkline of the health scores there are.
fn score_sparkline(history: &History) -> String {
    let scores: Vec<f64> = whole_scores(history).into_iter().flatten().collect();
    sparkline(&scores)
}

/// The commits of the series: `12 commits`, or `30 of 412 commits` when
/// it samples the range.
fn commits(history: &History) -> String {
    let commits = history.commit_count();
    match history.sampled_from {
        Some(total) if total > commits => format!("{commits} of {total} commits"),
        _ => format!("{commits} commit{}", if commits == 1 { "" } else { "s" }),
    }
}

/// What the series covers: `v5.0.0..HEAD: 12 commits + working tree`.
pub(crate) fn scope(history: &History) -> String {
    format!("{}: {} + working tree", history.range, commits(history))
}

/// Lowest and highest of the values that are there.
fn bounds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
}

/// Full console rendering, appended after the normal reporter output.
pub fn print_history(history: &History, style: &Style) {
    println!();
    println!(
        "{} {}",
        style.bold("History"),
        style.dim(&format!("({})", scope(history)))
    );
    print_series(history, style);
}

/// [`print_history`] without its title: the charts, the table and the
/// highlights, for a report that gives the series a heading of its own.
pub(crate) fn print_series(history: &History, style: &Style) {
    if history.points.is_empty() {
        return;
    }

    let pct = history.percentages();
    let (min, max) = bounds(pct.iter().copied());
    let now = pct[pct.len() - 1];
    println!(
        "  {}",
        style.dim(&format!(
            "duplicated lines, % of all lines: min {min:.1}%  max {max:.1}%  now {now:.1}%"
        ))
    );
    for line in render_chart(&pct, CHART_ROWS) {
        println!("  {line}");
    }
    if history.has_health() {
        let scores = whole_scores(history);
        let (min, max) = bounds(scores.iter().flatten().copied());
        let now = history.points.last().map(health_text).unwrap_or_default();
        println!();
        println!(
            "  {}",
            style.dim(&format!(
                "health, 0-100: min {}  max {}  now {now}",
                whole(min),
                whole(max)
            ))
        );
        for line in render_chart_on(&scores, CHART_ROWS, SCORE) {
            println!("  {line}");
        }
    }
    println!();
    print_table(history, style);
    for (text, tone) in highlights(history, "→") {
        let text = match tone {
            Tone::Duplication(change) => color_change(&text, Some(change), style),
            Tone::Health(change) => color_score_change(&text, Some(change), style),
            Tone::Note => style.dim(&text),
            Tone::Headroom => style.bold_green(&text),
        };
        println!("{text}");
    }
}

/// How the console colors a highlight.
enum Tone {
    /// By the direction of the duplication trend.
    Duplication(f64),
    /// By the direction of the health trend.
    Health(i64),
    Note,
    Headroom,
}

/// The highlights under a table: the duplication trend, the health trend,
/// the note on marked scores and the headroom under `--threshold`.
fn highlights(history: &History, arrow: &str) -> Vec<(String, Tone)> {
    let mut lines = Vec::new();
    if let Some(change) = history.overall_change() {
        let first = &history.points[0];
        let moved = match format_change(Some(change)).as_str() {
            "=" => "unchanged".to_string(),
            points => format!("{points} points"),
        };
        lines.push((
            format!("Trend: {moved} since {} ({})", first.short, first.date),
            Tone::Duplication(change),
        ));
    }
    if let Some((text, change)) = health_trend_text(history, arrow) {
        lines.push((format!("Health: {text}"), Tone::Health(change)));
    }
    if let Some(note) = gap_note(history) {
        lines.push((note, Tone::Note));
    }
    if let (Some(threshold), Some(headroom)) = (history.threshold, history.threshold_headroom()) {
        let now = history.points[history.points.len() - 1].percentage;
        lines.push((
            format!(
                "Threshold {threshold:.1}% has {headroom:.1} points of headroom: the series never needed it, tighten it with --threshold {now:.1}"
            ),
            Tone::Headroom,
        ));
    }
    lines
}

/// Which way a column is aligned, and how its cells are colored.
#[derive(Clone, Copy, PartialEq)]
enum Column {
    /// Right-aligned numbers.
    Number,
    /// Left-aligned commit and date.
    Text,
    /// The change of the duplication percentage.
    Change,
    /// The health score, in the color of its grade.
    Health,
    /// The change of the health score.
    HealthChange,
    /// The commit subject, last and unpadded.
    Subject,
}

/// One row per point: its totals, the change from the previous point and,
/// when the series was scored, its health and the change of that.
fn print_table(history: &History, style: &Style) {
    use Column::*;
    let mut columns = vec![
        ("#", Number),
        ("COMMIT", Text),
        ("DATE", Text),
        ("FILES", Number),
        ("LINES", Number),
        ("CLONES", Number),
        ("DUP LINES", Number),
        ("DUP%", Number),
        ("CHANGE", Change),
    ];
    let with_health = history.has_health();
    if with_health {
        columns.extend([("HEALTH", Health), ("CHANGE", HealthChange)]);
    }
    columns.push(("SUBJECT", Subject));
    // A marked score is one character wider; the others make room for the
    // mark so that the grades stay in one column.
    let marked = with_health && gap_note(history).is_some();

    let rows: Vec<Vec<String>> = history
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut row = vec![
                (i + 1).to_string(),
                p.short.clone(),
                p.date.clone(),
                p.sources.to_string(),
                p.lines.to_string(),
                p.clones.to_string(),
                p.duplicated_lines.to_string(),
                format!("{:.1}%", p.percentage),
                format_change(history.change_at(i)),
            ];
            if with_health {
                let mut cell = health_cell(history, i);
                if marked && !cell.ends_with('*') {
                    cell.push(' ');
                }
                row.push(cell);
                row.push(format_score_change(score_change(history, i)));
            }
            row.push(subject(p));
            row
        })
        .collect();

    let mut widths: Vec<usize> = columns.iter().map(|(h, _)| h.len()).collect();
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row.iter()) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let align = |col: usize, cell: &str| -> String {
        let width = widths[col];
        match columns[col].1 {
            Text => format!("{cell:<width$}"),
            Subject => cell.to_string(),
            _ => format!("{cell:>width$}"),
        }
    };
    let header_line = columns
        .iter()
        .enumerate()
        .map(|(col, (h, _))| align(col, h))
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {}", style.dim(header_line.trim_end()));
    for (i, row) in rows.iter().enumerate() {
        let point = &history.points[i];
        let line = row
            .iter()
            .enumerate()
            .map(|(col, cell)| {
                let text = align(col, cell);
                match columns[col].1 {
                    Change => color_change(&text, history.change_at(i), style),
                    Health => match point.health.as_ref().and_then(|h| h.grade) {
                        Some(grade) => style.paint(&text, ansi_color(Some(grade))),
                        None => style.dim(&text),
                    },
                    HealthChange => color_score_change(&text, score_change(history, i), style),
                    Subject if point.is_working_tree() => style.dim(&text),
                    _ => text,
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("  {}", line.trim_end());
    }
}

/// Compact rendering for the `ai` reporter: one line per point, each
/// change right after the value it changes, and the health score with a
/// word of its own so that it never reads as a share of lines.
pub fn print_history_compact(history: &History) {
    for line in compact_lines(history) {
        println!("{line}");
    }
}

/// The lines of [`print_history_compact`].
fn compact_lines(history: &History) -> Vec<String> {
    let with_health = history.has_health();
    let sampled = match history.sampled_from {
        Some(_) => format!(", {}", commits(history)),
        None => String::new(),
    };
    let mut lines = vec![format!(
        "history {}{sampled} (commit/date/files/clones/dup%{}): {}",
        history.range,
        if with_health { ", then health" } else { "" },
        history.sparkline()
    )];
    if with_health && let Some((text, _)) = health_trend_text(history, "->") {
        lines.push(format!(
            "history health {} {text}",
            score_sparkline(history)
        ));
    }
    for (i, p) in history.points.iter().enumerate() {
        let health = match (with_health, p.health_score()) {
            (false, _) => String::new(),
            (true, None) => " health -".to_string(),
            (true, Some(score)) => format!(
                " health {}{}{}",
                whole(score),
                if history.dimension_gap(i).is_empty() {
                    ""
                } else {
                    "*"
                },
                match score_change(history, i) {
                    Some(0) | None => String::new(),
                    Some(change) => format!(" {change:+}"),
                }
            ),
        };
        lines.push(format!(
            "{} {} {}/{}/{:.1}{}{health}",
            p.short,
            p.date,
            p.sources,
            p.clones,
            p.percentage,
            match history.change_at(i) {
                Some(c) if c.abs() >= 0.05 => format!(" {c:+.1}"),
                _ => String::new(),
            }
        ));
    }
    if let Some(note) = gap_note(history) {
        lines.push(note);
    }
    if let (Some(threshold), Some(headroom)) = (history.threshold, history.threshold_headroom()) {
        lines.push(format!("threshold {threshold:.1} headroom {headroom:.1}"));
    }
    lines
}

/// The subject of a point, as a table shows it.
fn subject(point: &HistoryPoint) -> String {
    match point.is_working_tree() {
        true => "(uncommitted changes)".to_string(),
        false => truncate(&point.subject, SUBJECT_WIDTH),
    }
}

/// The series as GitHub-flavoured Markdown, for the health and dashboard
/// reports: a table with one row per point, then the trends.
pub fn markdown_section(history: &History) -> String {
    let with_health = history.has_health();
    let mut md = format!("## History\n\n{}\n\n", markdown_cell(&scope(history)));
    if with_health {
        md.push_str(&format!(
            "Health {} · duplicated lines {}\n\n",
            score_sparkline(history),
            history.sparkline()
        ));
    }
    md.push_str("| # | Commit | Date | Files | Lines | Clones | Dup% | Change |");
    if with_health {
        md.push_str(" Health | Change |");
    }
    md.push_str(" Subject |\n|---:|---|---|---:|---:|---:|---:|---:|");
    if with_health {
        md.push_str("---:|---:|");
    }
    md.push_str("---|\n");
    for (i, p) in history.points.iter().enumerate() {
        md.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {:.1}% | {} |",
            i + 1,
            markdown_cell(&p.short),
            markdown_cell(&p.date),
            p.sources,
            p.lines,
            p.clones,
            p.percentage,
            format_change(history.change_at(i)),
        ));
        if with_health {
            md.push_str(&format!(
                " {} | {} |",
                markdown_text(&health_cell(history, i)),
                format_score_change(score_change(history, i))
            ));
        }
        md.push_str(&format!(" {} |\n", markdown_text(&subject(p))));
    }
    md.push('\n');
    for (line, _) in highlights(history, "→") {
        md.push_str(&format!("{}\n\n", markdown_text(&line)));
    }
    md
}

/// Bars for a series as inline SVG, one per point from 0 up to `top`, each
/// with its point, date and value as a tooltip. The drawing stretches to
/// the width of the page.
fn svg_bars(
    history: &History,
    values: &[Option<f64>],
    top: f64,
    fill: impl Fn(usize) -> &'static str,
    value: impl Fn(f64) -> String,
    label: &str,
) -> String {
    const HEIGHT: f64 = 100.0;
    const STEP: usize = 10;
    let width = (values.len() * STEP).max(STEP);
    let mut svg = format!(
        "<svg class=\"bars\" viewBox=\"0 0 {width} {HEIGHT}\" preserveAspectRatio=\"none\" role=\"img\" aria-label=\"{}\">",
        escape(label)
    );
    for (i, v) in values.iter().enumerate() {
        let Some(v) = *v else { continue };
        let height = (v / top.max(f64::MIN_POSITIVE) * HEIGHT).clamp(1.0, HEIGHT);
        let point = &history.points[i];
        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{:.2}\" width=\"8\" height=\"{height:.2}\" fill=\"{}\"><title>{} {} {}: {}</title></rect>",
            i * STEP + 1,
            HEIGHT - height,
            fill(i),
            i + 1,
            escape(&point.short),
            escape(&point.date),
            escape(&value(v))
        ));
    }
    svg.push_str("</svg>");
    svg
}

/// The series as an HTML fragment, for the health and dashboard reports:
/// bar charts of the health score and the duplication percentage, a table
/// with one row per point, then the trends.
pub fn html_section(history: &History) -> String {
    let with_health = history.has_health();
    let mut html = format!(
        "<h2>History</h2>\n<p class=\"muted\">{}</p>\n",
        escape(&scope(history))
    );
    if with_health {
        let scores = whole_scores(history);
        html.push_str(&format!(
            "<figure class=\"trend\"><figcaption class=\"muted\">Health, 0-100</figcaption>{}</figure>\n",
            svg_bars(
                history,
                &scores,
                100.0,
                |i| health_color(history.points[i].health.as_ref().and_then(|h| h.grade)),
                |v| whole(v).to_string(),
                "Health score per point",
            )
        ));
    }
    let pct: Vec<Option<f64>> = history.percentages().into_iter().map(Some).collect();
    let top = bounds(pct.iter().flatten().copied()).1.max(0.1);
    html.push_str(&format!(
        "<figure class=\"trend\"><figcaption class=\"muted\">Duplicated lines, % of all lines (0-{top:.1}%)</figcaption>{}</figure>\n",
        svg_bars(
            history,
            &pct,
            top,
            |_| "#5b8def",
            |v| format!("{v:.1}%"),
            "Duplicated lines per point",
        )
    ));
    html.push_str("<table>\n<thead><tr><th>#</th><th>Commit</th><th>Date</th><th>Files</th><th>Lines</th><th>Clones</th><th>Dup%</th><th>Change</th>");
    if with_health {
        html.push_str("<th>Health</th><th>Change</th>");
    }
    html.push_str("<th>Subject</th></tr></thead>\n<tbody>\n");
    for (i, p) in history.points.iter().enumerate() {
        html.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.1}%</td><td>{}</td>",
            i + 1,
            escape(&p.short),
            escape(&p.date),
            p.sources,
            p.lines,
            p.clones,
            p.percentage,
            format_change(history.change_at(i)),
        ));
        if with_health {
            html.push_str(&format!(
                "<td>{}</td><td>{}</td>",
                escape(&health_cell(history, i)),
                format_score_change(score_change(history, i))
            ));
        }
        html.push_str(&format!("<td>{}</td></tr>\n", escape(&subject(p))));
    }
    html.push_str("</tbody>\n</table>\n");
    for (line, _) in highlights(history, "→") {
        html.push_str(&format!("<p>{}</p>\n", escape(&line)));
    }
    html
}

/// The style the bar charts of [`html_section`] need, on top of [`CSS`].
pub(crate) const HISTORY_CSS: &str = "figure.trend{margin:.5rem 0 1rem}figure.trend svg.bars{display:block;width:100%;height:100px;background:#fafafa;border-bottom:1px solid #ddd}";

/// [`CSS`] with what [`html_section`] adds.
pub(crate) fn page_css(history: Option<&History>) -> String {
    match history {
        Some(_) => format!("{CSS}{HISTORY_CSS}"),
        None => CSS.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::history::{PointDimension, PointHealth, WORKING_TREE};

    #[test]
    fn chart_has_axis_rows_and_index_labels() {
        let lines = render_chart(&[0.0, 50.0, 100.0], 4);
        assert_eq!(lines.len(), 4 + 2, "{lines:?}");
        assert!(lines[0].starts_with(" 100.0% ┤"), "{}", lines[0]);
        assert!(lines[3].starts_with("   0.0% ┤"), "{}", lines[3]);
        assert!(lines[4].contains("└───"), "{}", lines[4]);
        assert_eq!(lines[5].trim(), "1  2  3");
        // Highest point fills the top row; lowest keeps one step in the bottom row.
        assert!(lines[0].ends_with("██"), "{}", lines[0]);
        assert!(lines[3].contains("▁▁"), "{}", lines[3]);
    }

    #[test]
    fn chart_flat_series_and_dense_columns() {
        let flat = render_chart(&[2.0, 2.0], 4);
        assert!(flat[3].starts_with("   2.0% ┤"), "{}", flat[3]);
        let dense = render_chart(&[1.0; 25], 2);
        assert_eq!(
            dense.len(),
            2 + 1,
            "no index row when columns are one char wide"
        );
        assert!(render_chart(&[], 4).is_empty());
    }

    #[test]
    fn a_score_chart_reads_whole_points_and_leaves_a_gap_for_no_score() {
        let lines = render_chart_on(&[None, Some(61.0), Some(78.4)], 4, SCORE);
        assert!(lines[0].starts_with("     78 ┤"), "{}", lines[0]);
        assert!(lines[3].starts_with("     61 ┤"), "{}", lines[3]);
        // The first column is empty in every row; the others are not.
        assert!(lines[3].starts_with("     61 ┤    ▁▁"), "{}", lines[3]);
        assert_eq!(lines[5].trim(), "1  2  3");
        // A flat score spans one point, so it stays one step high.
        let flat = render_chart_on(&[Some(75.0), Some(75.0)], 2, SCORE);
        assert!(flat[0].starts_with("     76 ┤"), "{}", flat[0]);
        assert!(flat[1].ends_with("▁▁ ▁▁"), "{}", flat[1]);
        assert!(render_chart_on(&[None, None], 4, SCORE).is_empty());
    }

    #[test]
    fn scores_round_as_the_badge_prints_them() {
        for score in [74.5, 75.5, 74.45, 74.55, 0.0, 100.0] {
            assert_eq!(whole(score).to_string(), format!("{score:.0}"), "{score}");
        }
    }

    #[test]
    fn truncate_adds_ellipsis_only_when_cut() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly-ten", 11), "exactly-ten");
        assert_eq!(truncate("a rather long subject line", 10), "a rather …");
    }

    #[test]
    fn change_formatting() {
        assert_eq!(format_change(None), "");
        assert_eq!(format_change(Some(0.0)), "=");
        assert_eq!(format_change(Some(0.04)), "=");
        assert_eq!(format_change(Some(1.26)), "+1.3");
        assert_eq!(format_change(Some(-0.5)), "-0.5");
        assert_eq!(format_score_change(None), "");
        assert_eq!(format_score_change(Some(0)), "=");
        assert_eq!(format_score_change(Some(3)), "+3");
        assert_eq!(format_score_change(Some(-11)), "-11");
    }

    #[test]
    fn change_colors_follow_direction() {
        let style = Style::new(false);
        assert!(color_change("+1.0", Some(1.0), &style).contains("\x1b[31m"));
        assert!(color_change("-1.0", Some(-1.0), &style).contains("\x1b[32m"));
        assert!(color_change("=", Some(0.0), &style).contains("\x1b[90m"));
        assert_eq!(color_change("x", None, &style), "x");
        assert_eq!(color_change("+1.0", Some(1.0), &Style::new(true)), "+1.0");
        // A score is the other way round: up is good.
        assert!(color_score_change("+2", Some(2), &style).contains("\x1b[32m"));
        assert!(color_score_change("-2", Some(-2), &style).contains("\x1b[31m"));
        assert!(color_score_change("=", Some(0), &style).contains("\x1b[90m"));
    }

    fn health(score: Option<f64>, dimensions: &[(&str, f64)]) -> PointHealth {
        PointHealth {
            score,
            grade: score.map(cpd_core::health::grade),
            code_lines: 2000,
            dimensions: dimensions
                .iter()
                .map(|(id, score)| PointDimension {
                    id: id.to_string(),
                    value: Some(2.0),
                    score: *score,
                })
                .collect(),
        }
    }

    fn point(short: &str, percentage: f64, health: Option<PointHealth>) -> HistoryPoint {
        HistoryPoint {
            commit: match short {
                "working" => WORKING_TREE.to_string(),
                _ => format!("{short}000000000000000000000000000000000"),
            },
            short: short.to_string(),
            date: "2026-08-01".to_string(),
            subject: format!("subject of {short}"),
            sources: 3,
            lines: 300,
            tokens: 900,
            clones: 1,
            duplicated_lines: 9,
            percentage,
            health,
        }
    }

    /// Four points: no code yet, then three scored ones, the first of them
    /// without dead code.
    fn series() -> History {
        let full = [
            ("duplication", 80.0),
            ("dead-code", 60.0),
            ("complexity", 70.0),
        ];
        History {
            range: "since 2026-01-01".to_string(),
            sampled_from: None,
            threshold: None,
            points: vec![
                point("a000001", 0.0, Some(health(None, &[]))),
                point(
                    "b000002",
                    4.0,
                    Some(health(
                        Some(74.5),
                        &[("duplication", 82.0), ("complexity", 70.0)],
                    )),
                ),
                point("c000003", 6.5, Some(health(Some(70.2), &full))),
                point("working", 6.5, Some(health(Some(63.0), &full))),
            ],
        }
    }

    #[test]
    fn a_health_cell_is_marked_when_its_dimensions_differ_from_the_latest() {
        let history = series();
        let cells: Vec<String> = (0..4).map(|i| health_cell(&history, i)).collect();
        assert_eq!(cells, ["n/a", "74 B*", "70 B", "63 C"]);
        assert_eq!(
            gap_note(&history).as_deref(),
            Some("* not scored on dead code, unlike the latest point")
        );
        let changes: Vec<Option<i64>> = (0..4).map(|i| score_change(&history, i)).collect();
        assert_eq!(changes, [None, None, Some(-4), Some(-7)]);
    }

    #[test]
    fn the_gap_note_names_extra_and_mixed_dimensions() {
        let mut history = series();
        history.points[1].health = Some(health(
            Some(74.0),
            &[
                ("duplication", 80.0),
                ("dead-code", 60.0),
                ("complexity", 70.0),
                ("coverage", 50.0),
            ],
        ));
        assert_eq!(
            gap_note(&history).as_deref(),
            Some("* scored on coverage too, unlike the latest point")
        );
        history.points[2].health = Some(health(Some(74.0), &[("duplication", 80.0)]));
        assert_eq!(
            gap_note(&history).as_deref(),
            Some(
                "* scored on other dimensions than the latest point (dead code, complexity, coverage)"
            )
        );
    }

    #[test]
    fn a_trend_that_did_not_move_says_so() {
        let mut history = series();
        history.points[1].percentage = 6.5;
        history.points[3].health = history.points[1].health.clone();
        history.points.remove(0);
        let lines: Vec<String> = highlights(&history, "→")
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert_eq!(lines[0], "Trend: unchanged since b000002 (2026-08-01)");
        assert!(
            lines[1].starts_with("Health: unchanged since b000002 (2026-08-01), 74 B → 74 B: "),
            "{lines:?}"
        );
    }

    #[test]
    fn the_health_trend_names_each_dimension_at_both_ends() {
        let (text, change) = health_trend_text(&series(), "→").unwrap();
        assert_eq!(change, -11);
        assert_eq!(
            text,
            "-11 since b000002 (2026-08-01), 74 B → 63 C: duplication 82 → 80, dead code n/a → 60, complexity 70 → 70"
        );
    }

    #[test]
    fn the_highlights_follow_the_table_in_order() {
        let lines: Vec<String> = highlights(&series(), "->")
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        assert_eq!(lines[0], "Trend: +6.5 points since a000001 (2026-08-01)");
        assert!(
            lines[1].starts_with("Health: -11 since b000002"),
            "{lines:?}"
        );
        assert!(lines[1].contains("74 B -> 63 C"), "{lines:?}");
        assert_eq!(
            lines[2],
            "* not scored on dead code, unlike the latest point"
        );
        assert_eq!(lines.len(), 3, "no threshold, no headroom: {lines:?}");
    }

    #[test]
    fn markdown_has_a_health_column_only_for_a_scored_series() {
        let md = markdown_section(&series());
        assert!(
            md.starts_with("## History\n\nsince 2026-01-01: 3 commits + working tree\n\n"),
            "{md}"
        );
        assert!(
            md.contains("| Dup% | Change | Health | Change | Subject |"),
            "{md}"
        );
        assert!(md.contains("| 74 B\\* |  |"), "{md}");
        assert!(md.contains("| 70 B | -4 |"), "{md}");
        assert!(md.contains("Health: -11 since b000002"), "{md}");
        assert!(md.contains("\\* not scored on dead code"), "{md}");

        let mut plain = series();
        for p in &mut plain.points {
            p.health = None;
        }
        let md = markdown_section(&plain);
        assert!(md.contains("| Dup% | Change | Subject |"), "{md}");
        assert!(!md.contains("Health"), "{md}");
    }

    #[test]
    fn markdown_and_html_neutralize_a_hostile_subject() {
        let mut history = series();
        history.points[2].subject = "<script>x</script> | break\n# heading".to_string();
        let md = markdown_section(&history);
        assert!(
            md.contains("&lt;script&gt;x&lt;/script&gt; \\| break # heading"),
            "{md}"
        );
        let html = html_section(&history);
        assert!(html.contains("&lt;script&gt;x&lt;/script&gt;"), "{html}");
        assert!(!html.contains("<script>"), "{html}");
        history.points[2].subject = "`run` *now* [d](u) $5 ~x~ a_b \\n".to_string();
        let md = markdown_section(&history);
        assert!(
            md.contains("| \\`run\\` \\*now\\* \\[d\\](u) \\$5 \\~x\\~ a\\_b \\\\n |"),
            "a subject reads as written: {md}"
        );
    }

    #[test]
    fn html_draws_a_bar_per_scored_point() {
        let html = html_section(&series());
        assert!(html.starts_with("<h2>History</h2>"), "{html}");
        let health_chart = html
            .split("</figure>")
            .next()
            .expect("the health chart comes first");
        assert!(health_chart.contains("Health, 0-100"), "{health_chart}");
        assert_eq!(
            health_chart.matches("<rect").count(),
            3,
            "the point with no code has no bar: {health_chart}"
        );
        assert!(
            health_chart.contains("fill=\"#f1c40f\""),
            "C: {health_chart}"
        );
        assert!(html.contains("<td>74 B*</td><td></td>"), "{html}");
        assert!(page_css(Some(&series())).contains("svg.bars"));
        assert!(!page_css(None).contains("svg.bars"));
    }

    #[test]
    fn the_ai_lines_keep_each_change_beside_its_value() {
        let lines = compact_lines(&series());
        assert_eq!(
            lines[0],
            "history since 2026-01-01 (commit/date/files/clones/dup%, then health): ▁▅██"
        );
        assert!(
            lines[1].starts_with("history health "),
            "a line of its own, not one more health line: {lines:?}"
        );
        assert_eq!(
            &lines[2..],
            [
                "a000001 2026-08-01 3/1/0.0 health -",
                "b000002 2026-08-01 3/1/4.0 +4.0 health 74*",
                "c000003 2026-08-01 3/1/6.5 +2.5 health 70 -4",
                "working 2026-08-01 3/1/6.5 health 63 -7",
                "* not scored on dead code, unlike the latest point",
            ]
        );
        let mut plain = series();
        for p in &mut plain.points {
            p.health = None;
        }
        let lines = compact_lines(&plain);
        assert_eq!(
            lines[0],
            "history since 2026-01-01 (commit/date/files/clones/dup%): ▁▅██"
        );
        assert_eq!(lines[2], "b000002 2026-08-01 3/1/4.0 +4.0");
    }

    #[test]
    fn a_sampled_series_says_how_many_commits_its_range_has() {
        let mut history = series();
        assert_eq!(
            scope(&history),
            "since 2026-01-01: 3 commits + working tree"
        );
        history.sampled_from = Some(412);
        assert_eq!(
            scope(&history),
            "since 2026-01-01: 3 of 412 commits + working tree"
        );
        assert!(
            compact_lines(&history)[0].starts_with("history since 2026-01-01, 3 of 412 commits ("),
            "{:?}",
            compact_lines(&history)
        );
        // A range with no more commits than the series is not sampled.
        history.sampled_from = Some(3);
        assert_eq!(
            scope(&history),
            "since 2026-01-01: 3 commits + working tree"
        );
    }

    #[test]
    fn a_score_chart_moves_only_when_the_rows_do() {
        let mut history = series();
        history.points.remove(0);
        for (point, score) in history.points.iter_mut().zip([71.4, 70.6, 70.6]) {
            point.health.as_mut().unwrap().score = Some(score);
        }
        assert_eq!(
            whole_scores(&history),
            [Some(71.0), Some(71.0), Some(71.0)],
            "every row reads 71"
        );
        assert_eq!(score_sparkline(&history), "▄▄▄");
        let chart = render_chart_on(&whole_scores(&history), 2, SCORE);
        assert!(
            chart[1].ends_with("▁▁ ▁▁ ▁▁"),
            "one height for one score: {chart:?}"
        );
    }
}
