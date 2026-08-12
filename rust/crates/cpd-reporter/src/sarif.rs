// sarif.rs
// SARIF 2.1.0 reporter: writes jscpd-report.sarif.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, fragment_text, print_saved_report};
use cpd_core::hash::snippet_pair_hash;
use cpd_core::models::CpdClone;
use serde_json::{Value, json};
use std::{collections::HashMap, fs, path::Path};

pub struct SarifReporter {
    blame: bool,
    style: Style,
}

impl SarifReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            blame: opts.blame,
            style: Style::new(opts.no_colors),
        }
    }
}

fn make_region(frag: &cpd_core::models::Fragment) -> Value {
    json!({
        "startLine": frag.start.line,
        "startColumn": frag.start.column + 1,
        "endLine": frag.end.line,
        "endColumn": frag.end.column + 1,
    })
}

fn make_clone_code_hash(
    clone: &CpdClone,
    file_cache: &mut HashMap<String, String>,
) -> Option<String> {
    let snippet_a = fragment_text(file_cache, &clone.fragment_a);
    let snippet_b = fragment_text(file_cache, &clone.fragment_b);
    if snippet_a.is_empty() && snippet_b.is_empty() {
        None
    } else {
        Some(format!(
            "{:016x}",
            snippet_pair_hash(&snippet_a, &snippet_b)
        ))
    }
}

