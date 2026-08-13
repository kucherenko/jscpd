// sarif.rs
// SARIF 2.1.0 reporter: writes jscpd-report.sarif.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, clean_source_id, fragment_text, print_saved_report};
use cpd_core::hash::snippet_pair_hash;
use cpd_core::models::CpdClone;
use serde_json::{Value, json};
use std::{collections::HashMap, fs, path::Path};

pub struct SarifReporter {
    blame: bool,
    style: Style,
    tool_version: String,
}

impl SarifReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            blame: opts.blame,
            style: Style::new(opts.no_colors),
            tool_version: opts.tool_version.clone(),
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

// Use the full scan-root-relative path, not just the file name: basenames
// like mod.rs or index.ts are ambiguous, and this matches the adjacent
// artifactLocation.uri.
fn make_file_and_line_string(frag: &cpd_core::models::Fragment) -> String {
    format!("{}:{}", clean_source_id(&frag.source_id), frag.start.line)
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

        // Artifact identity: (source_root, cleaned source_id) so that the same
        // relative path under two scan roots gets distinct artifact entries.
        let mut seen_artifacts: Vec<(Option<String>, String)> = Vec::new();
        let mut root_to_base_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        let make_artifact_loc = |frag: &cpd_core::models::Fragment,
                                 seen: &mut Vec<(Option<String>, String)>,
                                 roots: &mut std::collections::HashMap<String, String>|
         -> Value {
            let uri = clean_source_id(&frag.source_id).to_string();
            let key = (frag.source_root.clone(), uri.clone());
            let idx = match seen.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    seen.push(key);
                    seen.len() - 1
                }
            };
            let mut loc = json!({ "uri": uri, "index": idx });
            if let Some(ref root) = frag.source_root {
                let next_id = roots.len();
                let base_id = roots
                    .entry(root.clone())
                    .or_insert_with(|| {
                        if next_id == 0 {
                            "%SRCROOT%".to_string()
                        } else {
                            format!("%SRCROOT{}%", next_id)
                        }
                    })
                    .clone();
                loc["uriBaseId"] = json!(base_id);
            }
            loc
        };
        let mut file_cache: HashMap<String, String> = HashMap::new();

        let results: Vec<Value> = clones.iter().map(|clone| {
            let loc_a = make_artifact_loc(&clone.fragment_a, &mut seen_artifacts, &mut root_to_base_id);
            let loc_b = make_artifact_loc(&clone.fragment_b, &mut seen_artifacts, &mut root_to_base_id);

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
                // The embedded link [text](0) references relatedLocations id 0 —
                // GitHub code scanning only surfaces related locations that the
                // primary message links to this way.
                "message": { "text": format!(
                    "Duplicated code block ({} tokens), duplicated at [{}](0)",
                    clone.token_count,
                    make_file_and_line_string(&clone.fragment_b)
                ) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": loc_a,
                        "region": make_region(&clone.fragment_a),
                    }
                }],
                "relatedLocations": [{
                    "id": 0,
                    "message": { "text": format!("Duplicated at {}", make_file_and_line_string(&clone.fragment_b)) },
                    "physicalLocation": {
                        "artifactLocation": loc_b,
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

        let artifacts: Vec<Value> = seen_artifacts
            .iter()
            .map(|(root, uri)| {
                let mut loc = json!({ "uri": uri });
                if let Some(root) = root {
                    if let Some(base_id) = root_to_base_id.get(root) {
                        loc["uriBaseId"] = json!(base_id);
                    }
                }
                json!({ "location": loc })
            })
            .collect();

        let mut original_uri_base_ids = json!({});
        for (root, base_id) in &root_to_base_id {
            let uri = if root.starts_with('/') {
                format!("file://{}/", root)
            } else {
                let mut s = root.clone();
                if !s.ends_with('\\') && !s.ends_with('/') {
                    s.push('\\');
                }
                s
            };
            original_uri_base_ids[base_id] = json!({ "uri": uri });
        }

        let mut run = json!({
            "tool": {
                "driver": {
                    "name": "jscpd",
                    "version": self.tool_version,
                    "informationUri": "https://github.com/kucherenko/jscpd/",
                    "rules": [{
                        "id": "jscpd/duplicate-code",
                        "name": "jscpd/duplicate-code",
                        "shortDescription": { "text": "Duplicated code detected" },
                        "fullDescription": {
                            "text": "Duplicate sections of code increase the risk of development errors, especially if fixes are made to one code block but not the other. Duplicates that share the same abstractions should be refactored into reusable helper methods as an application of the Don't Repeat Yourself principle."
                        },
                        "defaultConfiguration": {
                            "enabled": true,
                            "level": "warning"
                        },
                        "helpUri": "https://github.com/kucherenko/jscpd/",
                        "properties": {
                            "tags": [ "quality" ],
                            "problem.severity": "warning"
                        }
                    }]
                }
            },
            "artifacts": artifacts,
            "results": results,
        });
        if !root_to_base_id.is_empty() {
            run["originalUriBaseIds"] = original_uri_base_ids;
        }

        let sarif = json!({
            "version": "2.1.0",
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "runs": [run]
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
                source_root: None,
                start: loc.clone(),
                end: end.clone(),
                range: [0, 100],
                blame: Some(blame),
            },
            fragment_b: Fragment {
                source_id: "src/bar.rs".to_string(),
                source_root: None,
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
    fn sarif_related_locations_have_messages_that_reference_the_location() {
        let content = run_sarif_report(&[make_clone()], false);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let result = &parsed["runs"][0]["results"][0];
        let message = &result["relatedLocations"][0]["message"];
        assert!(
            message.is_object(),
            "related location should include a message"
        );
        assert_eq!(
            message["text"], "Duplicated at src/bar.rs:10",
            "related location message should use the full relative path"
        );
        let primary = result["message"]["text"].as_str().unwrap();
        assert!(
            primary.contains("duplicated at [src/bar.rs:10](0)"),
            "primary message must reference relatedLocations id 0 via an embedded link, got: {primary}"
        );
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
    fn sarif_tool_version_comes_from_options() {
        let dir = tmp_dir("sarif");
        let mut opts = ReporterOptions::new(dir.clone());
        opts.tool_version = "9.9.9-test".to_string();
        let reporter = SarifReporter::new(&opts);
        reporter.report(&[], &empty_ctx(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["runs"][0]["tool"]["driver"]["version"], "9.9.9-test",
            "driver.version must come from ReporterOptions.tool_version, not a hardcoded string"
        );
    }

    #[test]
    fn sarif_reporter_name_is_sarif() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(SarifReporter::new(&opts).name(), "sarif");
    }
}
