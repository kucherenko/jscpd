//! Functions to embed for semantic clones (`--semantic`).
//!
//! A unit is a function found by a [`FunctionExtractor`](crate::functions)
//! plus the text an embedding model sees: the function's own code, from the
//! name it is declared under (a method's key, the variable an arrow is
//! assigned to) to its end, with its comments removed and its indentation
//! kept relative to its first line, so two copies that differ only in
//! comments or nesting depth embed alike.
//! Comments go because a jscpd tokenizer drops them in weak mode, which
//! makes the rule the same for every language with an extractor.
//!
//! Vue, Svelte and Astro files contribute the functions of their `<script>`
//! blocks (and Astro frontmatter), positioned in the host file and grouped
//! by the block's format, the same format the block's detection source has.

use crate::functions::extract_functions;
use crate::functions::extractor_for;
use crate::line_index::LineIndex;
use crate::tokenizer::{Mode, tokenize};
use cpd_core::models::{Location, Token};

/// A function ready to embed, positioned in the file it was found in.
#[derive(Debug, Clone, PartialEq)]
pub struct RawUnit {
    /// Grammar of the extractor that found it.
    pub grammar: &'static str,
    pub name: String,
    pub start: Location,
    pub end: Location,
    /// The function's code without comments.
    pub text: String,
}

/// The units of one detection source of a file. For a single-format file
/// `format` is the file's own; for a component it is a script block's.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitMap {
    pub format: String,
    pub units: Vec<RawUnit>,
}

const COMPONENT_FORMATS: &[&str] = &["vue", "svelte", "astro"];

/// Formats [`extract_units`] finds functions in.
pub fn supports_units(format: &str) -> bool {
    COMPONENT_FORMATS.contains(&format) || extractor_for(format).is_some()
}

/// Every function of `source`, a file of `format`, as units to embed.
/// Empty for formats without an extractor and for sources that do not
/// parse.
pub fn extract_units(source: &str, format: &str) -> Vec<UnitMap> {
    if COMPONENT_FORMATS.contains(&format) {
        return component_units(source, format);
    }
    let units = units_in(source, format, 0, None);
    if units.is_empty() {
        return Vec::new();
    }
    vec![UnitMap {
        format: format.to_string(),
        units,
    }]
}

fn component_units(source: &str, file_format: &str) -> Vec<UnitMap> {
    let host = LineIndex::new(source.as_bytes());
    let mut maps: Vec<UnitMap> = Vec::new();
    for (format, range) in crate::sfc::script_blocks(source, file_format) {
        if extractor_for(&format).is_none() {
            continue;
        }
        let units = units_in(&source[range.clone()], &format, range.start, Some(&host));
        if units.is_empty() {
            continue;
        }
        match maps.iter_mut().find(|m| m.format == format) {
            Some(map) => map.units.extend(units),
            None => maps.push(UnitMap { format, units }),
        }
    }
    maps
}

/// Units of `code`, which starts at byte `shift` of the file whose line
/// index is `host` (`None` when `code` is the whole file).
fn units_in(code: &str, format: &str, shift: usize, host: Option<&LineIndex>) -> Vec<RawUnit> {
    let functions = extract_functions(code, format);
    if functions.is_empty() {
        return Vec::new();
    }
    let tokens = tokenize(format, code, Mode::Weak);
    let place = |loc: &Location| match host {
        Some(index) => index.location(shift + loc.offset as usize),
        None => loc.clone(),
    };
    functions
        .into_iter()
        .filter_map(|f| {
            let text = code_text(code, &tokens, f.head.offset as usize, f.end.offset as usize);
            (!text.is_empty()).then(|| RawUnit {
                grammar: f.grammar,
                start: place(&f.head),
                end: place(&f.end),
                name: f.name,
                text,
            })
        })
        .collect()
}

