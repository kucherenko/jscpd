// statistics.rs
// Compute Statistics from a set of source files and detected clones.

use cpd_core::models::{CpdClone, SourceFile, StatRow, Statistics};
use std::collections::HashMap;

pub fn compute(sources: &[SourceFile], clones: &[CpdClone]) -> Statistics {
    let total_lines: u64 = sources
        .iter()
        .map(|f| f.tokens.iter().map(|t| t.start.line).max().unwrap_or(0) as u64)
        .sum();
    let total_tokens: u64 = sources.iter().map(|f| f.tokens.len() as u64).sum();

    let duplicated_lines: u64 = clones
        .iter()
        .map(|c| {
            c.fragment_a
                .end
                .line
                .saturating_sub(c.fragment_a.start.line) as u64
        })
        .sum();
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

    // Per-format stats
    let mut formats: HashMap<String, StatRow> = HashMap::new();
    for source in sources {
        let entry = formats
            .entry(source.format.clone())
            .or_insert_with(|| StatRow {
                lines: 0,
                tokens: 0,
                sources: 0,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
                new_duplicated_lines: 0,
                new_clones: 0,
            });
        entry.sources += 1;
        entry.tokens += source.tokens.len() as u64;
        entry.lines += source
            .tokens
            .iter()
            .map(|t| t.start.line)
            .max()
            .unwrap_or(0) as u64;
    }
    for clone in clones {
        if let Some(entry) = formats.get_mut(&clone.format) {
            entry.clones += 1;
            entry.duplicated_lines += clone
                .fragment_a
                .end
                .line
                .saturating_sub(clone.fragment_a.start.line)
                as u64;
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

    use std::time::{SystemTime, UNIX_EPOCH};
    let detection_date = {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        chrono::DateTime::from_timestamp(secs as i64, millis * 1_000_000)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_else(|| format!("{secs}"))
    };

    Statistics {
        total: StatRow {
            lines: total_lines,
            tokens: total_tokens,
            sources: sources.len() as u64,
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
        }
    }

    fn make_clone(format: &str, start_line: u32, end_line: u32, tc: u32) -> CpdClone {
        CpdClone {
            format: format.to_string(),
            fragment_a: Fragment {
                source_id: "a.js".to_string(),
                start: loc(start_line),
                end: loc(end_line),
                range: [0, tc],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "b.js".to_string(),
                start: loc(start_line),
                end: loc(end_line),
                range: [0, tc],
                blame: None,
            },
            token_count: tc,
        }
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
        // 9 lines duplicated out of 200 total => 4.5%
        assert!((stats.total.percentage - 4.5).abs() < 0.01);
    }

    #[test]
    fn per_format_stats_populated() {
        let sources = vec![make_source("a.js", "javascript", 10)];
        let stats = compute(&sources, &[]);
        assert!(stats.formats.contains_key("javascript"));
        assert_eq!(stats.formats["javascript"].sources, 1);
    }
}
