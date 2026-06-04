// sarif.rs
// SARIF 2.1.0 reporter: writes jscpd-report.sarif.

use std::{fs, path::Path};
use serde_json::{json, Value};
use cpd_core::models::{CpdClone, Statistics};
use crate::reporter::{Reporter, ReporterError, ReporterOptions};

pub struct SarifReporter {
    blame: bool,
}

impl SarifReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self { blame: opts.blame }
    }
}

impl Reporter for SarifReporter {
    fn name(&self) -> &str { "sarif" }

    fn report(&self, clones: &[CpdClone], _stats: &Statistics, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.sarif");

        let results: Vec<Value> = clones.iter().map(|clone| {
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
                "ruleId": "cpd/duplicate-code",
                "level": "warning",
                "message": { "text": format!("Duplicated code block ({} tokens)", clone.token_count) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": clone.fragment_a.source_id },
                        "region": {
                            "startLine": clone.fragment_a.start.line,
                            "endLine": clone.fragment_a.end.line,
                        }
                    }
                }],
                "relatedLocations": [{
                    "id": 0,
                    "physicalLocation": {
                        "artifactLocation": { "uri": clone.fragment_b.source_id },
                        "region": {
                            "startLine": clone.fragment_b.start.line,
                            "endLine": clone.fragment_b.end.line,
                        }
                    }
                }],
                "properties": props,
            })
        }).collect();

        let sarif = json!({
            "version": "2.1.0",
            "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "cpd",
                        "version": env!("CARGO_PKG_VERSION"),
                        "rules": [{
                            "id": "cpd/duplicate-code",
                            "name": "DuplicateCode",
                            "shortDescription": { "text": "Duplicated code detected" }
                        }]
                    }
                },
                "results": results,
            }]
        });

        let content = serde_json::to_string_pretty(&sarif)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
    use crate::reporter::ReporterOptions;
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-sarif-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn empty_stats() -> Statistics {
        Statistics {
            total: StatRow {
                lines: 0, tokens: 0, sources: 0, clones: 0,
                duplicated_lines: 0, duplicated_tokens: 0,
                percentage: 0.0, percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_clone() -> CpdClone {
        let loc = Location { line: 10, column: 0, offset: 0 };
        let end = Location { line: 20, column: 0, offset: 0 };
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

    #[test]
    fn sarif_version_is_2_1_0() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = SarifReporter::new(&opts);
        reporter.report(&[], &empty_stats(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
    }

    #[test]
    fn sarif_output_has_runs_and_results() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = SarifReporter::new(&opts);
        let clone = make_clone();
        reporter.report(&[clone], &empty_stats(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["runs"][0]["results"].is_array());
        assert_eq!(parsed["runs"][0]["results"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn sarif_blame_included_when_flag_set() {
        let dir = tmp_dir();
        let mut opts = ReporterOptions::new(dir.clone());
        opts.blame = true;
        let reporter = SarifReporter::new(&opts);
        let clone = make_clone();
        reporter.report(&[clone], &empty_stats(), &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.sarif")).unwrap();
        assert!(content.contains("deadbeef"), "SARIF must include blame SHA when blame=true");
    }

    #[test]
    fn sarif_reporter_name_is_sarif() {
        let opts = ReporterOptions::new(std::path::PathBuf::from("/tmp"));
        assert_eq!(SarifReporter::new(&opts).name(), "sarif");
    }
}
