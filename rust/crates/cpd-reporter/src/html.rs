// html.rs — HTML reporter using askama compile-time templates
use askama::Template;
use std::{fs, path::Path};
use cpd_core::models::CpdClone;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::context::ReportContext;

struct CloneView {
    token_count: u32,
    file_a: String,
    start_a: u32,
    end_a: u32,
    file_b: String,
    start_b: u32,
    end_b: u32,
}

#[derive(Template)]
#[template(path = "report.html")]
struct ReportTemplate {
    detection_date: String,
    clone_count: usize,
    duplicated_lines: u64,
    percentage: String,
    clones: Vec<CloneView>,
}

pub struct HtmlReporter;

impl HtmlReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

impl Reporter for HtmlReporter {
    fn name(&self) -> &str {
        "html"
    }

    fn report(&self, clones: &[CpdClone], ctx: &ReportContext, output_dir: &Path) -> Result<(), ReporterError> {
        fs::create_dir_all(output_dir)?;
        let path = output_dir.join("jscpd-report.html");

        let clone_views: Vec<CloneView> = clones
            .iter()
            .map(|c| CloneView {
                token_count: c.token_count,
                file_a: c.fragment_a.source_id.clone(),
                start_a: c.fragment_a.start.line,
                end_a: c.fragment_a.end.line,
                file_b: c.fragment_b.source_id.clone(),
                start_b: c.fragment_b.start.line,
                end_b: c.fragment_b.end.line,
            })
            .collect();

        let tmpl = ReportTemplate {
            detection_date: ctx.stats.detection_date.clone(),
            clone_count: clones.len(),
            duplicated_lines: ctx.stats.total.duplicated_lines,
            percentage: format!("{:.1}", ctx.stats.total.percentage),
            clones: clone_views,
        };

        let rendered = tmpl
            .render()
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        fs::write(&path, rendered)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::path::PathBuf;

    use cpd_core::models::{CpdClone, Fragment, Location, Statistics, StatRow};
    use std::collections::HashMap;
    use crate::reporter::ReporterOptions;
    use crate::context::ReportContext;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-html-{}",
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
                lines: 0,
                tokens: 0,
                sources: 0,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
            },
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn empty_clones_produces_html() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.html")).unwrap();
        assert!(content.contains("<html"), "output must be HTML");
        assert!(content.contains("<body"), "output must have body");
    }

    #[test]
    fn html_contains_clone_count() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
        let loc = Location { line: 1, column: 0, offset: 0 };
        let frag = Fragment {
            source_id: "a.js".to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 5],
            blame: None,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 50,
        };
        let mut stats = empty_stats();
        stats.total.clones = 1;
        let ctx = ReportContext { stats: &stats, duration: Duration::ZERO };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.html")).unwrap();
        assert!(content.contains("a.js"), "HTML must contain source file name");
    }

    #[test]
    fn empty_clones_shows_no_duplicates_message() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
        let ctx = ReportContext { stats: &empty_stats(), duration: Duration::ZERO };
        reporter.report(&[], &ctx, &dir).unwrap();
        let content = std::fs::read_to_string(dir.join("jscpd-report.html")).unwrap();
        assert!(
            content.contains("No duplicates") || content.contains("no-dupes"),
            "empty report must mention no duplicates"
        );
    }
}