/// The source of the tokens inside `start..end`, joined by the whitespace
/// between them: a line break (with the next line's indentation) where the
/// gap has one, a single space where it has anything else. Whatever the
/// tokenizer skipped — comments in weak mode — is gone. Lines lose the
/// indentation of the function's first line, so the text reads as if the
/// function stood at the top level.
fn code_text(code: &str, tokens: &[Token], start: usize, end: usize) -> String {
    let first = tokens.partition_point(|t| (t.start.offset as usize) < start);
    let mut out = String::new();
    let mut prev_end: Option<usize> = None;
    for token in &tokens[first..] {
        let (from, to) = (token.start.offset as usize, token.end.offset as usize);
        if to > end {
            break;
        }
        if from < to && to <= code.len() && code.is_char_boundary(from) && code.is_char_boundary(to)
        {
            if let Some(prev) = prev_end.filter(|&p| p <= from) {
                let gap = &code[prev..from];
                match gap.rfind('\n') {
                    Some(nl) => {
                        out.push('\n');
                        let line_start = &gap[nl + 1..];
                        let indent = line_start.len() - line_start.trim_start().len();
                        out.push_str(&line_start[..indent]);
                    }
                    None if !gap.is_empty() => out.push(' '),
                    None => {}
                }
            }
            out.push_str(&code[from..to]);
            prev_end = Some(to);
        }
    }
    let line_start = code[..start.min(code.len())]
        .rfind('\n')
        .map_or(0, |nl| nl + 1);
    let base = code[line_start..start.min(code.len())]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    dedent(&out, base)
}