impl Reporter for SarifReporter {
    fn name(&self) -> &str {
        "sarif"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        _ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.sarif");

        let mut seen_uris: Vec<String> = Vec::new();
        let mut file_cache: HashMap<String, String> = HashMap::new();

        let results: Vec<Value> = clones.iter().map(|clone| {
            let uri_a = clone.fragment_a.source_id.clone();
            let uri_b = clone.fragment_b.source_id.clone();

            let idx_a = match seen_uris.iter().position(|u| u == &uri_a) {
                Some(i) => i,
                None => { seen_uris.push(uri_a.clone()); seen_uris.len() - 1 }
            };
            let idx_b = match seen_uris.iter().position(|u| u == &uri_b) {
                Some(i) => i,
                None => { seen_uris.push(uri_b.clone()); seen_uris.len() - 1 }
            };

            let clone_hash = make_clone_code_hash(clone, &mut file_cache);
            let mut props = json!({
                "token_count": clone.token_count,
            });
            if let Some(hash) = &clone_hash {
                props["clone_hash"] = json!(hash);
            }
            if self.blame {
                if let Some(blame) = &clone.fragment_a.blame {
                    props["blame"] = json!({
                        "sha": blame.commit_sha,
                        "author": blame.author,
                        "timestamp": blame.timestamp,
                    });
                }
            }

            let mut result = json!({
                "ruleId": "jscpd/duplicate-code",
                "level": "warning",
                "message": { "text": format!("Duplicated code block ({} tokens)", clone.token_count) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri_a, "index": idx_a },
                        "region": make_region(&clone.fragment_a),
                    }
                }],
                "relatedLocations": [{
                    "id": 0,
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri_b, "index": idx_b },
                        "region": make_region(&clone.fragment_b),
                    }
                }],
                "properties": props,
            });
            // GitHub code scanning and other SARIF consumers use partialFingerprints
            // for result identity across runs; the properties bag alone is opaque to them.
            if let Some(hash) = clone_hash {
                result["partialFingerprints"] = json!({ "jscpdCloneHash/v1": hash });
            }
            result
        }).collect();

        let artifacts: Vec<Value> = seen_uris
            .iter()
            .map(|uri| {
                json!({
                    "location": { "uri": uri },
                })
            })
            .collect();

        let sarif = json!({
            "version": "2.1.0",
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "jscpd",
                        "version": "5.0.3",
                        "informationUri": "https://github.com/kucherenko/jscpd/",
                        "rules": [{
                            "id": "jscpd/duplicate-code",
                            "shortDescription": { "text": "Duplicated code detected" },
                            "helpUri": "https://github.com/kucherenko/jscpd/",
                        }]
                    }
                },
                "artifacts": artifacts,
                "results": results,
            }]
        });

        let content = serde_json::to_string_pretty(&sarif)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        fs::write(&path, content)?;
        print_saved_report(&self.style, "SARIF", &path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, empty_stats, tmp_dir};
    use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location};
    use std::time::Duration;

    fn make_clone() -> CpdClone {
        let loc = Location {
            line: 10,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 20,
            column: 0,
            offset: 0,
        };
        let blame = BlameEntry {
            commit_sha: "deadbeef".to_string(),
            author: "Bob".to_string(),
            timestamp: 1_700_000_000,
        };
        CpdClone {
            format: "rust".to_string(),
            fragment_a: Fragment {
                source_id: "src/foo.rs".to_string(),
                start: loc.clone(),
                end: end.clone(),
                range: [0, 100],
                blame: Some(blame),
            },
            fragment_b: Fragment {
                source_id: "src/bar.rs".to_string(),
                start: loc,
                end,
                range: [0, 100],
                blame: None,
            },
            token_count: 80,
        }
    }

    /// Write source fixtures into a fresh temp dir and point the clone's
    /// fragments at them via absolute paths, so tests never touch the
    /// crate's real `src/` tree and parallel tests cannot race on files.
    fn write_test_sources(clone: &mut CpdClone, num_lines: u32) {
        let dir = tmp_dir("sarif-src");
        let lines = (1..=num_lines)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");

        for fragment in [&mut clone.fragment_a, &mut clone.fragment_b] {
            let name = std::path::Path::new(&fragment.source_id)
                .file_name()
                .unwrap()
                .to_owned();
            let path = dir.join(name);
            fs::write(&path, &lines).unwrap();
            fragment.source_id = path.to_string_lossy().into_owned();
        }
    }

    fn run_sarif_report(clones: &[CpdClone], blame: bool) -> String {
        let dir = tmp_dir("sarif");
        let mut opts = ReporterOptions::new(dir.clone());
        opts.blame = blame;
        let reporter = SarifReporter::new(&opts);
        let ctx = empty_ctx();
        reporter.report(clones, &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap()
    }

    #[test]
    fn sarif_version_is_2_1_0() {
        let content = run_sarif_report(&[], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
    }

    #[test]
    fn sarif_output_has_runs_and_results() {
        let content = run_sarif_report(&[make_clone()], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["runs"][0]["results"].is_array());
        assert_eq!(parsed["runs"][0]["results"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn sarif_blame_included_when_flag_set() {
        let content = run_sarif_report(&[make_clone()], true);
        assert!(
            content.contains("deadbeef"),
            "SARIF must include blame SHA when blame=true"
        );
    }

    #[test]
    fn sarif_result_includes_clone_hash_property() {
        let mut clone = make_clone();
        write_test_sources(&mut clone, 25);

        let content = run_sarif_report(&[clone], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let result = &parsed["runs"][0]["results"][0];
        let properties = &result["properties"];
        assert!(
            properties["clone_hash"].is_string(),
            "clone_hash must be present and be a string"
        );
        let hash = properties["clone_hash"].as_str().unwrap();
        assert_eq!(hash.len(), 16, "clone_hash must be 16-char hex string");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "clone_hash must be hex"
        );
        assert_eq!(
            result["partialFingerprints"]["jscpdCloneHash/v1"], *hash,
            "partialFingerprints must carry the same hash"
        );
    }

    #[test]
    fn sarif_clone_hash_is_stable_across_fragment_order() {
        let mut clone = make_clone();
        write_test_sources(&mut clone, 25);
        let mut swapped = clone.clone();
        std::mem::swap(&mut swapped.fragment_a, &mut swapped.fragment_b);

        let extract_hash = |content: &str| -> String {
            let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
            parsed["runs"][0]["results"][0]["properties"]["clone_hash"]
                .as_str()
                .unwrap()
                .to_string()
        };
        let hash_ab = extract_hash(&run_sarif_report(&[clone], false));
        let hash_ba = extract_hash(&run_sarif_report(&[swapped], false));
        assert_eq!(
            hash_ab, hash_ba,
            "clone_hash must not depend on which copy is fragment A"
        );
    }

    #[test]
    fn sarif_result_excludes_clone_hash_when_snippets_empty() {
        let mut clone = make_clone();
        clone.fragment_a.source_id = "nonexistent-a.rs".to_string();
        clone.fragment_b.source_id = "nonexistent-b.rs".to_string();

        let content = run_sarif_report(&[clone], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let properties = &parsed["runs"][0]["results"][0]["properties"];
        assert!(
            properties["clone_hash"].is_null(),
            "clone_hash must not be present when snippet is empty"
        );
    }

    #[test]
    fn sarif_result_includes_token_count_property() {
        let clone = make_clone();
        let content = run_sarif_report(&[clone], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let properties = &parsed["runs"][0]["results"][0]["properties"];

        assert!(
            properties["token_count"].is_number(),
            "token_count must be present and be a number"
        );
        assert_eq!(
            properties["token_count"], 80,
            "token_count must match clone token count"
        );
    }

    #[test]
    fn sarif_reporter_name_is_sarif() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(SarifReporter::new(&opts).name(), "sarif");
    }
}
