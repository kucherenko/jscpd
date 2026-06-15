use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, extract_lines, read_file_cached, write_report_file};
use cpd_core::models::CpdClone;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;

pub struct JsonReporter {
    blame: bool,
    style: Style,
}

impl JsonReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            blame: opts.blame,
            style: Style::new(opts.no_colors),
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

    let frag_a = read_file_cached(file_cache, &clone.fragment_a.source_id);
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
        write_report_file(
            output_dir,
            "jscpd-report.json",
            &content,
            &self.style,
            "JSON",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{
        empty_ctx, empty_stats, make_clone, make_clone_with_locations, tmp_dir,
    };
    use cpd_core::models::{BlameEntry, Location, Statistics};
    use std::time::Duration;

    #[test]
    fn json_output_is_valid_json() {
        let dir = tmp_dir("json");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = JsonReporter::new(&opts);
        let ctx = empty_ctx();
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("statistics").is_some());
        assert!(parsed.get("duplicates").is_some());
    }

    fn run_json_report(clones: &[CpdClone], blame: bool) -> String {
        let dir = tmp_dir("json");
        let mut opts = ReporterOptions::new(dir.clone());
        opts.blame = blame;
        let reporter = JsonReporter::new(&opts);
        let ctx = empty_ctx();
        reporter.report(clones, &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap()
    }

    #[test]
    fn json_uses_first_file_second_file_keys() {
        let content = run_json_report(&[make_clone("src/a.js", "src/b.js", 42)], false);
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
        let mut clone = make_clone_with_locations("a.js", "b.js", loc.clone(), loc, 50);
        clone.fragment_a.blame = Some(blame);
        let content = run_json_report(&[clone], true);
        assert!(
            content.contains("abc123"),
            "JSON output must contain blame SHA"
        );
    }

    #[test]
    fn json_without_blame_omits_field() {
        let content = run_json_report(&[make_clone("a.js", "b.js", 10)], false);
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

    fn run_json_report_with_stats(clones: &[CpdClone], stats: &Statistics, blame: bool) -> String {
        let dir = tmp_dir("json");
        let mut opts = ReporterOptions::new(dir.clone());
        opts.blame = blame;
        let reporter = JsonReporter::new(&opts);
        let ctx = ReportContext {
            stats,
            duration: Duration::ZERO,
        };
        reporter.report(clones, &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.json")).unwrap()
    }

    fn parse_json_report(content: &str) -> serde_json::Value {
        serde_json::from_str(content).unwrap()
    }

    #[test]
    fn json_duplicate_includes_fragment_field() {
        let content = run_json_report(
            &[make_clone("nonexistent_file.js", "also_nonexistent.js", 10)],
            false,
        );
        let parsed = parse_json_report(&content);
        let dup = &parsed["duplicates"][0];
        assert!(
            dup.get("fragment").is_some(),
            "duplicate must include fragment field"
        );
    }

    #[test]
    fn json_duplicate_includes_start_loc_end_loc() {
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
        let clone =
            make_clone_with_locations("nonexistent.py", "also_nonexistent.py", start, end, 25);
        let content = run_json_report(&[clone], false);
        let parsed = parse_json_report(&content);
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
        let mut stats = empty_stats();
        stats.total.duplicated_lines = 100;
        stats.total.duplicated_tokens = 500;
        let content = run_json_report_with_stats(&[], &stats, false);
        let parsed = parse_json_report(&content);
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
        let content = run_json_report_with_stats(
            &[make_clone("nonexistent.js", "also_nonexistent.js", 42)],
            &empty_stats(),
            false,
        );
        let parsed = parse_json_report(&content);
        let tokens = &parsed["duplicates"][0]["tokens"];
        assert_eq!(tokens.as_i64(), Some(42), "tokens must match token_count");
    }
}