/// Remove up to `base` leading whitespace characters from every line after
/// the first (which starts at the function's first token).
fn dedent(text: &str, base: usize) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            let strip = line
                .char_indices()
                .take_while(|(k, c)| *k < base && c.is_whitespace())
                .count();
            out.push_str(&line[strip..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_units_lose_comments_and_shared_indentation() {
        let src = "impl Cart {\n    /// Sum of the lines.\n    pub fn total(&self) -> i64 {\n        // cents\n        self.lines.iter().map(|l| l.price /* each */ * l.qty).sum()\n    }\n}\n";
        let maps = extract_units(src, "rust");
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].format, "rust");
        let unit = &maps[0].units[0];
        assert_eq!(unit.name, "total");
        assert_eq!(unit.grammar, "rust");
        assert_eq!((unit.start.line, unit.end.line), (3, 6));
        assert_eq!(
            unit.text,
            "pub fn total(&self) -> i64 {\n    self.lines.iter().map(|l| l.price * l.qty).sum()\n}"
        );
    }

    #[test]
    fn typescript_units_keep_code_and_drop_comments() {
        let src = "// helpers\nexport function slug(t: string): string {\n  /* lower */\n  return t.toLowerCase().replace(/[^a-z0-9]+/g, '-'); // dash\n}\n";
        let maps = extract_units(src, "typescript");
        let unit = &maps[0].units[0];
        assert_eq!(
            (unit.name.as_str(), unit.start.line, unit.end.line),
            ("slug", 2, 5)
        );
        // The function node starts after `export`, as `--similarity` reports it.
        assert_eq!(
            unit.text,
            "function slug(t: string): string {\n  return t.toLowerCase().replace(/[^a-z0-9]+/g, '-');\n}"
        );
    }

    #[test]
    fn typescript_units_start_at_the_name_they_are_declared_under() {
        let src = "class Stream {\n  async segment(n: bigint): Promise<void> {\n    return run(n);\n  }\n  handle = (e: Event) => log(e);\n}\nexport const total = (xs: number[]) => xs.reduce((a, b) => a + b, 0);\nconst api = { fetchAll(url: string) { return get(url); }, save: async (x: number) => put(x) };\nconst later = debounce(() => refresh(), 300);\n";
        let maps = extract_units(src, "typescript");
        let texts: Vec<(&str, &str)> = maps[0]
            .units
            .iter()
            .map(|u| (u.name.as_str(), u.text.as_str()))
            .collect();
        assert_eq!(
            texts,
            vec![
                (
                    "segment",
                    "segment(n: bigint): Promise<void> {\n  return run(n);\n}"
                ),
                ("handle", "handle = (e: Event) => log(e)"),
                ("<arrow>", "(a, b) => a + b"),
                (
                    "total",
                    "total = (xs: number[]) => xs.reduce((a, b) => a + b, 0)"
                ),
                ("fetchAll", "fetchAll(url: string) { return get(url); }"),
                ("save", "save: async (x: number) => put(x)"),
                // Not the variable's value, so not under its name.
                ("later", "() => refresh()"),
            ]
        );
        let start = &maps[0].units[0].start;
        assert_eq!(start.line, 2);
        assert!(src[start.offset as usize..].starts_with("segment("));
    }

    #[test]
    fn svelte_script_functions_are_placed_in_the_host_file() {
        let src = "<script lang=\"ts\">\n  let n = $state(0);\n\n  function double(x: number): number {\n    return x * 2;\n  }\n</script>\n\n<button onclick={() => (n = double(n))}>{n}</button>\n";
        let maps = extract_units(src, "svelte");
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].format, "typescript");
        let unit = &maps[0].units[0];
        assert_eq!(unit.name, "double");
        assert_eq!((unit.start.line, unit.end.line), (4, 6));
        assert_eq!(
            &src[unit.start.offset as usize..unit.start.offset as usize + 8],
            "function"
        );
        assert_eq!(
            unit.text,
            "function double(x: number): number {\n  return x * 2;\n}"
        );
    }

    #[test]
    fn vue_blocks_of_one_format_share_a_map() {
        let src = "<template><p>{{ a }}</p></template>\n<script>\nexport function one() { return 1; }\n</script>\n<script setup>\nfunction two() { return 2; }\n</script>\n";
        let maps = extract_units(src, "vue");
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].format, "javascript");
        let names: Vec<&str> = maps[0].units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["one", "two"]);
        assert_eq!(maps[0].units[1].start.line, 6);
    }

    #[test]
    fn formats_without_an_extractor_have_no_units() {
        assert!(supports_units("rust") && supports_units("svelte") && supports_units("tsx"));
        assert!(supports_units("python"));
        assert!(!supports_units("ruby") && !supports_units("markdown"));
        assert!(extract_units("def f\n  1\nend\n", "ruby").is_empty());
        assert!(extract_units("<style>p { color: red }</style>", "svelte").is_empty());
    }

    #[test]
    fn crlf_sources_give_the_same_text() {
        let lf = "pub fn alpha(x: u32) -> u32 {\n    let y = x + 1; // one\n    y * 2\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        let text = |src: &str| extract_units(src, "rust")[0].units[0].text.clone();
        assert_eq!(text(&crlf), text(lf));
        assert_eq!(
            text(lf),
            "pub fn alpha(x: u32) -> u32 {\n    let y = x + 1;\n    y * 2\n}"
        );
    }

    #[test]
    fn python_units_drop_comments_but_keep_docstrings() {
        let src = "class Cart:\n    def total(self):\n        \"\"\"Sum of the lines.\"\"\"\n        # cents\n        return sum(l.price * l.qty for l in self.lines)  # all\n";
        let unit = &extract_units(src, "python")[0].units[0];
        assert_eq!(unit.grammar, "python");
        assert_eq!(
            unit.text,
            "def total(self):\n    \"\"\"Sum of the lines.\"\"\"\n    return sum(l.price * l.qty for l in self.lines)"
        );
    }

    #[test]
    fn dedent_strips_the_first_line_indentation() {
        assert_eq!(
            dedent(
                "fn f() {\n        if x {\n            y\n        }\n    }",
                4
            ),
            "fn f() {\n    if x {\n        y\n    }\n}"
        );
        assert_eq!(dedent("one line", 8), "one line");
        assert_eq!(
            dedent("def f():\n  x\n\ty", 4),
            "def f():\nx\ny",
            "never more than the line has"
        );
    }
}
