// html.rs — HTML reporter matching TypeScript jscpd HTML report layout
// Uses embedded CSS matching Tailwind v2 color scheme and layout.

use crate::context::ReportContext;
use crate::prism;
use crate::reporter::{Reporter, ReporterError, ReporterOptions};
use crate::shared::{
    Style, clean_source_id, extract_lines, resolve_fragment_path, write_report_file,
};
use askama::Template;
use cpd_core::models::{CloneKind, CpdClone, Fragment};
use cpd_tokenizer::{formats, markdown, sfc};
use std::collections::{BTreeMap, HashMap};
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
    /// The side of `fragment`, in `format` unless the fragment is a block of
    /// another file, which names its own.
    fn new(files: &mut Files, fragment: &Fragment, format: &str) -> Self {
        let format = block_format(&fragment.source_id).unwrap_or(format);
        let file = files.get(fragment);
        // A lone carriage return ends no line for jscpd, but the browser
        // breaks the line there, and the numbers below it would be off.
        let code = file
            .lines(fragment.start.line, fragment.end.line)
            .replace('\r', "\u{240d}");
        let rows = if code.is_empty() {
            0
        } else {
            code.split('\n').count() as u32
        };
        let numbers = (fragment.start.line..fragment.start.line + rows)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let segments = segments(file, fragment, format, code);
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

/// The files the page shows code from, each read, indexed and split once,
/// however many clones it holds.
#[derive(Default)]
struct Files(HashMap<String, SourceText>);

/// A file's text, where its lines start and, once a fragment of a block in
/// it asks, its blocks.
struct SourceText {
    text: String,
    /// Line `n` starts at `line_starts[n - 1]`.
    line_starts: Vec<usize>,
    /// The blocks in the order of the file.
    blocks: Option<Vec<Block>>,
}

/// A block of a Markdown or component file: the lines it covers, its format
/// and the Prism language of that format, empty for none.
struct Block {
    lines: Range<u32>,
    format: String,
    lang: &'static str,
}

impl Files {
    fn get(&mut self, fragment: &Fragment) -> &mut SourceText {
        self.0
            .entry(resolve_fragment_path(fragment))
            .or_insert_with_key(|path| {
                SourceText::new(std::fs::read_to_string(path).unwrap_or_default())
            })
    }
}

impl SourceText {
    fn new(text: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        Self {
            text,
            line_starts,
            blocks: None,
        }
    }

    /// Lines `start..=end`, as [`extract_lines`] reads them from the whole
    /// text, without walking the lines before them.
    fn lines(&self, start: u32, end: u32) -> String {
        let from = self
            .line_starts
            .get(start.saturating_sub(1) as usize)
            .map_or(self.text.len(), |&at| at);
        let count = end.saturating_sub(start.saturating_sub(1));
        extract_lines(&self.text[from..], 1, count)
    }

    /// The blocks of the file `source_id` names a block of, with the lines
    /// each covers: a line is a block's when it starts inside it. Markdown,
    /// Vue, Svelte and Astro files are split as the tokenizer splits them,
    /// once. A file of another kind has no blocks: a mapping of the user's
    /// split it, and the report does not know as what.
    fn blocks(&mut self, source_id: &str) -> &[Block] {
        self.blocks.get_or_insert_with(|| {
            let blocks = match extension_format(source_id) {
                Some("markdown") => markdown::embedded_blocks(&self.text),
                Some(component @ ("vue" | "svelte" | "astro")) => {
                    sfc::embedded_blocks(&self.text, component)
                }
                _ => Vec::new(),
            };
            let line = |offset: usize| {
                self.line_starts.partition_point(|&start| start < offset) as u32 + 1
            };
            blocks
                .into_iter()
                .map(|(format, range)| Block {
                    lines: line(range.start)..line(range.end),
                    lang: prism::language(&format).unwrap_or_default(),
                    format,
                })
                .collect()
        })
    }
}

/// The format a block embedded in another file names in its source id
/// (`guide.md:typescript`), None for a whole file.
fn block_format(source_id: &str) -> Option<&str> {
    let path = clean_source_id(source_id);
    source_id
        .get(path.len() + 1..)
        .filter(|format| !format.is_empty())
}

/// The format jscpd gives the file `source_id` is or is a block of by its
/// extension.
fn extension_format(source_id: &str) -> Option<&'static str> {
    Path::new(clean_source_id(source_id))
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(formats::get_format_by_extension)
}

