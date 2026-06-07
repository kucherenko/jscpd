use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use cpd_core::models::CpdClone;
use serde_json::json;
use std::collections::HashMap;
use std::{fs, path::Path};

pub struct JsonReporter {
    blame: bool,
    no_colors: bool,
}

impl JsonReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            blame: opts.blame,
            no_colors: opts.no_colors,
        }
    }
}

fn location_to_json(loc: &cpd_core::models::Location) -> serde_json::Value {
    json!({
        "line": loc.line,
        "column": loc.column,
        "position": loc.offset,
    })
}

fn extract_lines(content: &str, start_line: u32, end_line: u32) -> String {
    content
        .lines()
        .skip(start_line.saturating_sub(1) as usize)
        .take(end_line.saturating_sub(start_line.saturating_sub(1)) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

fn clone_to_dup(
    clone: &CpdClone,
    include_blame: bool,
    file_cache: &mut HashMap<String, String>,
) -> serde_json::Value {
    let lines = clone
        .fragment_a
        .end
        .line
        .saturating_sub(clone.fragment_a.start.line)
        + 1;

    let frag_a = file_cache
        .entry(clone.fragment_a.source_id.clone())
        .or_insert_with(|| fs::read_to_string(&clone.fragment_a.source_id).unwrap_or_default());
    let fragment = extract_lines(
        frag_a,
        clone.fragment_a.start.line,
        clone.fragment_a.end.line,
    );

    let mut first_file = json!({
        "name": clone.fragment_a.source_id,
        "start": clone.fragment_a.start.line,
        "end": clone.fragment_a.end.line,
        "startLoc": location_to_json(&clone.fragment_a.start),
        "endLoc": location_to_json(&clone.fragment_a.end),
    });
    let mut second_file = json!({
        "name": clone.fragment_b.source_id,
        "start": clone.fragment_b.start.line,
        "end": clone.fragment_b.end.line,
        "startLoc": location_to_json(&clone.fragment_b.start),
        "endLoc": location_to_json(&clone.fragment_b.end),
    });

    if include_blame {
        if let Some(ref blame) = clone.fragment_a.blame {
            first_file["blame"] = json!({
                "commitSha": blame.commit_sha,
                "author": blame.author,
            });
        }
        if let Some(ref blame) = clone.fragment_b.blame {
            second_file["blame"] = json!({
                "commitSha": blame.commit_sha,
                "author": blame.author,
            });
        }
    }

    json!({
        "format": clone.format,
        "lines": lines,
        "fragment": fragment,
        "tokens": clone.token_count,
        "firstFile": first_file,
        "secondFile": second_file,
    })
}

impl Reporter for JsonReporter {
    fn name(&self) -> &str {
        "json"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.json");

        let mut file_cache: HashMap<String, String> = HashMap::new();
        let duplicates: Vec<serde_json::Value> = clones
            .iter()
            .map(|c| clone_to_dup(c, self.blame, &mut file_cache))
            .collect();

        let value = json!({
            "statistics": ctx.stats,
            "duplicates": duplicates,
        });

        let content = serde_json::to_string_pretty(&value)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        fs::write(&path, content)?;

        let path_display = path.display();
        if self.no_colors {
            println!("JSON report saved to {}", path_display);
        } else {
            println!("\x1b[32mJSON report saved to {}\x1b[39m", path_display);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Duration;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-json-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
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
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn json_output_is_valid_json() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("statistics").is_some());
        assert!(parsed.get("duplicates").is_some());
    }

    #[test]
    fn json_uses_first_file_second_file_keys() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let loc = Location {
            line: 5,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 15,
            column: 0,
            offset: 50,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: Fragment {
                source_id: "src/a.js".to_string(),
                start: loc.clone(),
                end: end.clone(),
                range: [0, 50],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "src/b.js".to_string(),
                start: loc,
                end,
                range: [0, 50],
                blame: None,
            },
            token_count: 42,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        assert!(content.contains("\"firstFile\""), "must use firstFile key");
        assert!(
            content.contains("\"secondFile\""),
            "must use secondFile key"
        );
        assert!(content.contains("\"lines\""), "must include lines field");
        assert!(content.contains("\"tokens\""), "must include tokens field");
    }

    #[test]
    fn json_with_blame_includes_sha() {
        let dir = tmp_dir();
        let mut opts = ReporterOptions::new(dir.clone());
        opts.blame = true;
        let reporter = JsonReporter::new(&opts);
        let blame = BlameEntry {
            commit_sha: "abc123".to_string(),
            author: "Alice".to_string(),
            timestamp: 1_700_000_000,
        };
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: Fragment {
                source_id: "a.js".to_string(),
                start: loc.clone(),
                end: loc.clone(),
                range: [0, 10],
                blame: Some(blame),
            },
            fragment_b: Fragment {
                source_id: "b.js".to_string(),
                start: loc.clone(),
                end: loc,
                range: [0, 10],
                blame: None,
            },
            token_count: 50,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        assert!(
            content.contains("abc123"),
            "JSON output must contain blame SHA"
        );
    }

    #[test]
    fn json_without_blame_omits_field() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let clone = CpdClone {
            format: "js".to_string(),
            fragment_a: Fragment {
                source_id: "a.js".to_string(),
                start: loc.clone(),
                end: loc.clone(),
                range: [0, 5],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "b.js".to_string(),
                start: loc.clone(),
                end: loc,
                range: [0, 5],
                blame: None,
            },
            token_count: 10,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        assert!(
            !content.contains("commitSha"),
            "without blame, should not contain blame fields"
        );
    }

    #[test]
    fn json_reporter_name_is_json() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(JsonReporter::new(&opts).name(), "json");
    }

    #[test]
    fn json_duplicate_includes_fragment_field() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: Fragment {
                source_id: "nonexistent_file.js".to_string(),
                start: loc,
                end,
                range: [0, 5],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "also_nonexistent.js".to_string(),
                start: Location {
                    line: 1,
                    column: 0,
                    offset: 0,
                },
                end: Location {
                    line: 1,
                    column: 0,
                    offset: 0,
                },
                range: [0, 5],
                blame: None,
            },
            token_count: 10,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let dup = &parsed["duplicates"][0];
        assert!(
            dup.get("fragment").is_some(),
            "duplicate must include fragment field"
        );
    }

    #[test]
    fn json_duplicate_includes_start_loc_end_loc() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let start = Location {
            line: 10,
            column: 5,
            offset: 100,
        };
        let end = Location {
            line: 20,
            column: 3,
            offset: 500,
        };
        let clone = CpdClone {
            format: "python".to_string(),
            fragment_a: Fragment {
                source_id: "nonexistent.py".to_string(),
                start: start.clone(),
                end: end.clone(),
                range: [100, 500],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "also_nonexistent.py".to_string(),
                start: start,
                end: end,
                range: [100, 500],
                blame: None,
            },
            token_count: 25,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let first_file = &parsed["duplicates"][0]["firstFile"];
        assert!(
            first_file.get("startLoc").is_some(),
            "firstFile must include startLoc"
        );
        assert!(
            first_file.get("endLoc").is_some(),
            "firstFile must include endLoc"
        );
        let start_loc = &first_file["startLoc"];
        assert_eq!(start_loc["line"], 10);
        assert_eq!(start_loc["column"], 5);
        assert_eq!(start_loc["position"], 100);
    }

    #[test]
    fn json_statistics_uses_camel_case() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let mut stats = empty_stats();
        stats.total.duplicated_lines = 100;
        stats.total.duplicated_tokens = 500;
        let ctx = ReportContext {
            stats: &stats,
            duration: Duration::ZERO,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let total = &parsed["statistics"]["total"];
        assert!(
            total.get("duplicatedLines").is_some(),
            "statistics must use camelCase: duplicatedLines"
        );
        assert!(
            total.get("duplicatedTokens").is_some(),
            "statistics must use camelCase: duplicatedTokens"
        );
        assert!(
            total.get("percentageTokens").is_some(),
            "statistics must use camelCase: percentageTokens"
        );
        assert!(
            total.get("detectionDate").is_some()
                || parsed["statistics"].get("detectionDate").is_some(),
            "statistics must use camelCase: detectionDate"
        );
        assert!(
            total.get("duplicated_lines").is_none(),
            "statistics must NOT use snake_case: duplicated_lines"
        );
        assert!(
            total.get("duplicated_tokens").is_none(),
            "statistics must NOT use snake_case: duplicated_tokens"
        );
        assert!(
            total.get("newDuplicatedLines").is_some(),
            "statistics must include newDuplicatedLines"
        );
        assert!(
            total.get("newClones").is_some(),
            "statistics must include newClones"
        );
    }

    #[test]
    fn json_duplicate_tokens_uses_token_count() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let loc = Location {
            line: 5,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 15,
            column: 0,
            offset: 50,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: Fragment {
                source_id: "nonexistent.js".to_string(),
                start: loc,
                end: end,
                range: [0, 50],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "also_nonexistent.js".to_string(),
                start: Location {
                    line: 5,
                    column: 0,
                    offset: 0,
                },
                end: Location {
                    line: 15,
                    column: 0,
                    offset: 50,
                },
                range: [0, 50],
                blame: None,
            },
            token_count: 42,
        };
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let tokens = &parsed["duplicates"][0]["tokens"];
        assert_eq!(tokens.as_i64(), Some(42), "tokens must match token_count");
    }
}
