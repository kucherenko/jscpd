//! Reports a CI system consumes: SARIF, Code Climate, OpenMetrics, a badge,
//! Xcode diagnostics, and the exit-code gate.

use super::structured::escape_xml;
use super::{DeadCodeContext, DeadCodeReporter, reasons_clause, severity};
use crate::reporter::{ReporterError, ReporterOptions};
use crate::shared::{Style, write_report_file};
use cpd_core::deadcode::{Category, Finding};
use serde_json::{Value, json};
use std::path::Path;

/// SARIF 2.1.0, which GitHub code scanning reads directly.
pub struct SarifReporter {
    style: Style,
    tool_version: String,
}

impl SarifReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
            tool_version: options.tool_version.clone(),
        }
    }
}

impl DeadCodeReporter for SarifReporter {
    fn name(&self) -> &str {
        "sarif"
    }

    fn report(
        &self,
        findings: &[Finding],
        _ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        // Every category is declared as a rule whether or not it fired, so a
        // run that fixes the last finding of a category still resolves that
        // category's alerts instead of leaving them orphaned.
        let rules: Vec<Value> = Category::ALL
            .iter()
            .map(|category| {
                json!({
                    "id": category.as_str(),
                    "name": category.title(),
                    "shortDescription": { "text": category.title() },
                    "fullDescription": { "text": rule_description(*category) },
                    "defaultConfiguration": { "level": "warning" },
                    "properties": { "tags": ["dead-code", "maintainability"] }
                })
            })
            .collect();

        let results: Vec<Value> = findings
            .iter()
            .map(|finding| {
                let mut result = json!({
                    "ruleId": finding.category.as_str(),
                    "level": severity(finding).sarif(),
                    "message": { "text": finding.message },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": finding.path },
                            "region": {
                                "startLine": finding.start.line.max(1),
                                "startColumn": finding.start.column + 1,
                                "endLine": finding.end.line.max(finding.start.line).max(1)
                            }
                        }
                    }],
                    "partialFingerprints": { "bastaFingerprint/v1": finding.fingerprint() },
                    "properties": {
                        "confidence": finding.confidence,
                        "level": finding.level().as_str()
                    }
                });
                let reasons = reasons_clause(finding);
                if !reasons.is_empty() {
                    result["properties"]["reasons"] = json!(reasons);
                }
                result
            })
            .collect();

        let sarif = json!({
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "version": "2.1.0",
            "runs": [{
                "tool": { "driver": {
                    "name": "basta",
                    "informationUri": "https://jscpd.dev",
                    "version": self.tool_version,
                    "rules": rules
                }},
                "results": results
            }]
        });
        let json = serde_json::to_string_pretty(&sarif)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        write_report_file(
            output_dir,
            "basta-report.sarif",
            json,
            &self.style,
            "Dead code SARIF",
        )?;
        Ok(())
    }
}

fn rule_description(category: Category) -> &'static str {
    match category {
        Category::UnusedFile => {
            "A file that no entry point reaches through the import graph. Nothing runs it."
        }
        Category::UnusedExport => "An exported declaration that no module in the project imports.",
        Category::UnusedSymbol => {
            "A module-private declaration that nothing in its own module reaches."
        }
        Category::UnusedImport => "An import binding with no references in the file that made it.",
        Category::UnusedMember => {
            "A class or enum member whose name is never read anywhere in the project."
        }
    }
}

/// Code Climate / GitLab Code Quality JSON.
pub struct CodeClimateReporter {
    style: Style,
}

impl CodeClimateReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for CodeClimateReporter {
    fn name(&self) -> &str {
        "codeclimate"
    }

