// sarif.rs
// SARIF 2.1.0 reporter: writes jscpd-report.sarif.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, print_saved_report};
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

        let mut seen_uris: Vec<String> = Vec::new();

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
            })
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
