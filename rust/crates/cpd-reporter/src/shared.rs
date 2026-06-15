// shared.rs — common reporter utilities to keep output formatting DRY.

use cpd_core::models::{CpdClone, Fragment, StatRow, Statistics};
use std::collections::BTreeMap;
use std::collections::HashMap;

/// ANSI terminal styling that respects `--no-colors`.
pub struct Style {
    no_colors: bool,
}

impl Style {
    pub fn new(no_colors: bool) -> Self {
        Self { no_colors }
    }

    pub fn dim(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[90m{}\x1b[39m", text)
        }
    }

    pub fn red(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[31m{}\x1b[39m", text)
        }
    }

    pub fn bold(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m{}\x1b[22m", text)
        }
    }

    pub fn bold_green(&self, text: &str) -> String {
        if self.no_colors {
            text.to_string()
        } else {
            format!("\x1b[1m\x1b[32m{}\x1b[39m\x1b[22m", text)
        }
    }

    pub fn green_prefix(&self, message: &str) -> String {
        if self.no_colors {
            message.to_string()
        } else {
            format!("\x1b[32m{}\x1b[39m", message)
        }
    }
}

pub fn make_row(fmt: &str, row: &StatRow) -> [String; 7] {
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

/// Iterate over format statistics sorted by format name.
///
/// `callback` receives `(format_name, stat_row)` for each known format, followed
/// by `("Total:", total_row)`.
pub fn for_each_sorted_format<F>(stats: &Statistics, mut callback: F)
where
    F: FnMut(&str, &StatRow),
{
    let mut format_names: Vec<&str> = stats.formats.keys().map(|s| s.as_str()).collect();
    format_names.sort();
    for fmt in &format_names {
        if let Some(row) = stats.formats.get(*fmt) {
            callback(fmt, row);
        }
    }
    callback("Total:", &stats.total);
}

/// Prints the statistics table in the jscpd console style.
pub fn print_table(stats: &Statistics, style: &Style, box_chars: BoxChars) {
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

    print_sep(
        &widths,
        box_chars.top_left,
        box_chars.top_mid,
        box_chars.top_right,
        style,
    );
    print_row(
        &headers.map(|h| h.to_string()),
        &widths,
        true,
        style,
        box_chars.vsep,
    );
    for row in &format_rows {
        print_sep(
            &widths,
            box_chars.mid_left,
            box_chars.mid_mid,
            box_chars.mid_right,
            style,
        );
        print_row(row, &widths, false, style, box_chars.vsep);
    }
    print_sep(
        &widths,
        box_chars.mid_left,
        box_chars.mid_mid,
        box_chars.mid_right,
        style,
    );
    print_row(&total_row, &widths, false, style, box_chars.vsep);
    print_sep(
        &widths,
        box_chars.bot_left,
        box_chars.bot_mid,
        box_chars.bot_right,
        style,
    );
}

/// Set of box-drawing characters used by `print_table`.
pub struct BoxChars {
    pub top_left: char,
    pub top_mid: char,
    pub top_right: char,
    pub mid_left: char,
    pub mid_mid: char,
    pub mid_right: char,
    pub bot_left: char,
    pub bot_mid: char,
    pub bot_right: char,
    pub hsep: char,
    pub vsep: char,
}

impl BoxChars {
    /// ASCII-friendly box drawing characters.
    pub fn ascii() -> Self {
        Self {
            top_left: '┌',
            top_mid: '┬',
            top_right: '┐',
            mid_left: '├',
            mid_mid: '┼',
            mid_right: '┤',
            bot_left: '└',
            bot_mid: '┴',
            bot_right: '┘',
            hsep: '─',
            vsep: '│',
        }
    }

    /// Unicode box drawing characters.
    pub fn unicode() -> Self {
        Self {
            top_left: '\u{250c}',
            top_mid: '\u{252c}',
            top_right: '\u{2510}',
            mid_left: '\u{251c}',
            mid_mid: '\u{253c}',
            mid_right: '\u{2524}',
            bot_left: '\u{2514}',
            bot_mid: '\u{2534}',
            bot_right: '\u{2518}',
            hsep: '\u{2500}',
            vsep: '\u{2502}',
        }
    }
}

fn print_sep(widths: &[usize], left: char, mid: char, right: char, style: &Style) {
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
    println!("{}", style.dim(&s));
}

fn print_row(cells: &[String; 7], widths: &[usize], header: bool, style: &Style, vsep: char) {
    let sep = style.dim(&vsep.to_string());
    print!("{}", sep);
    for (i, cell) in cells.iter().enumerate() {
        if header {
            let text = style.red(&format!(" {:width$} ", cell, width = widths[i]));
            print!("{}{}", text, sep);
        } else if i == 0 && cell == "Total:" {
            let padded = format!("{:<width$}", cell, width = widths[i]);
            print!(" {}{} {}", style.bold(cell), &padded[cell.len()..], sep);
        } else {
            print!(" {:width$} {}", cell, sep, width = widths[i]);
        }
    }
    println!();
}

/// Strip a `:format` suffix from a source id so it can be used as a real path.
pub fn clean_source_id(source_id: &str) -> &str {
    match source_id.rfind(':') {
        Some(pos) => &source_id[..pos],
        None => source_id,
    }
}

/// Read the text lines from `content` between `[start_line, end_line]` (1-indexed, inclusive).
pub fn extract_lines(content: &str, start_line: u32, end_line: u32) -> String {
    content
        .lines()
        .skip(start_line.saturating_sub(1) as usize)
        .take(end_line.saturating_sub(start_line.saturating_sub(1)) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Load file contents once per source id. Returns the cached entry when available.
pub fn read_file_cached<'a>(cache: &'a mut HashMap<String, String>, source_id: &str) -> &'a str {
    let clean = clean_source_id(source_id);
    let key = clean.to_string();
    cache
        .entry(key)
        .or_insert_with(|| std::fs::read_to_string(clean).unwrap_or_default())
        .as_str()
}

/// Read the source text for a fragment from disk, if available.
pub fn fragment_text(cache: &mut HashMap<String, String>, fragment: &Fragment) -> String {
    let content = read_file_cached(cache, &fragment.source_id).to_string();
    extract_lines(&content, fragment.start.line, fragment.end.line)
}

/// Print a source snippet for a fragment, with optional color dimming.
pub fn print_snippet(fragment: &Fragment, style: &Style, max_display: usize) {
    let clean_id = clean_source_id(&fragment.source_id);
    let content = match std::fs::read_to_string(clean_id) {
        Ok(c) => c,
        Err(_) => return,
    };

    let start = fragment.start.line.saturating_sub(1) as usize;
    let end = fragment.end.line as usize;

    let lines: Vec<&str> = content.lines().collect();
    let snippet_lines = lines.get(start..end.min(lines.len())).unwrap_or(&[]);

    let truncated = snippet_lines.len() > max_display;
    let display_lines = if truncated {
        &snippet_lines[..max_display]
    } else {
        snippet_lines
    };

    for (i, line) in display_lines.iter().enumerate() {
        let line_num = fragment.start.line as usize + i;
        let prefix = format!("{:>4} │ ", line_num);
        println!("{}{}", style.dim(&prefix), line);
    }
    if truncated {
        let remaining = snippet_lines.len() - max_display;
        println!("{}", style.dim(&format!("     … {} more lines", remaining)));
    }
}

/// Format a clone location summary line, e.g. `file [1:1 - 10:10]`.
pub fn format_location(
    source_id: &str,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
) -> String {
    format!(
        "{} [{}:{} - {}:{}]",
        source_id,
        start_line,
        start_col + 1,
        end_line,
        end_col + 1
    )
}

/// Print a clone header line in console style: `Clone found (format)`.
pub fn print_clone_header(style: &Style, format: &str) {
    println!("{}", style.bold(&format!("Clone found ({})", format)));
}

/// Print the two fragment location lines for a clone (console-full style).
pub fn print_clone_locations(_style: &Style, clone: &CpdClone) {
    let fa = &clone.fragment_a;
    let fb = &clone.fragment_b;
    println!(
        " - {}",
        format_location(
            &fa.source_id,
            fa.start.line,
            fa.start.column,
            fa.end.line,
            fa.end.column
        )
    );
    println!(
        "   {}",
        format_location(
            &fb.source_id,
            fb.start.line,
            fb.start.column,
            fb.end.line,
            fb.end.column
        )
    );
}

/// Build the standard duplication summary sentence used by silent/markdown reporters.
pub fn summary_line(
    clones_len: usize,
    total: &cpd_core::models::StatRow,
    format_count: usize,
) -> String {
    format!(
        "Duplications detection: Found {} exact clones with {}({:.2}%) duplicated lines in {} ({} formats) files.",
        clones_len, total.duplicated_lines, total.percentage, total.sources, format_count,
    )
}

/// Print the trailing "Found N clones" summary line.
pub fn print_found_count(clones: &[CpdClone], style: &Style) {
    if clones.is_empty() {
        println!("{}", style.dim("Found 0 clones."));
    } else {
        println!("{}", style.dim(&format!("Found {} clones.", clones.len())));
    }
}

/// Print the report trailer shared by console-style reporters.
pub fn print_report_trailer(
    clones: &[CpdClone],
    stats: &Statistics,
    style: &Style,
    box_chars: BoxChars,
) {
    print_table(stats, style, box_chars);
    print_found_count(clones, style);
}

/// Print the "No duplicates found." message used by console-style reporters.
pub fn print_no_duplicates(style: &Style) {
    println!("{}", style.dim("No duplicates found."));
}

/// Shared skeleton for console-style reporters.
///
/// Prints the empty-clones message, invokes `render_clone` for each clone, then prints the
/// statistics table and found-count summary.
pub fn report_console_style<F>(
    clones: &[CpdClone],
    stats: &Statistics,
    style: &Style,
    box_chars: BoxChars,
    mut render_clone: F,
) where
    F: FnMut(&CpdClone),
{
    if clones.is_empty() {
        print_no_duplicates(style);
    } else {
        for clone in clones {
            render_clone(clone);
        }
    }
    print_report_trailer(clones, stats, style, box_chars);
}

/// Print a green "X report saved to PATH" message, respecting `--no-colors`.
pub fn print_saved_report(style: &Style, report_name: &str, path: &std::path::Path) {
    let msg = format!("{} report saved to {}", report_name, path.display());
    println!("{}", style.green_prefix(&msg));
}

/// Write serialized report content to `output_dir/<filename>` and print the saved message.
pub fn write_report_file<C: AsRef<[u8]>>(
    output_dir: &std::path::Path,
    filename: &str,
    content: C,
    style: &Style,
    report_name: &str,
) -> Result<std::path::PathBuf, std::io::Error> {
    std::fs::create_dir_all(output_dir)?;
    let path = output_dir.join(filename);
    std::fs::write(&path, content)?;
    print_saved_report(style, report_name, &path);
    Ok(path)
}

/// Build a minimal Statistics total with the given duplication percentage and duplicated lines.
pub fn stats_with_pct(pct: f64, lines: u64) -> Statistics {
    Statistics {
        total: StatRow {
            lines: 100,
            tokens: 500,
            sources: 5,
            clones: 2,
            duplicated_lines: lines,
            duplicated_tokens: 50,
            percentage: pct,
            percentage_tokens: pct,
            new_duplicated_lines: 0,
            new_clones: 0,
        },
        formats: HashMap::new(),
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[cfg(test)]
pub mod fixtures {
    pub use super::stats_with_pct;

    use crate::context::ReportContext;
    use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Generate a test asserting that `Reporter::report` succeeds on an empty clone list.
    #[macro_export]
    macro_rules! assert_empty_report_ok {
        ($test_name:ident, $reporter:ty) => {
            #[test]
            fn $test_name() {
                let output_dir = std::path::PathBuf::from("/tmp");
                let opts = $crate::reporter::ReporterOptions::new(output_dir.clone());
                let reporter = <$reporter>::new(&opts);
                let ctx = $crate::shared::fixtures::empty_ctx();
                assert!(
                    $crate::reporter::Reporter::report(&reporter, &[], &ctx, &output_dir,).is_ok()
                );
            }
        };
    }

    /// Generate a test asserting that the reporter `name()` returns the expected value.
    #[macro_export]
    macro_rules! assert_reporter_name {
        ($test_name:ident, $reporter:ty, $expected:expr) => {
            #[test]
            fn $test_name() {
                let opts = $crate::reporter::ReporterOptions::new(std::path::PathBuf::from("/tmp"));
                let reporter = <$reporter>::new(&opts);
                assert_eq!($crate::reporter::Reporter::name(&reporter), $expected);
            }
        };
    }

    /// Leaked empty statistics for tests that need a `ReportContext<'static>`.
    pub fn empty_ctx() -> ReportContext<'static> {
        let stats = Box::leak(Box::new(empty_stats()));
        ReportContext::new(stats, Duration::ZERO)
    }

    pub fn tmp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    pub fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow::default(),
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    pub fn one_clone_stats() -> Statistics {
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
                new_duplicated_lines: 0,
                new_clones: 0,
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
                new_duplicated_lines: 0,
                new_clones: 0,
            },
            formats,
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    pub fn make_clone(source_a: &str, source_b: &str, token_count: u32) -> CpdClone {
        make_clone_with_lines(source_a, source_b, 1, 10, token_count)
    }

    pub fn make_clone_with_lines(
        source_a: &str,
        source_b: &str,
        start_line: u32,
        end_line: u32,
        token_count: u32,
    ) -> CpdClone {
        let start = Location {
            line: start_line,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: end_line,
            column: 0,
            offset: 100,
        };
        make_clone_with_locations(source_a, source_b, start.clone(), end.clone(), token_count)
    }

    pub fn make_clone_with_locations(
        source_a: &str,
        source_b: &str,
        start: Location,
        end: Location,
        token_count: u32,
    ) -> CpdClone {
        let frag_a = Fragment {
            source_id: source_a.to_string(),
            start: start.clone(),
            end: end.clone(),
            range: [0, 100],
            blame: None,
        };
        let frag_b = Fragment {
            source_id: source_b.to_string(),
            start,
            end,
            range: [0, 100],
            blame: None,
        };
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag_a,
            fragment_b: frag_b,
            token_count,
        }
    }
}