    fn report(
        &self,
        findings: &[Finding],
        _ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let issues: Vec<Value> = findings
            .iter()
            .map(|finding| {
                json!({
                    "type": "issue",
                    "check_name": format!("basta/{}", finding.category.as_str()),
                    "description": finding.message,
                    "categories": ["Complexity"],
                    "fingerprint": finding.fingerprint(),
                    "severity": severity(finding).codeclimate(),
                    "location": {
                        "path": finding.path,
                        "lines": {
                            "begin": finding.start.line.max(1),
                            "end": finding.end.line.max(finding.start.line).max(1)
                        }
                    }
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&issues)
            .map_err(|e| ReporterError::Format(e.to_string()))?;
        write_report_file(
            output_dir,
            "basta-codeclimate.json",
            json,
            &self.style,
            "Dead code Code Climate",
        )?;
        Ok(())
    }
}

/// OpenMetrics counters, for a metrics pipeline that tracks dead code over time.
pub struct OpenMetricsReporter {
    style: Style,
}

impl OpenMetricsReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for OpenMetricsReporter {
    fn name(&self) -> &str {
        "openmetrics"
    }

    fn report(
        &self,
        findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let stats = ctx.stats;
        let mut out = String::new();
        out.push_str("# TYPE basta_findings gauge\n");
        out.push_str("# HELP basta_findings Dead code findings by category.\n");
        for category in Category::ALL {
            out.push_str(&format!(
                "basta_findings{{category=\"{}\"}} {}\n",
                category.as_str(),
                stats.count_of(*category)
            ));
        }
        for (name, help, value) in [
            (
                "basta_findings_total",
                "Dead code findings.",
                findings.len() as f64,
            ),
            ("basta_files", "Files analyzed.", f64::from(stats.files)),
            (
                "basta_files_reachable",
                "Files an entry point reaches.",
                f64::from(stats.reachable_files),
            ),
            (
                "basta_files_unparsed",
                "Files that could not be parsed.",
                f64::from(stats.unparsed),
            ),
            (
                "basta_entry_points",
                "Entry points detected.",
                f64::from(stats.entry_points),
            ),
            (
                "basta_dead_lines",
                "Lines covered by findings.",
                f64::from(stats.dead_lines),
            ),
            (
                "basta_total_lines",
                "Lines analyzed.",
                f64::from(stats.total_lines),
            ),
            (
                "basta_dead_percentage",
                "Dead lines as a percentage of lines analyzed.",
                stats.percentage,
            ),
        ] {
            out.push_str(&format!(
                "# TYPE {name} gauge\n# HELP {name} {help}\n{name} {value}\n"
            ));
        }
        out.push_str("# EOF\n");
        write_report_file(
            output_dir,
            "basta-report.txt",
            out,
            &self.style,
            "Dead code OpenMetrics",
        )?;
        Ok(())
    }
}

/// An SVG badge for a README.
pub struct BadgeReporter {
    style: Style,
}

impl BadgeReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            style: Style::new(options.no_colors),
        }
    }
}

impl DeadCodeReporter for BadgeReporter {
    fn name(&self) -> &str {
        "badge"
    }

    fn report(
        &self,
        findings: &[Finding],
        _ctx: &DeadCodeContext<'_>,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let count = findings.len();
        let value = if count == 0 {
            "none".to_string()
        } else {
            count.to_string()
        };
        let color = match count {
            0 => "#4c1",
            1..=10 => "#dfb317",
            _ => "#e05d44",
        };
        let label = "dead code";
        // 6px per character plus padding is the same approximation the clone
        // badge uses, so the two sit together without one looking cramped.
        let label_width = label.len() * 6 + 10;
        let value_width = value.len() * 7 + 10;
        let total = label_width + value_width;
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="20" role="img" aria-label="{label}: {value}">
  <title>{label}: {value}</title>
  <linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient>
  <clipPath id="r"><rect width="{total}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_width}" height="20" fill="#555"/>
    <rect x="{label_width}" width="{value_width}" height="20" fill="{color}"/>
    <rect width="{total}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11">
    <text x="{label_x}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{label_x}" y="14">{label}</text>
    <text x="{value_x}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{value_x}" y="14">{value}</text>
  </g>
</svg>
"##,
            label = escape_xml(label),
            value = escape_xml(&value),
            label_x = label_width / 2,
            value_x = label_width + value_width / 2,
        );
        write_report_file(
            output_dir,
            "basta-badge.svg",
            svg,
            &self.style,
            "Dead code badge",
        )?;
        Ok(())
    }
}

/// Xcode-style diagnostics on stdout, which Xcode and many editors parse.
pub struct XcodeReporter;

impl XcodeReporter {
    pub fn new(_options: &ReporterOptions) -> Self {
        Self
    }
}

impl DeadCodeReporter for XcodeReporter {
    fn name(&self) -> &str {
        "xcode"
    }

    fn report(
        &self,
        findings: &[Finding],
        _ctx: &DeadCodeContext<'_>,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        for finding in findings {
            println!(
                "{}:{}:{}: {}: {} [basta.{}]",
                finding.path,
                finding.start.line.max(1),
                finding.start.column + 1,
                severity(finding).xcode(),
                finding.message,
                finding.category.as_str(),
            );
        }
        Ok(())
    }
}

/// Fails the run when dead code exceeds a percentage of the codebase.
pub struct ThresholdReporter {
    threshold: Option<f64>,
}

impl ThresholdReporter {
    pub fn new(options: &ReporterOptions) -> Self {
        Self {
            threshold: options.threshold,
        }
    }
}

impl DeadCodeReporter for ThresholdReporter {
    fn name(&self) -> &str {
        "threshold"
    }

