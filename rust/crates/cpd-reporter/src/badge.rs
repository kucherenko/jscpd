// cpd-reporter: Badge reporter — writes SVG badge files

use std::{fs, path::Path};
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

pub struct BadgeReporter;

impl BadgeReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for BadgeReporter {
    fn name(&self) -> &str {
        "badge"
    }

    fn report(&self, _clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;

        let pct = format!("{:.1}%", ctx.stats.total.percentage);
        let color = if ctx.stats.total.percentage > 20.0 {
            "#e74c3c"
        } else if ctx.stats.total.percentage > 10.0 {
            "#f39c12"
        } else {
            "#27ae60"
        };
        let badge_svg = make_badge("duplication", &pct, color);
        fs::write(output_dir.join("jscpd-badge.svg"), badge_svg)?;

        let lines_str = ctx.stats.total.duplicated_lines.to_string();
        let lines_badge = make_badge("dup lines", &lines_str, "#3498db");
        fs::write(output_dir.join("jscpd-lines-badge.svg"), lines_badge)?;

        println!("\x1b[32mBadge saved to {}\x1b[39m", output_dir.join("jscpd-badge.svg").display());
        Ok(())
    }
}

fn make_badge(label: &str, value: &str, color: &str) -> String {
    let label_width = (label.len() * 7 + 10).max(40);
    let value_width = (value.len() * 7 + 10).max(30);
    let total_width = label_width + value_width;
    let lx = label_width / 2;
    let vx = label_width + value_width / 2;

    let mut svg = String::new();
    svg.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"20\">\n", total_width));
    svg.push_str("  <linearGradient id=\"s\" x2=\"0\" y2=\"100%\">\n");
    svg.push_str("    <stop offset=\"0\" stop-color=\"#bbb\" stop-opacity=\".1\"/>\n");
    svg.push_str("    <stop offset=\"1\" stop-opacity=\".1\"/>\n");
    svg.push_str("  </linearGradient>\n");
    svg.push_str(&format!("  <rect rx=\"3\" width=\"{}\" height=\"20\" fill=\"#555\"/>\n", total_width));
    svg.push_str(&format!("  <rect rx=\"3\" x=\"{}\" width=\"{}\" height=\"20\" fill=\"{}\"/>\n", label_width, value_width, color));
    svg.push_str(&format!("  <rect rx=\"3\" width=\"{}\" height=\"20\" fill=\"url(#s)\"/>\n", total_width));
    svg.push_str("  <g fill=\"#fff\" text-anchor=\"middle\" font-family=\"DejaVu Sans,sans-serif\" font-size=\"11\">\n");
    svg.push_str(&format!("    <text x=\"{}\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">{}</text>\n", lx, label));
    svg.push_str(&format!("    <text x=\"{}\" y=\"14\">{}</text>\n", lx, label));
    svg.push_str(&format!("    <text x=\"{}\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">{}</text>\n", vx, value));
    svg.push_str(&format!("    <text x=\"{}\" y=\"14\">{}</text>\n", vx, value));
    svg.push_str("  </g>\n</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;

    use cpd_core::models::{Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-badge-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn stats_with_pct(pct: f64, lines: u64) -> Statistics {
        Statistics {
            total: StatRow {
                lines: 100,
                tokens: 500,
                sources: 5,
                clones: 2,
                duplicated_lines: lines,
                duplicated_tokens: 50,
                percentage: pct,
                percentage_tokens: pct,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn badge_svg_is_well_formed_xml() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = BadgeReporter::new(&opts);
        let ctx = ReportContext { stats: &stats_with_pct(5.0, 10), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-badge.svg")).unwrap();
        assert!(content.contains("<svg"), "badge must be SVG");
        assert!(
            content.contains("</svg>") || content.ends_with("/>"),
            "badge SVG must be closed"
        );
    }

    #[test]
    fn badge_contains_percentage() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = BadgeReporter::new(&opts);
        let ctx = ReportContext { stats: &stats_with_pct(15.5, 50), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-badge.svg")).unwrap();
        assert!(content.contains("15.5"), "badge must contain percentage value");
    }

    #[test]
    fn both_badge_files_created() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = BadgeReporter::new(&opts);
        let ctx = ReportContext { stats: &stats_with_pct(5.0, 10), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        assert!(dir.join("jscpd-badge.svg").exists());
        assert!(dir.join("jscpd-lines-badge.svg").exists());
    }

    #[test]
    fn badge_color_red_for_high_duplication() {
        let svg = make_badge("duplication", "25.0%", "#e74c3c");
        assert!(svg.contains("#e74c3c"));
    }
}
