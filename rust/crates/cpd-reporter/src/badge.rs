// cpd-reporter: Badge reporter — writes SVG badge files

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use badgelib::{Badge, Color};
use cpd_core::models::CpdClone;
use std::{fs, path::Path};

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

    fn report(
        &self,
        _clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;

        let pct = format!("{:.1}%", ctx.stats.total.percentage);
        let color = duplication_color(ctx.stats.total.percentage);
        let badge_svg = make_badge("duplication", &pct, color);
        fs::write(output_dir.join("jscpd-badge.svg"), badge_svg)?;

        let lines_str = ctx.stats.total.duplicated_lines.to_string();
        let lines_badge = make_badge("dup lines", &lines_str, "#3498db");
        fs::write(output_dir.join("jscpd-lines-badge.svg"), lines_badge)?;

        println!(
            "\x1b[32mBadge saved to {}\x1b[39m",
            output_dir.join("jscpd-badge.svg").display()
        );
        Ok(())
    }
}

pub fn duplication_color(percentage: f64) -> &'static str {
    if percentage > 20.0 {
        "#e74c3c"
    } else if percentage > 10.0 {
        "#f39c12"
    } else {
        "#27ae60"
    }
}

fn make_badge(label: &str, value: &str, color: &str) -> String {
    Badge::new()
        .label(label)
        .label_color(Color::Hex("555".into()))
        .value(value)
        .value_color(Color::Hex(color.trim_start_matches('#').into()))
        .to_svg()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{stats_with_pct, tmp_dir};
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn badge_svg_is_well_formed_xml() {
        let dir = tmp_dir("badge");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = BadgeReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(5.0, 10),
            duration: Duration::ZERO,
            summary: None,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-badge.svg")).unwrap();
        assert!(content.contains("<svg"), "badge must be SVG");
        assert!(
            content.contains("</svg>") || content.ends_with("/>"),
            "badge SVG must be closed"
        );
    }

    fn run_badge_report(pct: f64, duplicated_lines: u64) -> (PathBuf, PathBuf) {
        let dir = tmp_dir("badge");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = BadgeReporter::new(&opts);
        let ctx = ReportContext {
            stats: &stats_with_pct(pct, duplicated_lines),
            duration: Duration::ZERO,
            summary: None,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        (dir.clone(), dir.join("jscpd-badge.svg"))
    }

    #[test]
    fn badge_contains_percentage() {
        let (_dir, path) = run_badge_report(15.5, 50);
        let content = std::fs::read_to_string(path).unwrap();
        assert!(
            content.contains("15.5"),
            "badge must contain percentage value"
        );
    }

    #[test]
    fn both_badge_files_created() {
        let (dir, _path) = run_badge_report(5.0, 10);
        assert!(dir.join("jscpd-badge.svg").exists());
        assert!(dir.join("jscpd-lines-badge.svg").exists());
    }

    #[test]
    fn badge_color_red_for_high_duplication() {
        let svg = make_badge("duplication", "25.0%", "#e74c3c");
        assert!(svg.contains("#e74c3c"));
    }
}
