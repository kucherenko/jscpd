// sarif.rs
// SARIF 2.1.0 reporter: writes jscpd-report.sarif.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, clean_source_id, print_saved_report};
use cpd_core::models::CpdClone;
use serde_json::{Value, json};
use std::{fs, path::Path};

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

        let results: Vec<Value> = clones.iter().map(|clone| {
            let loc_a = make_artifact_loc(&clone.fragment_a, &mut seen_artifacts, &mut root_to_base_id);
            let loc_b = make_artifact_loc(&clone.fragment_b, &mut seen_artifacts, &mut root_to_base_id);

            let mut props = json!({});
            if self.blame {
                if let Some(blame) = &clone.fragment_a.blame {
                    props["blame"] = json!({
                        "sha": blame.commit_sha,
                        "author": blame.author,
                        "timestamp": blame.timestamp,
                    });
                }
            }

            json!({
                "ruleId": "jscpd/duplicate-code",
                "level": "warning",
                "message": { "text": format!("Duplicated code block ({} tokens)", clone.token_count) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": loc_a,
                        "region": make_region(&clone.fragment_a),
                    }
                }],
                "relatedLocations": [{
                    "id": 0,
                    "physicalLocation": {
                        "artifactLocation": loc_b,
                        "region": make_region(&clone.fragment_b),
                    }
                }],
                "properties": props,
            })
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
                format!("file:///{}/", root.replace('\\', "/"))
            };
            original_uri_base_ids[base_id] = json!({ "uri": uri });
        }

        let mut run = json!({
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
    fn sarif_reporter_name_is_sarif() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(SarifReporter::new(&opts).name(), "sarif");
    }
}
