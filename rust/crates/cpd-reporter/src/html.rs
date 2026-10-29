// html.rs — HTML reporter matching TypeScript jscpd HTML report layout
// Uses embedded CSS matching Tailwind v2 color scheme and layout.

use crate::context::ReportContext;
use crate::prism;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{Style, clean_source_id, fragment_text, read_file_cached, write_report_file};
use askama::Template;
use cpd_core::models::{CpdClone, Fragment};
use cpd_tokenizer::{formats, markdown, sfc};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::path::Path;

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

/// One fragment of a clone: where it is and its lines. The page shows the
/// two fragments side by side (issue #1263).
struct SideView {
    file: String,
    start: u32,
    start_col: u32,
    end: u32,
    end_col: u32,
    /// The line number of each line of the code, for the gutter.
    numbers: String,
    segments: Vec<Segment>,
}

/// A run of a fragment's lines and the Prism language that highlights it,
/// empty for none.
struct Segment {
    lang: &'static str,
    text: String,
}

impl SideView {
    fn new(file_cache: &mut HashMap<String, String>, fragment: &Fragment, format: &str) -> Self {
        let code = fragment_text(file_cache, fragment);
        let rows = if code.is_empty() {
            0
        } else {
            code.split('\n').count() as u32
        };
        let numbers = (fragment.start.line..fragment.start.line + rows)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let content = read_file_cached(file_cache, fragment);
        let segments = segments(content, fragment, format, code);
        Self {
            file: fragment.source_id.clone(),
            start: fragment.start.line,
            start_col: fragment.start.column + 1,
            end: fragment.end.line,
            end_col: fragment.end.column + 1,
            numbers,
            segments,
        }
    }
}