/// The lines of a fragment in runs that Prism highlights alike. A fragment of
/// a whole file is one run. A fragment of a block embedded in another file
/// (`guide.md:typescript`) can take in the lines between two blocks, such as
/// Markdown prose and fences, and those stay plain. Markdown prose and the
/// markup of a component (`html`) are the text the other blocks of the file
/// sit in: a fragment of them shows each block it takes in in the block's
/// language.
fn segments(
    file: &mut SourceText,
    fragment: &Fragment,
    format: &str,
    code: String,
) -> Vec<Segment> {
    let lang = prism::language(format).unwrap_or_default();
    let host = matches!(format, "markdown" | "html");
    if block_format(&fragment.source_id).is_none() || (lang.is_empty() && !host) {
        return vec![Segment { lang, text: code }];
    }
    let blocks = file.blocks(&fragment.source_id);
    let line_lang = |line: u32| {
        let at = blocks.partition_point(|block| block.lines.end <= line);
        match blocks.get(at).filter(|block| block.lines.contains(&line)) {
            Some(block) if host || block.format == format => block.lang,
            Some(_) => "",
            None if host => lang,
            None => "",
        }
    };
    let lines: Vec<(&'static str, &str)> = (fragment.start.line..)
        .zip(code.split_inclusive('\n'))
        .map(|(line, text)| (line_lang(line), text))
        .collect();
    lines
        .chunk_by(|a, b| a.0 == b.0)
        .map(|run| Segment {
            lang: run[0].0,
            text: run.iter().map(|(_, text)| *text).collect(),
        })
        .collect()
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
        let mut files = Files::default();

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
            // `--similarity` and `--semantic` pair files of two formats, such
            // as JavaScript and TypeScript, and the clone is in the format of
            // the first one. When the extensions of the two files name two
            // formats, the second file is shown in its own.
            let format_b = match clone.kind {
                CloneKind::Similar | CloneKind::Semantic => {
                    match extension_format(&clone.fragment_b.source_id) {
                        Some(b) if extension_format(&clone.fragment_a.source_id) != Some(b) => b,
                        _ => &clone.format,
                    }
                }
                _ => &clone.format,
            };
            let view = CloneView {
                a: SideView::new(&mut files, &clone.fragment_a, &clone.format),
                b: SideView::new(&mut files, &clone.fragment_b, format_b),
            };
            group_map
                .entry(clone.format.clone())
                .or_default()
                .push(view);
        }

        let clone_groups: Vec<CloneGroup> = group_map
            .into_iter()
            .map(|(format, clones)| CloneGroup { format, clones })
            .collect();
        // The page carries the Prism languages its code is shown in.
        let prism = prism::script(
            clone_groups
                .iter()
                .flat_map(|group| &group.clones)
                .flat_map(CloneView::sides)
                .flat_map(|side| &side.segments)
                .map(|segment| segment.lang),
        );

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
            prism,
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
    use cpd_core::models::{CpdClone, Location, Statistics};

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
        let expected = concat!(
            "<pre class=\"code-text\">",
            "<code class=\"language-markdown\"># Guide\n\n```ts\n</code>",
            "<code class=\"language-typescript\">const a = 1;\n</code>",
            "<code class=\"language-markdown\">```\n\nSome prose.</code>",
            "</pre>",
        );
        assert_eq!(
            content.matches(expected).count(),
            2,
            "prose in the file's own format, a fence in its own: {content}"
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
        assert!(
            !content.contains("MIT LICENSE"),
            "and no Prism that nothing uses: {content}"
        );
    }

    #[test]
    fn html_highlights_the_markup_of_a_component_and_the_blocks_in_it() {
        // The markup of a Svelte file is the text around its blocks, so a
        // fragment of it can take in a script, shown as a script.
        let svelte = "<script>\n  let count = 0;\n</script>\n\n<button on:click={() => count++}>\n  Clicked {count} times\n</button>\n";
        let content = report_with(clone_of_block(
            "Counter.svelte",
            svelte,
            ":html",
            "html",
            1,
            7,
        ));
        assert!(
            content.contains(concat!(
                "<pre class=\"code-text\">",
                "<code class=\"language-markup\">&#60;script&#62;\n</code>",
                "<code class=\"language-javascript\">  let count = 0;\n</code>",
                "<code class=\"language-markup\">&#60;/script&#62;\n\n&#60;button"
            )),
            "{content}"
        );
        assert!(content.contains("Prism.languages.markup="), "{content}");
        assert!(content.contains("Prism.languages.javascript="), "{content}");

        // The frontmatter of an Astro file is TypeScript.
        let astro = "---\nconst posts: Array<Post> = [];\n---\n<ul>{posts.length}</ul>\n";
        let content = report_with(clone_of_block("Posts.astro", astro, ":html", "html", 1, 4));
        assert!(
            content.contains(
                "<code class=\"language-typescript\">const posts: Array&#60;Post&#62; = [];\n</code>"
            ),
            "{content}"
        );

        // The markup of a Vue file is its template.
        let vue = "<template>\n  <p>{{ a }}</p>\n</template>\n";
        let content = report_with(clone_of_block("App.vue", vue, ":html", "html", 2, 2));
        assert!(
            content.contains(
                "<code class=\"language-markup\">  &#60;p&#62;{{ a }}&#60;/p&#62;</code>"
            ),
            "{content}"
        );
    }

    #[test]
    fn html_highlights_front_matter_as_yaml() {
        let post = "---\ntitle: Guide\ntags: [a, b]\n---\n\n# Guide\n";
        let content = report_with(clone_of_block("post.md", post, ":yaml", "yaml", 2, 3));
        assert!(
            content.contains("<code class=\"language-yaml\">title: Guide\ntags: [a, b]</code>"),
            "{content}"
        );
    }

    #[test]
    fn html_shows_each_side_in_its_own_language() {
        // `--cross-formats` pairs a TypeScript file with a JavaScript fence,
        // and the clone is in the format of its first side.
        let dir = tmp_dir("html");
        let guide = "# Sum\n\n```js\nconst sum = (a, b) => a + b;\n```\n";
        let mut fence = file_fragment(&dir, "guide.md", guide, 4, 4);
        fence.source_id.push_str(":javascript");
        let ts = "const sum = (a: number, b: number) => a + b;\n";
        let clone = CpdClone::exact(
            "typescript",
            file_fragment(&dir, "sum.ts", ts, 1, 1),
            fence,
            20,
        );
        let content = report_with(clone);
        assert!(
            content.contains("<code class=\"language-typescript\">const sum = (a: number"),
            "{content}"
        );
        assert!(
            content.contains("<code class=\"language-javascript\">const sum = (a, b)"),
            "{content}"
        );
    }

    #[test]
    fn html_shows_the_second_file_of_a_pair_of_two_languages_in_its_own() {
        // `--semantic` can pair any two languages.
        let dir = tmp_dir("html");
        let py = "def total(xs):\n    return sum(xs)\n";
        let rs = "fn total(xs: &[u32]) -> u32 {\n    xs.iter().sum()\n}\n";
        let mut clone = CpdClone::exact(
            "python",
            file_fragment(&dir, "total.py", py, 1, 2),
            file_fragment(&dir, "total.rs", rs, 1, 3),
            20,
        );
        clone.kind = CloneKind::Semantic;
        let content = report_with(clone);
        assert!(
            content.contains("<code class=\"language-python\">def total"),
            "{content}"
        );
        assert!(
            content.contains("<code class=\"language-rust\">fn total"),
            "{content}"
        );
        assert!(
            content.contains("languages.rust="),
            "the page carries both languages"
        );

        // `--similarity` pairs JavaScript with TypeScript.
        let js = "function total(xs) {\n  return xs.length;\n}\n";
        let ts = "function total(xs: number[]): number {\n  return xs.length;\n}\n";
        let mut clone = CpdClone::exact(
            "javascript",
            file_fragment(&dir, "total.js", js, 1, 3),
            file_fragment(&dir, "total.ts", ts, 1, 3),
            20,
        );
        clone.kind = CloneKind::Similar;
        let content = report_with(clone);
        assert!(
            content.contains("<code class=\"language-typescript\">function total(xs: number[])"),
            "{content}"
        );
    }

    #[test]
    fn html_keeps_a_lone_carriage_return_on_its_line() {
        // A lone CR ends no line for jscpd, and the browser would start a
        // new line there, so the numbers below it would be off by one.
        let text = "let a = 1; // one\rtwo\nlet b = 2;\n";
        let content = report_with(clone_of_two_files(text, (1, 2), text, (1, 2)));
        assert!(
            content.contains(">1\n2</pre><pre class=\"code-text\"><code class=\"language-javascript\">let a = 1; // one\u{240d}two\nlet b = 2;</code>"),
            "{content}"
        );
        assert!(!content.contains("one\rtwo"), "{content}");
    }

    #[test]
    fn html_keeps_the_format_out_of_the_page_scripts() {
        // A format can be any name `--formats-exts` or a config file gives.
        let mut clone = clone_of_two_files("a\n", (1, 1), "a\n", (1, 1));
        clone.format = "x'+alert(1)+'".to_string();
        let content = report_with(clone);
        assert!(
            content.contains("onclick=\"toggle(this.dataset.code)\""),
            "{content}"
        );
        assert!(!content.contains("toggle('code-"), "{content}");
    }

    #[test]
    fn html_splits_a_file_once_however_many_clones_it_holds() {
        // Every side of a clone in a block split the whole file again: a
        // README of 450 KB with 500 clones took 20 s in a debug build.
        let fences = 2000;
        let readme: String = (0..fences)
            .map(|n| format!("```ts\nexport const value{n} = compute({n});\n```\n\nNote {n}.\n\n"))
            .collect();
        let path = tmp_dir("html").join("README.md");
        std::fs::write(&path, &readme).unwrap();
        let fence = |n: u32| {
            let line = Location::new(n * 6 + 2, 0, 0);
            let source_id = format!("{}:typescript", path.to_string_lossy());
            Fragment::new(source_id, line.clone(), line, [0, 0])
        };
        let clones: Vec<CpdClone> = (0..fences / 2)
            .map(|n| CpdClone::exact("typescript", fence(2 * n), fence(2 * n + 1), 10))
            .collect();
        let started = std::time::Instant::now();
        let content = run_html_report(&clones, &empty_stats());
        let elapsed = started.elapsed();
        assert_eq!(
            content
                .matches("<code class=\"language-typescript\">export const value")
                .count(),
            fences as usize
        );
        assert!(elapsed < std::time::Duration::from_secs(10), "{elapsed:?}");
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
