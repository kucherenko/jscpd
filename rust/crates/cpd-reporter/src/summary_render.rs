// summary_render.rs — console rendering for the opt-in `--summary` block.

use crate::shared::Style;
use cpd_core::summary::{FileSummary, FolderSummary, Summary};

/// Human-readable byte size: 999 → "999", 12_345 → "12.1K", 3_400_000 → "3.2M".
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes}")
    } else if b < KB * KB {
        format!("{:.1}K", b / KB)
    } else {
        format!("{:.1}M", b / (KB * KB))
    }
}

/// Duplication percentage for display, clamped to 100: overlapping clones can
/// push the raw fragment-line sum past the file's line count.
fn dup_percent(duplicated_lines: u64, lines: u64) -> String {
    if lines == 0 {
        "0.0".to_string()
    } else {
        let pct = duplicated_lines as f64 / lines as f64 * 100.0;
        format!("{:.1}", pct.min(100.0))
    }
}

fn file_row(f: &FileSummary) -> [String; 6] {
    [
        f.tokens.to_string(),
        f.lines.to_string(),
        human_size(f.bytes),
        f.complexity.to_string(),
        dup_percent(f.duplicated_lines, f.lines),
        f.path.clone(),
    ]
}

fn folder_row(f: &FolderSummary) -> [String; 6] {
    let mean_cx = if f.files > 0 {
        f.complexity / f.files
    } else {
        0
    };
    [
        f.files.to_string(),
        f.tokens.to_string(),
        f.lines.to_string(),
        human_size(f.bytes),
        mean_cx.to_string(),
        f.path.clone(),
    ]
}

/// Print rows with right-aligned numeric columns and the path last.
fn print_aligned(headers: [&str; 6], rows: &[[String; 6]], style: &Style) {
    let mut widths: [usize; 6] = headers.map(str::len);
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row.iter()) {
            *w = (*w).max(cell.len());
        }
    }
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == 5 {
                h.to_string()
            } else {
                format!("{h:>width$}", width = widths[i])
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {}", style.dim(&header_line));
    for row in rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i == 5 {
                    cell.clone()
                } else {
                    format!("{cell:>width$}", width = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("  {line}");
    }
}

/// Full console rendering, appended after the normal reporter output.
pub fn print_summary(summary: &Summary, style: &Style) {
    println!();
    println!(
        "{} {}",
        style.bold("Summary"),
        style.dim(&format!(
            "(by {}; {} files, {} folders analyzed)",
            summary.by, summary.total_files, summary.total_folders
        ))
    );
    if !summary.files.is_empty() {
        println!("{}", style.bold("Top files:"));
        let rows: Vec<[String; 6]> = summary.files.iter().map(file_row).collect();
        print_aligned(
            ["TOKENS", "LINES", "SIZE", "CX", "DUP%", "PATH"],
            &rows,
            style,
        );
    }
    if !summary.folders.is_empty() {
        println!("{}", style.bold("Top folders:"));
        let rows: Vec<[String; 6]> = summary.folders.iter().map(folder_row).collect();
        print_aligned(
            ["FILES", "TOKENS", "LINES", "SIZE", "CX", "PATH"],
            &rows,
            style,
        );
    }
}

/// Compact rendering for the `ai` reporter: one line per entry, no table
/// padding, minimal punctuation — designed to cost as few LLM tokens as
/// possible while keeping every metric available.
pub fn print_summary_compact(summary: &Summary) {
    println!(
        "Summary by {} ({} files, {} folders):",
        summary.by, summary.total_files, summary.total_folders
    );
    println!("files (tokens/lines/size/cx/dup%):");
    for f in &summary.files {
        println!(
            "{} {}/{}/{}/{}/{}%",
            f.path,
            f.tokens,
            f.lines,
            human_size(f.bytes),
            f.complexity,
            dup_percent(f.duplicated_lines, f.lines),
        );
    }
    println!("folders (files/tokens/lines/size):");
    for f in &summary.folders {
        println!(
            "{} {}/{}/{}/{}",
            f.path,
            f.files,
            f.tokens,
            f.lines,
            human_size(f.bytes),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::summary::SummaryMetric;

    #[test]
    fn human_size_units() {
        assert_eq!(human_size(999), "999");
        assert_eq!(human_size(2048), "2.0K");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0M");
    }

    #[test]
    fn dup_percent_handles_zero_lines() {
        assert_eq!(dup_percent(5, 0), "0.0");
        assert_eq!(dup_percent(25, 100), "25.0");
        assert_eq!(dup_percent(150, 100), "100.0", "clamped at 100");
    }

    fn sample_summary() -> Summary {
        Summary {
            by: SummaryMetric::Tokens,
            files: vec![FileSummary {
                path: "src/a.js".to_string(),
                format: "javascript".to_string(),
                lines: 100,
                tokens: 500,
                bytes: 2048,
                duplicated_lines: 10,
                duplicated_tokens: 50,
                complexity: 7,
            }],
            folders: vec![FolderSummary {
                path: "src".to_string(),
                files: 1,
                lines: 100,
                tokens: 500,
                bytes: 2048,
                duplicated_lines: 10,
                complexity: 7,
            }],
            total_files: 1,
            total_folders: 1,
        }
    }

    #[test]
    fn print_summary_does_not_panic() {
        print_summary(&sample_summary(), &Style::new(true));
    }

    #[test]
    fn print_summary_compact_does_not_panic() {
        print_summary_compact(&sample_summary());
    }

    #[test]
    fn folder_row_uses_mean_complexity() {
        let mut folder = sample_summary().folders.remove(0);
        folder.files = 2;
        folder.complexity = 9;
        assert_eq!(folder_row(&folder)[4], "4");
    }
}