    fn report(
        &self,
        _findings: &[Finding],
        ctx: &DeadCodeContext<'_>,
        _output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let Some(threshold) = self.threshold else {
            return Ok(());
        };
        if ctx.stats.percentage > threshold {
            return Err(ReporterError::ThresholdExceeded {
                actual: ctx.stats.percentage,
                threshold,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures;
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn write(name: &str, reporter: &dyn DeadCodeReporter, file: &str) -> String {
        let dir = fixtures::unique_dir(name);
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        reporter
            .report(&fixtures::findings(), &ctx, &dir)
            .expect("report");
        let content = std::fs::read_to_string(dir.join(file)).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        content
    }

    fn options() -> ReporterOptions {
        ReporterOptions::new(PathBuf::from("/tmp"))
    }

    #[test]
    fn sarif_declares_every_rule_and_one_result_per_finding() {
        let sarif = write(
            "sarif",
            &SarifReporter::new(&options()),
            "basta-report.sarif",
        );
        let value: Value = serde_json::from_str(&sarif).expect("valid JSON");
        assert_eq!(value["version"], "2.1.0");
        let run = &value["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "basta");
        assert_eq!(
            run["tool"]["driver"]["rules"].as_array().unwrap().len(),
            Category::ALL.len(),
            "a category with no findings still needs its rule, or its alerts never resolve"
        );
        let results = run["results"].as_array().unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0]["ruleId"], "unused-file");
        assert!(results[0]["partialFingerprints"]["bastaFingerprint/v1"].is_string());
    }

    #[test]
    fn sarif_regions_are_one_based_and_never_zero() {
        let mut finding = fixtures::finding(Category::UnusedFile, "a.ts", "", 95);
        finding.start = fixtures::location(0);
        finding.end = fixtures::location(0);
        let dir = fixtures::unique_dir("sarif-zero");
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        SarifReporter::new(&options())
            .report(&[finding], &ctx, &dir)
            .unwrap();
        let value: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("basta-report.sarif")).unwrap())
                .unwrap();
        let region = &value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];
        assert_eq!(region["startLine"], 1, "SARIF lines start at 1");
        assert_eq!(region["startColumn"], 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn codeclimate_issues_carry_a_stable_fingerprint() {
        let json = write(
            "cc",
            &CodeClimateReporter::new(&options()),
            "basta-codeclimate.json",
        );
        let issues: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0]["type"], "issue");
        assert!(
            issues[0]["check_name"]
                .as_str()
                .unwrap()
                .starts_with("basta/")
        );
        assert!(issues[0]["fingerprint"].is_string());
    }

    #[test]
    fn openmetrics_emits_one_series_per_category_and_ends_with_eof() {
        let text = write(
            "om",
            &OpenMetricsReporter::new(&options()),
            "basta-report.txt",
        );
        for category in Category::ALL {
            assert!(
                text.contains(&format!("category=\"{}\"", category.as_str())),
                "{text}"
            );
        }
        assert!(text.contains("basta_findings_total 3"), "{text}");
        assert!(
            text.ends_with("# EOF\n"),
            "OpenMetrics requires the EOF marker"
        );
    }

    #[test]
    fn the_badge_changes_colour_with_the_count() {
        let clean = badge_for(&[]);
        assert!(
            clean.contains("#4c1") && clean.contains(">none<"),
            "{clean}"
        );
        let some = badge_for(&fixtures::findings());
        assert!(some.contains("#dfb317") && some.contains(">3<"), "{some}");
    }

    fn badge_for(findings: &[Finding]) -> String {
        let dir = fixtures::unique_dir("badge");
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        BadgeReporter::new(&options())
            .report(findings, &ctx, &dir)
            .unwrap();
        let svg = std::fs::read_to_string(dir.join("basta-badge.svg")).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        svg
    }

    #[test]
    fn the_threshold_reporter_fails_only_above_its_limit() {
        let stats = fixtures::stats(); // 3.4%
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        let mut options = options();

        options.threshold = Some(5.0);
        assert!(
            ThresholdReporter::new(&options)
                .report(&[], &ctx, Path::new("/tmp"))
                .is_ok()
        );

        options.threshold = Some(1.0);
        let error = ThresholdReporter::new(&options)
            .report(&[], &ctx, Path::new("/tmp"))
            .unwrap_err();
        assert!(matches!(error, ReporterError::ThresholdExceeded { .. }));

        options.threshold = None;
        assert!(
            ThresholdReporter::new(&options)
                .report(&[], &ctx, Path::new("/tmp"))
                .is_ok(),
            "without a threshold there is nothing to exceed"
        );
    }

    #[test]
    fn xcode_output_does_not_panic_on_a_zero_line_finding() {
        let mut finding = fixtures::finding(Category::UnusedFile, "a.ts", "", 95);
        finding.start = fixtures::location(0);
        let stats = fixtures::stats();
        let ctx = DeadCodeContext::new(&stats, Duration::ZERO);
        assert!(
            XcodeReporter
                .report(&[finding], &ctx, Path::new("/tmp"))
                .is_ok()
        );
    }
}
