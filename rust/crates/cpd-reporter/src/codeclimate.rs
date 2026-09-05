// codeclimate.rs — CodeClimate / GitLab Code Quality reporter (issue #958).
// Produces: <output_dir>/gl-code-quality-report.json
//
// The output follows the CodeClimate issue spec, restricted to the subset
// GitLab defines as its Code Quality report format
// (https://docs.gitlab.com/ci/testing/code_quality/#code-quality-report-format),
// which GitLab CI consumes via `artifacts:reports:codequality` to annotate
// merge requests — hence the GitLab-conventional output filename. Unlike the
// SARIF reporter (which GitLab ingests as security vulnerability findings),
// this surfaces duplicates as code quality issues.
//
// Each clone yields two issues — one anchored at each fragment, describing
// the other — so the MR widget annotates the duplication whichever side of
// the pair a merge request touches.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, clean_source_id, clone_pair_hash, write_report_file};
use cpd_core::models::{CpdClone, Fragment};
use serde_json::{Value, json};
use std::{collections::HashMap, path::Path};
use xxhash_rust::xxh3::xxh3_64;

pub struct CodeClimateReporter {
    style: Style,
    threshold: Option<f64>,
}

impl CodeClimateReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
            threshold: opts.threshold,
        }
    }

    /// Severity for one clone, mirroring the SARIF level escalation: "major"
    /// when the clone is new relative to the baseline or when the run as a
    /// whole exceeded the duplication threshold (same strictly-greater
    /// comparison the ThresholdReporter fails the build with); "minor"
    /// otherwise.
    fn severity(&self, clone: &CpdClone, over_threshold: bool) -> &'static str {
        if over_threshold || clone.is_new {
            "major"
        } else {
            "minor"
        }
    }
}

/// Unique, deterministic fingerprint for one issue. GitLab tracks issue
/// identity across pipeline runs by this value, so it must not depend on
/// anything that varies between runs (fragment discovery order, absolute
/// paths). Built from the clone pair's content hash plus the anchored
/// fragment's path and start line — the line keeps the two issues of a
/// self-duplicating file distinct. When the snippets cannot be read the
/// token count stands in for the content hash.
fn fingerprint(clone_hash: Option<&str>, clone: &CpdClone, frag: &Fragment) -> String {
    let identity = match clone_hash {
        Some(hash) => hash.to_string(),
        None => format!("tokens:{}", clone.token_count),
    };
    let composite = format!(
        "{}\0{}\0{}",
        identity,
        clean_source_id(&frag.source_id),
        frag.start.line
    );
    format!("{:016x}", xxh3_64(composite.as_bytes()))
}

fn make_lines(frag: &Fragment) -> Value {
    json!({ "begin": frag.start.line, "end": frag.end.line })
}

fn make_location(frag: &Fragment) -> Value {
    json!({
        "path": clean_source_id(&frag.source_id),
        "lines": make_lines(frag),
    })
}

fn make_issue(
    clone: &CpdClone,
    frag: &Fragment,
    other: &Fragment,
    severity: &str,
    clone_hash: Option<&str>,
) -> Value {
    json!({
        "type": "issue",
        "check_name": if clone.kind.is_renamed() { "jscpd/similar-code" } else { "jscpd/duplicate-code" },
        "description": format!(
            "Duplicated code block ({} tokens), duplicated at {}:{}",
            clone.token_count,
            clean_source_id(&other.source_id),
            other.start.line
        ),
        "categories": ["Duplication"],
        "severity": severity,
        "fingerprint": fingerprint(clone_hash, clone, frag),
        "location": make_location(frag),
        // CodeClimate spec field; GitLab ignores it but other consumers can
        // recover the full pair from a single issue.
        "other_locations": [make_location(other)],
    })
}

