// html.rs — HTML reporter matching TypeScript jscpd HTML report layout
// Uses embedded CSS matching Tailwind v2 color scheme and layout.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, fragment_text, write_report_file};
use askama::Template;
use cpd_core::models::CpdClone;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct FormatView {
    name: String,
    sources: u64,
    lines: u64,
    clones: u64,
    duplicated_lines: u64,
    duplicated_tokens: u64,
    percentage: String,
    percentage_tokens: String,
}

struct CloneView {
    file_a: String,
    start_a: u32,
    start_col_a: u32,
    end_a: u32,
    end_col_a: u32,
    file_b: String,
    start_b: u32,
    start_col_b: u32,
    end_b: u32,
    end_col_b: u32,
    fragment: String,
}

struct CloneGroup {
    format: String,
    clones: Vec<CloneView>,
}

#[derive(Template)]
#[template(path = "report.html")]
struct ReportTemplate {
    version: String,
    total_sources: u64,
    total_lines: u64,
    total_clones: usize,
    duplicated_lines: u64,
    duplicated_tokens: u64,
    percentage: String,
    percentage_tokens: String,
    formats: Vec<FormatView>,
    clone_groups: Vec<CloneGroup>,
}

pub struct HtmlReporter {
    style: Style,
}

impl HtmlReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
        }
    }
}

impl Reporter for HtmlReporter {
    fn name(&self) -> &str {
        "html"
    }

    fn report(
        &self,
        clones: &[CpdClone],
        ctx: &ReportContext,
        output_dir: &Path,
    ) -> Result<(), ReporterError> {
        let mut file_cache: HashMap<String, String> = HashMap::new();

        let mut formats: Vec<FormatView> = ctx
            .stats
            .formats
            .iter()
            .filter(|(_, row)| row.sources > 0)
            .map(|(name, row)| FormatView {
                name: name.clone(),
                sources: row.sources,
                lines: row.lines,
                clones: row.clones,
                duplicated_lines: row.duplicated_lines,
                duplicated_tokens: row.duplicated_tokens,
                percentage: format!("{:.2}", row.percentage),
                percentage_tokens: format!("{:.2}", row.percentage_tokens),
            })
            .collect();
        formats.sort_by(|a, b| a.name.cmp(&b.name));

        let mut group_map: BTreeMap<String, Vec<CloneView>> = BTreeMap::new();
        for clone in clones {
            let fragment_text = fragment_text(&mut file_cache, &clone.fragment_a);

            group_map
                .entry(clone.format.clone())
                .or_default()
                .push(CloneView {
                    file_a: clone.fragment_a.source_id.clone(),
                    start_a: clone.fragment_a.start.line,
                    start_col_a: clone.fragment_a.start.column + 1,
                    end_a: clone.fragment_a.end.line,
                    end_col_a: clone.fragment_a.end.column + 1,
                    file_b: clone.fragment_b.source_id.clone(),
                    start_b: clone.fragment_b.start.line,
                    start_col_b: clone.fragment_b.start.column + 1,
                    end_b: clone.fragment_b.end.line,
                    end_col_b: clone.fragment_b.end.column + 1,
                    fragment: fragment_text,
                });
        }

        let clone_groups: Vec<CloneGroup> = group_map
            .into_iter()
            .map(|(format, clones)| CloneGroup { format, clones })
            .collect();

        let tmpl = ReportTemplate {
            version: VERSION.to_string(),
            total_sources: ctx.stats.total.sources,
            total_lines: ctx.stats.total.lines,
            total_clones: clones.len(),
            duplicated_lines: ctx.stats.total.duplicated_lines,
            duplicated_tokens: ctx.stats.total.duplicated_tokens,
            percentage: format!("{:.2}", ctx.stats.total.percentage),
            percentage_tokens: format!("{:.2}", ctx.stats.total.percentage_tokens),
            formats,
            clone_groups,
        };

        let rendered = tmpl
            .render()
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        write_report_file(
            output_dir,
            "jscpd-report.html",
            &rendered,
            &self.style,
            "HTML",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use crate::shared::fixtures::{empty_ctx, empty_stats, make_clone_with_locations, tmp_dir};
    use cpd_core::models::{CpdClone, Fragment, Location, Statistics};
    use std::time::Duration;

    fn run_html_report(clones: &[CpdClone], stats: &Statistics) -> String {
        let dir = tmp_dir("html");
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
        let ctx = ReportContext {
            stats,
            duration: Duration::ZERO,
        };
        reporter.report(clones, &ctx, &dir).unwrap();
        std::fs::read_to_string(dir.join("jscpd-report.html")).unwrap()
    }

    #[test]
    fn empty_clones_produces_html() {
        let content = run_html_report(&[], &empty_stats());
        assert!(content.contains("<html"), "output must be HTML");
        assert!(content.contains("<body"), "output must have body");
    }

    #[test]
    fn html_contains_clone_count() {
        let dir = tmp_dir("html");
        let file_a = dir.join("a.js");
        std::fs::write(&file_a, "hello\nworld\n").unwrap();
        let file_a_str = file_a.to_string_lossy().into_owned();
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let end = Location {
            line: 2,
            column: 0,
            offset: 10,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: Fragment {
                source_id: file_a_str,
                start: loc.clone(),
                end,
                range: [0, 10],
                blame: None,
            },
            fragment_b: Fragment {
                source_id: "b.js".to_string(),
                start: loc,
                end: Location {
                    line: 2,
                    column: 0,
                    offset: 10,
                },
                range: [0, 10],
                blame: None,
            },
            token_count: 50,
        };
        let mut stats = empty_stats();
        stats.total.clones = 1;
        let content = run_html_report(&[clone], &stats);
        assert!(
            content.contains("a.js"),
            "HTML must contain source file name"
        );
    }

    #[test]
    fn empty_clones_shows_no_duplicates_message() {
        let content = run_html_report(&[], &empty_stats());
        assert!(
            content.contains("No duplicates"),
            "empty report must mention no duplicates"
        );
    }
}
