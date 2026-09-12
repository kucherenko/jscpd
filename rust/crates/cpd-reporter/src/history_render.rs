// history_render.rs — console rendering for the opt-in `--history` block.
//
// A sparkline of the duplication percentage across the series, a table with
// one row per commit (plus the working tree), the change from the previous
// point colored by direction, and two highlights: the overall trend, and the
// room left under `--threshold`. That last line is what an automatic ratchet
// would apply; jscpd reports it and leaves the decision to the reader.

use crate::shared::Style;
use cpd_core::history::History;

const SUBJECT_WIDTH: usize = 48;

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

fn color_change(text: &str, change: Option<f64>, style: &Style) -> String {
    match change {
        Some(c) if c >= 0.05 => style.red(text),
        Some(c) if c <= -0.05 => style.green_prefix(text),
        Some(_) => style.dim(text),
        None => text.to_string(),
    }
}

/// Full console rendering, appended after the normal reporter output.
pub fn print_history(history: &History, style: &Style) {
    println!();
    println!(
        "{} {}",
        style.bold("History"),
        style.dim(&format!(
            "({}: {} commits + working tree)",
            history.range,
            history.commit_count()
        ))
    );
    if history.points.is_empty() {
        return;
    }

    let pct = history.percentages();
    let min = pct.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = pct.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let now = pct[pct.len() - 1];
    println!(
        "  {}  {}",
        history.sparkline(),
        style.dim(&format!("min {min:.1}%  max {max:.1}%  now {now:.1}%"))
    );

    let headers = [
        "COMMIT",
        "DATE",
        "FILES",
        "LINES",
        "CLONES",
        "DUP LINES",
        "DUP%",
        "CHANGE",
        "SUBJECT",
    ];
    let rows: Vec<[String; 9]> = history
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            [
                p.short.clone(),
                p.date.clone(),
                p.sources.to_string(),
                p.lines.to_string(),
                p.clones.to_string(),
                p.duplicated_lines.to_string(),
                format!("{:.1}%", p.percentage),
                format_change(history.change_at(i)),
                if p.is_working_tree() {
                    "(uncommitted changes)".to_string()
                } else {
                    truncate(&p.subject, SUBJECT_WIDTH)
                },
            ]
        })
        .collect();

    // Left-align the first two and the last column, right-align the numbers.
    let mut widths: [usize; 9] = headers.map(str::len);
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row.iter()) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let align = |i: usize, cell: &str| -> String {
        let width = widths[i];
        match i {
            0 | 1 => format!("{cell:<width$}"),
            8 => cell.to_string(),
            _ => format!("{cell:>width$}"),
        }
    };
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| align(i, h))
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {}", style.dim(header_line.trim_end()));
    for (i, row) in rows.iter().enumerate() {
        let change = history.change_at(i);
        let line = row
            .iter()
            .enumerate()
            .map(|(col, cell)| {
                let text = align(col, cell);
                if col == 7 {
                    color_change(&text, change, style)
                } else if col == 8 && history.points[i].is_working_tree() {
                    style.dim(&text)
                } else {
                    text
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("  {}", line.trim_end());
    }

    if let Some(change) = history.overall_change() {
        let first = &history.points[0];
        let text = format!(
            "Trend: {} points since {} ({})",
            format_change(Some(change)),
            first.short,
            first.date
        );
        println!("{}", color_change(&text, Some(change), style));
    }
    if let (Some(threshold), Some(headroom)) = (history.threshold, history.threshold_headroom()) {
        println!(
            "{}",
            style.bold_green(&format!(
                "Threshold {threshold:.1}% has {headroom:.1} points of headroom: the series never needed it, tighten it with --threshold {now:.1}"
            ))
        );
    }
}

/// Compact rendering for the `ai` reporter: one line per point.
pub fn print_history_compact(history: &History) {
    println!(
        "history {} (commit/date/files/clones/dup%): {}",
        history.range,
        history.sparkline()
    );
    for (i, p) in history.points.iter().enumerate() {
        println!(
            "{} {} {}/{}/{:.1}{}",
            p.short,
            p.date,
            p.sources,
            p.clones,
            p.percentage,
            match history.change_at(i) {
                Some(c) if c.abs() >= 0.05 => format!(" {c:+.1}"),
                _ => String::new(),
            }
        );
    }
    if let (Some(threshold), Some(headroom)) = (history.threshold, history.threshold_headroom()) {
        println!("threshold {threshold:.1} headroom {headroom:.1}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn change_colors_follow_direction() {
        let style = Style::new(false);
        assert!(color_change("+1.0", Some(1.0), &style).contains("\x1b[31m"));
        assert!(color_change("-1.0", Some(-1.0), &style).contains("\x1b[32m"));
        assert!(color_change("=", Some(0.0), &style).contains("\x1b[90m"));
        assert_eq!(color_change("x", None, &style), "x");
        assert_eq!(color_change("+1.0", Some(1.0), &Style::new(true)), "+1.0");
    }
}