impl Reporter for CodeClimateReporter {
    fn name(&self) -> &str {
        "codeclimate"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let over_threshold = self
            .threshold
            .is_some_and(|t| ctx.stats.total.percentage > t);

        let mut file_cache: HashMap<String, String> = HashMap::new();
        let issues: Vec<Value> = clones
            .iter()
            .flat_map(|clone| {
                let severity = self.severity(clone, over_threshold);
                let clone_hash = clone_pair_hash(&mut file_cache, clone);
                let hash = clone_hash.as_deref();
                [
                    make_issue(clone, &clone.fragment_a, &clone.fragment_b, severity, hash),
                    make_issue(clone, &clone.fragment_b, &clone.fragment_a, severity, hash),
                ]
            })
            .collect();

        let content = serde_json::to_string_pretty(&issues)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        write_report_file(
            output_dir,
            "gl-code-quality-report.json",
            &content,
            &self.style,
            "CodeClimate",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, make_clone_with_lines, stats_with_pct, tmp_dir};
    use crate::{assert_empty_report_ok, assert_reporter_name};
    use std::time::Duration;

    assert_reporter_name!(
        codeclimate_reporter_name,
        CodeClimateReporter,
        "codeclimate"
    );
    assert_empty_report_ok!(codeclimate_empty_clones_ok, CodeClimateReporter);

    fn run_codeclimate_report(
        clones: &[CpdClone],
        threshold: Option<f64>,
        total_pct: f64,
    ) -> Value {
        let dir = tmp_dir("codeclimate");
        let mut opts = ReporterOptions::new(dir.clone());
        opts.threshold = threshold;
        let reporter = CodeClimateReporter::new(&opts);
        let stats = stats_with_pct(total_pct, total_pct as u64);
        let ctx = ReportContext {
            stats: &stats,
            duration: Duration::ZERO,
            summary: None,
        };
        reporter.report(clones, &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("gl-code-quality-report.json")).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    fn make_clone() -> CpdClone {
        make_clone_with_lines("src/foo.rs", "src/bar.rs", 10, 20, 80)
    }

    #[test]
    fn codeclimate_empty_clones_writes_empty_array() {
        let dir = tmp_dir("codeclimate");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = CodeClimateReporter::new(&opts);
        reporter.report(&[], &empty_ctx(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("gl-code-quality-report.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, json!([]), "no clones must produce a JSON []");
    }

    #[test]
    fn codeclimate_emits_one_issue_per_fragment() {
        let parsed = run_codeclimate_report(&[make_clone()], None, 0.0);
        let issues = parsed.as_array().unwrap();
        assert_eq!(issues.len(), 2, "each clone yields an issue per fragment");
        assert_eq!(issues[0]["location"]["path"], "src/foo.rs");
        assert_eq!(issues[1]["location"]["path"], "src/bar.rs");
        assert_eq!(issues[0]["other_locations"][0]["path"], "src/bar.rs");
        assert_eq!(issues[1]["other_locations"][0]["path"], "src/foo.rs");
    }

    #[test]
    fn codeclimate_issues_have_required_fields() {
        let parsed = run_codeclimate_report(&[make_clone()], None, 0.0);
        for issue in parsed.as_array().unwrap() {
            assert_eq!(issue["type"], "issue");
            assert_eq!(issue["check_name"], "jscpd/duplicate-code");
            assert!(issue["description"].is_string());
            assert!(issue["fingerprint"].is_string());
            assert!(issue["severity"].is_string());
            assert!(issue["location"]["path"].is_string());
            assert_eq!(issue["location"]["lines"]["begin"], 10);
            assert_eq!(issue["location"]["lines"]["end"], 20);
            assert_eq!(issue["categories"], json!(["Duplication"]));
        }
    }

    #[test]
    fn codeclimate_description_names_the_other_location() {
        let parsed = run_codeclimate_report(&[make_clone()], None, 0.0);
        assert_eq!(
            parsed[0]["description"],
            "Duplicated code block (80 tokens), duplicated at src/bar.rs:10"
        );
        assert_eq!(
            parsed[1]["description"],
            "Duplicated code block (80 tokens), duplicated at src/foo.rs:10"
        );
    }

    #[test]
    fn codeclimate_severity_is_minor_by_default() {
        let parsed = run_codeclimate_report(&[make_clone()], None, 0.0);
        assert_eq!(parsed[0]["severity"], "minor");
        assert_eq!(parsed[1]["severity"], "minor");
    }

    #[test]
    fn codeclimate_new_clone_is_major_severity() {
        let mut new_clone = make_clone();
        new_clone.is_new = true;
        let parsed = run_codeclimate_report(&[new_clone, make_clone()], None, 0.0);
        assert_eq!(parsed[0]["severity"], "major", "new clone must be major");
        assert_eq!(parsed[2]["severity"], "minor", "known clone stays minor");
    }

    #[test]
    fn codeclimate_severity_escalates_when_threshold_exceeded() {
        // Same strictly-greater semantics as the ThresholdReporter: equal to
        // the threshold does not fail the build, so it must not escalate.
        let parsed = run_codeclimate_report(&[make_clone()], Some(20.0), 25.0);
        assert_eq!(parsed[0]["severity"], "major");
        let parsed = run_codeclimate_report(&[make_clone()], Some(20.0), 20.0);
        assert_eq!(parsed[0]["severity"], "minor");
    }

    #[test]
    fn codeclimate_fingerprints_are_distinct_hex_per_issue() {
        let parsed = run_codeclimate_report(&[make_clone()], None, 0.0);
        let fp_a = parsed[0]["fingerprint"].as_str().unwrap();
        let fp_b = parsed[1]["fingerprint"].as_str().unwrap();
        assert_ne!(fp_a, fp_b, "the two issues of a clone must not collide");
        for fp in [fp_a, fp_b] {
            assert_eq!(fp.len(), 16, "fingerprint must be a 16-char hex string");
            assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn codeclimate_fingerprints_are_stable_across_fragment_order() {
        let clone = make_clone();
        let mut swapped = clone.clone();
        std::mem::swap(&mut swapped.fragment_a, &mut swapped.fragment_b);

        let collect_fps = |parsed: &Value| -> std::collections::BTreeSet<String> {
            parsed
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["fingerprint"].as_str().unwrap().to_string())
                .collect()
        };
        let fps_ab = collect_fps(&run_codeclimate_report(&[clone], None, 0.0));
        let fps_ba = collect_fps(&run_codeclimate_report(&[swapped], None, 0.0));
        assert_eq!(
            fps_ab, fps_ba,
            "fingerprint set must not depend on which copy is fragment A"
        );
    }

    #[test]
    fn codeclimate_self_duplication_in_one_file_keeps_issues_distinct() {
        let clone = make_clone_with_lines("src/foo.rs", "src/foo.rs", 10, 20, 80);
        let mut clone = clone;
        clone.fragment_b.start.line = 50;
        clone.fragment_b.end.line = 60;
        let parsed = run_codeclimate_report(&[clone], None, 0.0);
        assert_ne!(
            parsed[0]["fingerprint"], parsed[1]["fingerprint"],
            "same-file clone pairs need per-line fingerprints"
        );
    }

    #[test]
    fn codeclimate_strips_format_suffix_from_paths() {
        let clone = make_clone_with_lines("src/foo.rs:rust", "src/bar.rs:rust", 10, 20, 80);
        let parsed = run_codeclimate_report(&[clone], None, 0.0);
        assert_eq!(parsed[0]["location"]["path"], "src/foo.rs");
        assert_eq!(parsed[0]["other_locations"][0]["path"], "src/bar.rs");
    }
}
