// statistics.rs
// Compute Statistics from a set of source files and detected clones.

use cpd_core::models::{CpdClone, SourceFile, StatRow, Statistics};
use std::collections::HashMap;

/// Statistics of `sources` with `clones` found among them. The sources are
/// read once, so a caller that keeps them in several places (a server's
/// index holds them per file) passes an iterator instead of a copy.
pub fn compute<'a>(
    sources: impl IntoIterator<Item = &'a SourceFile>,
    clones: &[CpdClone],
) -> Statistics {
    // Per-format stats, and the totals with them.
    let mut formats: HashMap<String, StatRow> = HashMap::new();
    let (mut total_sources, mut total_lines, mut total_tokens) = (0u64, 0u64, 0u64);
    for source in sources {
        let (lines, tokens) = (source.line_count(), source.tokens.len() as u64);
        total_sources += 1;
        total_lines += lines;
        total_tokens += tokens;
        let entry = formats.entry(source.format.clone()).or_default();
        entry.sources += 1;
        entry.tokens += tokens;
        entry.lines += lines;
    }

    // Matched lines of the primary fragment; whatever its span covers without
    // duplicating stays out of the percentage.
    let duplicated_lines: u64 = clones.iter().map(CpdClone::matched_lines).sum();
    let duplicated_tokens: u64 = clones.iter().map(|c| c.token_count as u64).sum();

    let percentage = if total_lines > 0 {
        (duplicated_lines as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };
    let percentage_tokens = if total_tokens > 0 {
        (duplicated_tokens as f64 / total_tokens as f64) * 100.0
    } else {
        0.0
    };

    for clone in clones {
        if let Some(entry) = formats.get_mut(&clone.format) {
            entry.clones += 1;
            entry.duplicated_lines += clone.matched_lines();
            entry.duplicated_tokens += clone.token_count as u64;
        }
    }
    for row in formats.values_mut() {
        if row.lines > 0 {
            row.percentage = (row.duplicated_lines as f64 / row.lines as f64) * 100.0;
        }
        if row.tokens > 0 {
            row.percentage_tokens = (row.duplicated_tokens as f64 / row.tokens as f64) * 100.0;
        }
    }

    let detection_date = now_iso8601();

    Statistics {
        total: StatRow {
            lines: total_lines,
            tokens: total_tokens,
            sources: total_sources,
            clones: clones.len() as u64,
            duplicated_lines,
            duplicated_tokens,
            percentage,
            percentage_tokens,
            new_duplicated_lines: 0,
            new_clones: 0,
        },
        formats,
        detection_date,
    }
}

/// The current UTC time as `2026-11-18T09:30:00.123Z`, the shape of
/// JavaScript's `Date.prototype.toISOString()`.
pub fn now_iso8601() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    iso8601_utc(duration.as_secs(), duration.subsec_millis())
}

/// Format seconds since the Unix epoch, plus milliseconds, as an ISO 8601 UTC
/// timestamp. The date comes from the days-to-civil conversion of the
/// proleptic Gregorian calendar, so no date-time crate is needed for it.
pub fn iso8601_utc(secs: u64, millis: u32) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    // Shift the epoch to 0000-03-01 so a leap day is the last day of a year.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{CpdClone, Fragment, Location, SourceFile, Token, TokenKind};

    fn loc(line: u32) -> Location {
        Location {
            line,
            column: 0,
            offset: 0,
        }
    }

    fn make_token(line: u32) -> Token {
        Token {
            kind: TokenKind::Keyword,
            value: "x".to_string(),
            start: loc(line),
            end: loc(line),
        }
    }

    fn make_source(id: &str, format: &str, lines: u32) -> SourceFile {
        let tokens = (1..=lines).map(make_token).collect();
        SourceFile {
            id: id.to_string(),
            format: format.to_string(),
            tokens,
            bytes: 0,
        }
    }

    fn make_clone(format: &str, start_line: u32, end_line: u32, tc: u32) -> CpdClone {
        CpdClone::exact(
            format.to_string(),
            Fragment {
                source_id: "a.js".to_string(),
                source_root: None,
                start: loc(start_line),
                end: loc(end_line),
                range: [0, tc],
                blame: None,
            },
            Fragment {
                source_id: "b.js".to_string(),
                source_root: None,
                start: loc(start_line),
                end: loc(end_line),
                range: [0, tc],
                blame: None,
            },
            tc,
        )
    }

    #[test]
    fn empty_inputs_produce_zero_stats() {
        let stats = compute(&[], &[]);
        assert_eq!(stats.total.sources, 0);
        assert_eq!(stats.total.clones, 0);
        assert_eq!(stats.total.lines, 0);
        assert_eq!(stats.total.tokens, 0);
        assert_eq!(stats.total.percentage, 0.0);
    }

    #[test]
    fn sources_counted_correctly() {
        let sources = vec![make_source("a.js", "javascript", 10)];
        let stats = compute(&sources, &[]);
        assert_eq!(stats.total.sources, 1);
        assert_eq!(stats.total.tokens, 10);
        assert_eq!(stats.total.clones, 0);
    }

    #[test]
    fn clone_stats_computed() {
        let sources = vec![
            make_source("a.js", "javascript", 100),
            make_source("b.js", "javascript", 100),
        ];
        let clones = vec![make_clone("javascript", 1, 10, 50)];
        let stats = compute(&sources, &clones);
        assert_eq!(stats.total.clones, 1);
        assert_eq!(stats.total.duplicated_tokens, 50);
        // Lines 1 through 10 is ten lines, out of 200 total => 5%
        assert_eq!(stats.total.duplicated_lines, 10);
        assert!((stats.total.percentage - 5.0).abs() < 0.01);
    }

    #[test]
    fn gap_lines_of_a_merged_clone_are_not_duplicated() {
        let sources = vec![
            make_source("a.js", "javascript", 100),
            make_source("b.js", "javascript", 100),
        ];
        let mut merged = make_clone("javascript", 1, 15, 120);
        merged.unmatched_lines = [2, 0];
        let stats = compute(&sources, &[merged]);
        assert_eq!(
            stats.total.duplicated_lines, 13,
            "15-line span minus 2 gap lines"
        );
        assert_eq!(stats.formats["javascript"].duplicated_lines, 13);
    }

    #[test]
    fn per_format_stats_populated() {
        let sources = vec![make_source("a.js", "javascript", 10)];
        let stats = compute(&sources, &[]);
        assert!(stats.formats.contains_key("javascript"));
        assert_eq!(stats.formats["javascript"].sources, 1);
    }
}

#[cfg(test)]
mod iso8601_tests {
    use super::iso8601_utc;

    #[test]
    fn formats_known_instants() {
        let cases = [
            (0, 0, "1970-01-01T00:00:00.000Z"),
            (86_399, 999, "1970-01-01T23:59:59.999Z"),
            (951_782_400, 0, "2000-02-29T00:00:00.000Z"),
            (951_868_800, 0, "2000-03-01T00:00:00.000Z"),
            (1_709_164_800, 7, "2024-02-29T00:00:00.007Z"),
            (1_735_689_599, 0, "2024-12-31T23:59:59.000Z"),
            (1_735_689_600, 0, "2025-01-01T00:00:00.000Z"),
            (1_795_080_645, 123, "2026-11-19T09:30:45.123Z"),
            (4_107_542_400, 0, "2100-03-01T00:00:00.000Z"),
        ];
        for (secs, millis, expected) in cases {
            assert_eq!(iso8601_utc(secs, millis), expected, "secs = {secs}");
        }
    }
}
