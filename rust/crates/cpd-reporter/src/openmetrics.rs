// openmetrics.rs — OpenMetrics text-format reporter (issue #422).
// Produces: <output_dir>/jscpd-metrics.txt
//
// The output follows the OpenMetrics exposition format
// (https://openmetrics.io/), which GitLab CI consumes via
// `artifacts:reports:metrics` to show metric changes in merge requests.
// Each statistic is a gauge family with an unlabeled total sample followed
// by one `format="..."` labeled sample per detected format.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, write_report_file};
use cpd_core::models::{CpdClone, StatRow, Statistics};
use std::path::Path;
use std::time::Duration;

pub struct OpenMetricsReporter {
    style: Style,
}

impl OpenMetricsReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
        }
    }
}

/// Escape a label value per the OpenMetrics ABNF: backslash, double quote
/// and line feed must be backslash-escaped.
fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Render a sample value. f64 Display already prints integral values
/// without a trailing ".0" (`10.0` -> "10"), which OpenMetrics allows.
fn fmt_value(value: f64) -> String {
    format!("{}", value)
}

/// Append one gauge family: metadata, the unlabeled total sample, then a
/// `format`-labeled sample per format (sorted by name).
fn push_gauge(
    out: &mut String,
    stats: &Statistics,
    format_names: &[&str],
    name: &str,
    unit: Option<&str>,
    help: &str,
    value_of: impl Fn(&StatRow) -> f64,
) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    if let Some(unit) = unit {
        out.push_str(&format!("# UNIT {name} {unit}\n"));
    }
    out.push_str(&format!("{name} {}\n", fmt_value(value_of(&stats.total))));
    for fmt in format_names {
        if let Some(row) = stats.formats.get(*fmt) {
            out.push_str(&format!(
                "{name}{{format=\"{}\"}} {}\n",
                escape_label(fmt),
                fmt_value(value_of(row))
            ));
        }
    }
}

fn render_openmetrics(stats: &Statistics, duration: Duration) -> String {
    let mut format_names: Vec<&str> = stats.formats.keys().map(|s| s.as_str()).collect();
    format_names.sort_unstable();

    let mut out = String::new();
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_source_files",
        None,
        "Number of source files analyzed.",
        |r| r.sources as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_lines_analyzed",
        None,
        "Total number of lines analyzed.",
        |r| r.lines as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_tokens_analyzed",
        None,
        "Total number of tokens analyzed.",
        |r| r.tokens as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_clones_found",
        None,
        "Number of duplicated code blocks found.",
        |r| r.clones as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_duplicated_lines",
        None,
        "Number of duplicated lines.",
        |r| r.duplicated_lines as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_duplicated_tokens",
        None,
        "Number of duplicated tokens.",
        |r| r.duplicated_tokens as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_new_clones",
        None,
        "Number of duplicated code blocks absent from the configured baseline.",
        |r| r.new_clones as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_new_duplicated_lines",
        None,
        "Number of duplicated lines in clones absent from the configured baseline.",
        |r| r.new_duplicated_lines as f64,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_duplicated_lines_percent",
        Some("percent"),
        "Percentage of duplicated lines.",
        |r| r.percentage,
    );
    push_gauge(
        &mut out,
        stats,
        &format_names,
        "jscpd_duplicated_tokens_percent",
        Some("percent"),
        "Percentage of duplicated tokens.",
        |r| r.percentage_tokens,
    );

    let name = "jscpd_detection_duration_seconds";
    out.push_str(&format!("# HELP {name} Time spent detecting duplicates.\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    out.push_str(&format!("# UNIT {name} seconds\n"));
    out.push_str(&format!("{name} {}\n", fmt_value(duration.as_secs_f64())));

    out.push_str("# EOF\n");
    out
}

impl Reporter for OpenMetricsReporter {
    fn name(&self) -> &str {
        "openmetrics"
    }

    fn report(
        &self,
        _clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let content = render_openmetrics(ctx.stats, ctx.duration);
        write_report_file(
            output_dir,
            "jscpd-metrics.txt",
            &content,
            &self.style,
            "OpenMetrics",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::shared::fixtures::{one_clone_stats, tmp_dir};
    use crate::{assert_empty_report_ok, assert_reporter_name};
    use std::time::Duration;

    assert_reporter_name!(
        openmetrics_reporter_name,
        OpenMetricsReporter,
        "openmetrics"
    );
    assert_empty_report_ok!(openmetrics_empty_clones_ok, OpenMetricsReporter);

    fn run_openmetrics_report() -> String {
        let dir = tmp_dir("openmetrics");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = OpenMetricsReporter::new(&opts);
        let stats = one_clone_stats();
        let ctx = ReportContext::new(&stats, Duration::from_millis(1500));
        reporter.report(&[], &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-metrics.txt")).unwrap()
    }

    #[test]
    fn openmetrics_ends_with_eof() {
        let content = run_openmetrics_report();
        assert!(content.ends_with("# EOF\n"), "must end with # EOF marker");
    }

    #[test]
    fn openmetrics_has_metadata_before_samples() {
        let content = run_openmetrics_report();
        let help = content.find("# HELP jscpd_clones_found").unwrap();
        let ty = content.find("# TYPE jscpd_clones_found gauge").unwrap();
        let sample = content.find("\njscpd_clones_found 1\n").unwrap();
        assert!(help < ty && ty < sample);
    }

    #[test]
    fn openmetrics_contains_total_and_format_samples() {
        let content = run_openmetrics_report();
        assert!(content.contains("jscpd_duplicated_lines 10\n"));
        assert!(content.contains("jscpd_duplicated_lines{format=\"javascript\"} 10\n"));
        assert!(content.contains("jscpd_duplicated_lines_percent{format=\"javascript\"} 10\n"));
    }

    #[test]
    fn openmetrics_reports_new_clone_gauges() {
        let mut stats = one_clone_stats();
        stats.total.new_clones = 1;
        stats.total.new_duplicated_lines = 9;
        if let Some(row) = stats.formats.get_mut("javascript") {
            row.new_clones = 1;
            row.new_duplicated_lines = 9;
        }
        let dir = tmp_dir("openmetrics-new");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = OpenMetricsReporter::new(&opts);
        let ctx = ReportContext::new(&stats, Duration::ZERO);
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-metrics.txt")).unwrap();
        assert!(content.contains("# TYPE jscpd_new_clones gauge"));
        assert!(content.contains("\njscpd_new_clones 1\n"));
        assert!(content.contains("jscpd_new_clones{format=\"javascript\"} 1\n"));
        assert!(content.contains("\njscpd_new_duplicated_lines 9\n"));
    }

    #[test]
    fn openmetrics_reports_duration_in_seconds() {
        let content = run_openmetrics_report();
        assert!(content.contains("jscpd_detection_duration_seconds 1.5\n"));
    }

    #[test]
    fn escape_label_handles_special_chars() {
        assert_eq!(escape_label(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_label("a\nb"), "a\\nb");
        assert_eq!(escape_label("c#"), "c#");
    }

    #[test]
    fn fmt_value_prints_integral_floats_without_fraction() {
        assert_eq!(fmt_value(10.0), "10");
        assert_eq!(fmt_value(10.25), "10.25");
    }
}