/// The lines of a fragment in runs that Prism highlights alike. A fragment of
/// a block embedded in another file (`guide.md:typescript`) can take in the
/// lines between two blocks, such as Markdown prose and fences, and those stay
/// plain. Any other fragment is one run in the language of its format.
fn segments(content: &str, fragment: &Fragment, format: &str, code: String) -> Vec<Segment> {
    let lang = prism::language(format).unwrap_or_default();
    let Some(blocks) = embedded_blocks(&fragment.source_id, content, format) else {
        return vec![Segment { lang, text: code }];
    };
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(content.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let in_block = |line: u32| {
        line.checked_sub(1)
            .and_then(|index| line_starts.get(index as usize))
            .is_some_and(|start| blocks.iter().any(|block| block.contains(start)))
    };

    let mut segments: Vec<Segment> = Vec::new();
    for (line, text) in (fragment.start.line..).zip(code.split('\n')) {
        let line_lang = if in_block(line) { lang } else { "" };
        match segments.last_mut() {
            Some(last) if last.lang == line_lang => {
                last.text.push('\n');
                last.text.push_str(text);
            }
            last => {
                if let Some(last) = last {
                    last.text.push('\n');
                }
                segments.push(Segment {
                    lang: line_lang,
                    text: text.to_string(),
                });
            }
        }
    }
    segments
}

/// The byte ranges of `content` that hold code of `format` when `source_id`
/// names a block embedded in a file of another format, None for a fragment
/// of a whole file. Markdown, Vue, Svelte and Astro files are split into
/// their blocks; in a file of any other kind no line counts as the block's.
fn embedded_blocks(source_id: &str, content: &str, format: &str) -> Option<Vec<Range<usize>>> {
    let path = clean_source_id(source_id);
    if path == source_id {
        return None;
    }
    let container = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(formats::get_format_by_extension);
    if container == Some(format) {
        return None;
    }
    let blocks = match container {
        Some("markdown") => markdown::code_blocks(content),
        Some(file_format @ ("vue" | "svelte" | "astro")) => {
            sfc::script_blocks(content, file_format)
                .into_iter()
                .chain(
                    sfc::extract_blocks(content, file_format)
                        .into_iter()
                        .map(|b| {
                            (
                                b.block_format,
                                b.start_offset..b.start_offset + b.content.len(),
                            )
                        }),
                )
                .collect()
        }
        _ => Vec::new(),
    };
    Some(
        blocks
            .into_iter()
            .filter(|(block_format, _)| block_format == format)
            .map(|(_, range)| range)
            .collect(),
    )
}

struct CloneView {
    a: SideView,
    b: SideView,
}

impl CloneView {
    fn sides(&self) -> [&SideView; 2] {
        [&self.a, &self.b]
    }
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
    /// Prism with the languages of the clones, empty without one.
    prism: String,
}

pub struct HtmlReporter {
    style: Style,
    tool_version: String,
    /// `--report-name` with this report's extension.
    file_name: String,
}

impl HtmlReporter {
    pub fn new(opts: &ReporterOptions) -> Self {
        Self {
            style: Style::new(opts.no_colors),
            tool_version: opts.tool_version.clone(),
            file_name: opts.report_file("html"),
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
            let view = CloneView {
                a: SideView::new(&mut file_cache, &clone.fragment_a, &clone.format),
                b: SideView::new(&mut file_cache, &clone.fragment_b, &clone.format),
            };
            group_map
                .entry(clone.format.clone())
                .or_default()
                .push(view);
        }

        let languages: BTreeSet<&str> = clones
            .iter()
            .filter_map(|clone| prism::language(&clone.format))
            .collect();

        let clone_groups: Vec<CloneGroup> = group_map
            .into_iter()
            .map(|(format, clones)| CloneGroup { format, clones })
            .collect();

        let tmpl = ReportTemplate {
            version: self.tool_version.clone(),
            total_sources: ctx.stats.total.sources,
            total_lines: ctx.stats.total.lines,
            total_clones: clones.len(),
            duplicated_lines: ctx.stats.total.duplicated_lines,
            duplicated_tokens: ctx.stats.total.duplicated_tokens,
            percentage: format!("{:.2}", ctx.stats.total.percentage),
            percentage_tokens: format!("{:.2}", ctx.stats.total.percentage_tokens),
            formats,
            clone_groups,
            prism: prism::script(languages),
        };

        let rendered = tmpl
            .render()
            .map_err(|e| ReporterError::Format(e.to_string()))?;

        write_report_file(output_dir, &self.file_name, &rendered, &self.style, "HTML")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::fixtures::{empty_stats, file_fragment, report_to_file, tmp_dir};
    use cpd_core::models::{CpdClone, Statistics};

    fn run_html_report(clones: &[CpdClone], stats: &Statistics) -> String {
        report_to_file(
            "html",
            "jscpd-report.html",
            clones,
            stats,
            |_| {},
            HtmlReporter::new,
        )
    }

    #[test]
    fn empty_clones_produces_html() {
        let content = run_html_report(&[], &empty_stats());
        assert!(content.contains("<html"), "output must be HTML");
        assert!(content.contains("<body"), "output must have body");
    }

    /// A clone of `lines_a` of `a.js` and `lines_b` of `b.js`, both files
    /// written to a fresh temp dir.
    fn clone_of_two_files(
        text_a: &str,
        (start_a, end_a): (u32, u32),
        text_b: &str,
        (start_b, end_b): (u32, u32),
    ) -> CpdClone {
        let dir = tmp_dir("html");
        CpdClone::exact(
            "javascript".to_string(),
            file_fragment(&dir, "a.js", text_a, start_a, end_a),
            file_fragment(&dir, "b.js", text_b, start_b, end_b),
            50,
        )
    }

    fn report_with(clone: CpdClone) -> String {
        let mut stats = empty_stats();
        stats.total.clones = 1;
        run_html_report(&[clone], &stats)
    }

    #[test]
    fn html_names_both_locations_and_shows_the_code_of_both() {
        let content = report_with(clone_of_two_files(
            "hello\nworld\n",
            (1, 2),
            "hallo\nwelt\n",
            (1, 2),
        ));
        assert!(
            content.contains("a.js (Line 1:1 - Line 2:1)"),
            "HTML must name the first location: {content}"
        );
        assert!(
            content.contains("b.js (Line 1:1 - Line 2:1)"),
            "and the second: {content}"
        );
        assert!(
            content.contains(">hello\nworld</code></pre>"),
            "and show the code of the first fragment: {content}"
        );
        assert!(
            content.contains(">hallo\nwelt</code></pre>"),
            "and of the second (#1263): {content}"
        );
    }

    #[test]
    fn html_numbers_the_lines_of_each_fragment() {
        let text = "one\ntwo\nthree\nfour\nfive\n";
        let content = report_with(clone_of_two_files(text, (2, 3), text, (4, 5)));
        assert!(
            content.contains(">2\n3</pre><pre class=\"code-text\"><code class=\"language-javascript\">two\nthree</code></pre>"),
            "the first fragment's lines: {content}"
        );
        assert!(
            content.contains(">4\n5</pre><pre class=\"code-text\"><code class=\"language-javascript\">four\nfive</code></pre>"),
            "the second fragment's lines: {content}"
        );
    }

    #[test]
    fn html_escapes_the_code_of_both_fragments() {
        let text = "if (a < b && c > d) {}\n";
        let content = report_with(clone_of_two_files(text, (1, 1), text, (1, 1)));
        assert_eq!(
            content
                .matches("if (a &#60; b &#38;&#38; c &#62; d) {}")
                .count(),
            2,
            "both fragments are escaped: {content}"
        );
        assert!(!content.contains("a < b"), "no raw code: {content}");
    }

    #[test]
    fn html_carries_prism_for_the_languages_of_its_clones() {
        let content = report_with(clone_of_two_files("a\n", (1, 1), "a\n", (1, 1)));
        assert!(
            content.contains("<code class=\"language-javascript\">a</code>"),
            "the code names its language: {content}"
        );
        assert!(
            content.contains("Prism.languages.javascript="),
            "the page carries the javascript component"
        );
        assert!(
            !content.contains("Prism.languages.python="),
            "and no other language"
        );
    }

    #[test]
    fn html_without_a_prism_language_carries_no_prism() {
        let mut clone = clone_of_two_files("a\n", (1, 1), "a\n", (1, 1));
        clone.format = "txt".to_string();
        let content = report_with(clone);
        assert!(content.contains("<code>a</code>"), "{content}");
        assert!(!content.contains("MIT LICENSE"), "no Prism: {content}");
        let empty = run_html_report(&[], &empty_stats());
        assert!(!empty.contains("MIT LICENSE"), "no Prism without clones");
    }

    /// A clone of lines `start..=end` of `name`, known by its path with
    /// `suffix` after it, as a block embedded in the file is.
    fn clone_of_block(
        name: &str,
        text: &str,
        suffix: &str,
        format: &str,
        start: u32,
        end: u32,
    ) -> CpdClone {
        let mut fragment = file_fragment(&tmp_dir("html"), name, text, start, end);
        fragment.source_id.push_str(suffix);
        CpdClone::exact(format.to_string(), fragment.clone(), fragment, 50)
    }

    const GUIDE: &str =
        "# Guide\n\n```ts\nconst a = 1;\n```\n\nSome prose.\n\n```ts\nconst b = 2;\n```\n";

    #[test]
    fn html_highlights_only_the_block_lines_of_an_embedded_fragment() {
        let content = report_with(clone_of_block(
            "guide.md",
            GUIDE,
            ":typescript",
            "typescript",
            4,
            10,
        ));
        let expected = concat!(
            "<pre class=\"code-text\">",
            "<code class=\"language-typescript\">const a = 1;\n</code>",
            "<code>```\n\nSome prose.\n\n```ts\n</code>",
            "<code class=\"language-typescript\">const b = 2;</code>",
            "</pre>",
        );
        assert_eq!(
            content.matches(expected).count(),
            2,
            "both sides: {content}"
        );
    }

    #[test]
    fn html_highlights_markdown_prose_as_markdown() {
        let content = report_with(clone_of_block(
            "guide.md",
            GUIDE,
            ":markdown",
            "markdown",
            1,
            7,
        ));
        assert!(
            content.contains("<code class=\"language-markdown\"># Guide\n\n```ts\n"),
            "one run in the file's own format: {content}"
        );
    }

    #[test]
    fn html_leaves_a_block_of_an_unknown_file_plain() {
        let content = report_with(clone_of_block(
            "notes.unknown",
            "x = 1\n",
            ":python",
            "python",
            1,
            1,
        ));
        assert!(content.contains("<code>x = 1</code>"), "{content}");
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
