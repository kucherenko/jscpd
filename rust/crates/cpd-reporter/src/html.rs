// html.rs — HTML reporter matching TypeScript jscpd HTML report layout
// Uses embedded CSS matching Tailwind v2 color scheme and layout.

use crate::context::ReportContext;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use askama::Template;
use cpd_core::models::CpdClone;
use std::collections::BTreeMap;
use std::{fs, path::Path};

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

pub struct HtmlReporter;

impl HtmlReporter {
    pub fn new(_opts: &ReporterOptions) -> Self {
        Self
    }
}

fn extract_lines(content: &str, start_line: u32, end_line: u32) -> String {
    content
        .lines()
        .skip(start_line.saturating_sub(1) as usize)
        .take(end_line.saturating_sub(start_line.saturating_sub(1)) as usize)
        .collect::<Vec<_>>()
        .join("\n")
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
        fs::create_dir_all(output_dir)?;

        let path = output_dir.join("jscpd-report.html");

        let mut file_cache: BTreeMap<String, String> = BTreeMap::new();

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
            let content_a = file_cache
                .entry(clone.fragment_a.source_id.clone())
                .or_insert_with(|| {
                    fs::read_to_string(&clone.fragment_a.source_id).unwrap_or_default()
                });
            let fragment_text = extract_lines(
                content_a,
                clone.fragment_a.start.line,
                clone.fragment_a.end.line,
            );

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

        fs::write(&path, rendered)?;

        println!(
            "\x1b[32mHTML report saved to {}\x1b[39m",
            path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::context::ReportContext;
    use crate::reporter::ReporterOptions;
    use cpd_core::models::{CpdClone, Fragment, Location, StatRow, Statistics};
    use std::collections::HashMap;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cpd-html-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
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
                new_duplicated_lines: 0,
                new_clones: 0,
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
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        let html_path = dir.join("jscpd-report.html");
        let content = std::fs::read_to_string(html_path).unwrap();
        assert!(content.contains("<html"), "output must be HTML");
        assert!(content.contains("<body"), "output must have body");
    }

    #[test]
    fn html_contains_clone_count() {
        let dir = tmp_dir();
        let file_a = dir.join("a.js");
        std::fs::write(&file_a, "hello\nworld\n").unwrap();
        let file_a_str = file_a.to_string_lossy().into_owned();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
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
        let frag = Fragment {
            source_id: file_a_str,
            start: loc.clone(),
            end: end,
            range: [0, 10],
            blame: None,
        };
        let frag_b = Fragment {
            source_id: "b.js".to_string(),
            start: loc,
            end: Location {
                line: 2,
                column: 0,
                offset: 10,
            },
            range: [0, 10],
            blame: None,
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag,
            fragment_b: frag_b,
            token_count: 50,
        };
        let mut stats = empty_stats();
        stats.total.clones = 1;
        let ctx = ReportContext {
            stats: &stats,
            duration: Duration::ZERO,
        };
        reporter.report(&[clone], &ctx, &dir).unwrap();
        let html_path = dir.join("jscpd-report.html");
        let content = std::fs::read_to_string(html_path).unwrap();
        assert!(
            content.contains("a.js"),
            "HTML must contain source file name"
        );
    }

    #[test]
    fn empty_clones_shows_no_duplicates_message() {
        let dir = tmp_dir();
        let opts = ReporterOptions::new(dir.clone());
        let reporter = HtmlReporter::new(&opts);
        let ctx = ReportContext {
            stats: &empty_stats(),
            duration: Duration::ZERO,
        };
        reporter.report(&[], &ctx, &dir).unwrap();
        let html_path = dir.join("jscpd-report.html");
        let content = std::fs::read_to_string(html_path).unwrap();
        assert!(
            content.contains("No duplicates"),
            "empty report must mention no duplicates"
        );
    }
}
