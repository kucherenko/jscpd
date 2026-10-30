//! Behavior of the tokenizers as clone detection sees it: which snippets
//! tokenize to equal streams, which code is left out, and that no input —
//! however malformed — panics or yields a token outside the source.

use cpd_core::models::{DetectionToken, TokenKind};
use cpd_tokenizer::formats::{get_format_by_shebang, resolve_format};
use cpd_tokenizer::markdown::code_blocks;
use cpd_tokenizer::sfc::script_blocks;
use cpd_tokenizer::tokenizer::{
    Mode, TokenMap, TokenizeOptions, code_ignore_ranges, compile_ignore_patterns, ignored_ranges,
    tokenize, tokenize_to_detection, tokenize_to_detection_maps,
};
use cpd_tokenizer::{markdown, sfc};

fn opts(mode: Mode) -> TokenizeOptions {
    TokenizeOptions::new(mode)
}

fn hashes(format: &str, source: &str, options: &TokenizeOptions) -> Vec<u64> {
    tokenize_to_detection(format, source, options)
        .iter()
        .map(|t| t.hash)
        .collect()
}

/// The source text each detection token stands for.
fn texts<'a>(source: &'a str, tokens: &[DetectionToken]) -> Vec<&'a str> {
    tokens
        .iter()
        .map(|t| &source[t.range[0]..t.range[1]])
        .collect()
}

fn map<'a>(maps: &'a [TokenMap], format: &str) -> Option<&'a TokenMap> {
    maps.iter().find(|m| m.format == format)
}

fn formats_of(maps: &[TokenMap]) -> Vec<&str> {
    maps.iter().map(|m| m.format.as_str()).collect()
}

/// Every token lies inside `source`, on character boundaries, and its line
/// and column agree with its byte offset.
#[track_caller]
fn assert_tokens_sit_in_source(label: &str, source: &str, tokens: &[DetectionToken]) {
    for t in tokens {
        let [start, end] = t.range;
        assert!(
            start <= end && end <= source.len(),
            "{label}: {:?} outside a source of {} bytes",
            t.range,
            source.len()
        );
        assert!(
            source.is_char_boundary(start) && source.is_char_boundary(end),
            "{label}: {:?} splits a character",
            t.range
        );
        assert_eq!(t.start.offset as usize, start, "{label}: start offset");
        let line = source[..start].matches('\n').count() as u32 + 1;
        assert_eq!(t.start.line, line, "{label}: line of {:?}", t.range);
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        assert_eq!(
            t.start.column as usize,
            start - line_start,
            "{label}: column of {:?}",
            t.range
        );
    }
}

const ALL_FORMATS: &[&str] = &[
    "javascript",
    "typescript",
    "jsx",
    "tsx",
    "python",
    "java",
    "css",
    "html",
    "markdown",
    "vue",
    "svelte",
    "astro",
    "razor",
    "sql",
    "lua",
    "txt",
];

// ── hostile input ────────────────────────────────────────────────────────

#[test]
fn hostile_inputs_never_panic_and_every_token_sits_in_the_source() {
    let hostile = [
        "const a = \"never closed\nconst b = 2;\n",
        "/* a block comment that never closes\nint x = 1;\n",
        "x = 1 \"\"\" open triple\n",
        "`template ${ never",
        "line one\r\nline two\r\n\r\n",
        "lone\rcarriage\rreturns",
        "\u{feff}const bom = 1;\n",
        "const ü = 'é'; // ünïcödé\n",
        "名前 = \"値\"\n🦀🦀🦀\n",
        "é",
        "\u{feff}",
        "\0\0\0",
        "```js\nconst open = 1;\n",
        "---\ntitle: open front matter\n",
        "<script>\nconst unclosed = 1;\n",
        "<template><div></template><script lang=\"ts\">let x: number = 1;</script><style",
        "@{ var a = 1;\n<p>@a</p>",
        "@code { \"never closed }\n",
        &String::from_utf8_lossy(b"valid \xff\xfe invalid \xc3 bytes\n"),
        "\n\n\n",
        "{{{{((((",
    ];
    for format in ALL_FORMATS {
        for source in &hostile {
            for mode in [Mode::Mild, Mode::Weak, Mode::Strict] {
                let label = format!("{format} {mode:?} {source:?}");
                let maps = tokenize_to_detection_maps(format, source, &opts(mode));
                for m in &maps {
                    assert_tokens_sit_in_source(&label, source, &m.tokens);
                }
                for token in tokenize(format, source, mode) {
                    let (s, e) = (token.start.offset as usize, token.end.offset as usize);
                    assert!(s <= e && e <= source.len(), "{label}: {token:?}");
                    // Blanked markup around embedded code stays whitespace
                    // of the same length; everything else is its own text.
                    if token.kind != TokenKind::Whitespace {
                        assert_eq!(&source[s..e], token.value, "{label}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_crlf_file_tokenizes_like_its_lf_twin() {
    let cases = [
        (
            "python",
            "def total(items):\n    s = 0\n    for i in items:\n        s += i\n    return s\n",
        ),
        (
            "javascript",
            "function total(items) {\n  let s = 0;\n  for (const i of items) s += i;\n  return s;\n}\n",
        ),
        (
            "java",
            "class A {\n  /* doc\n     more */\n  int f() { return 1; }\n}\n",
        ),
        (
            "markdown",
            "# Title\n\nProse.\n\n```python\nx = 1\ny = 2\n```\n\nMore.\n",
        ),
        (
            "vue",
            "<template>\n  <p>{{ a }}</p>\n</template>\n<script>\nexport default { data() { return { a: 1 }; } };\n</script>\n",
        ),
    ];
    for (format, lf) in cases {
        let crlf = lf.replace('\n', "\r\n");
        let lf_maps = tokenize_to_detection_maps(format, lf, &opts(Mode::Mild));
        let crlf_maps = tokenize_to_detection_maps(format, &crlf, &opts(Mode::Mild));
        assert_eq!(formats_of(&lf_maps), formats_of(&crlf_maps), "{format}");
        for (a, b) in lf_maps.iter().zip(&crlf_maps) {
            let ha: Vec<u64> = a.tokens.iter().map(|t| t.hash).collect();
            let hb: Vec<u64> = b.tokens.iter().map(|t| t.hash).collect();
            assert_eq!(
                ha, hb,
                "{format}/{}: CRLF must not change the stream",
                a.format
            );
            // And the positions are those of the CRLF file itself.
            assert_tokens_sit_in_source(format, &crlf, &b.tokens);
            assert_eq!(
                a.tokens.iter().map(|t| t.start.line).collect::<Vec<_>>(),
                b.tokens.iter().map(|t| t.start.line).collect::<Vec<_>>(),
                "{format}: line numbers"
            );
        }
    }
}

#[test]
fn a_byte_order_mark_does_not_change_the_code_tokens() {
    for (format, code) in [
        ("javascript", "const total = items.length;\n"),
        ("python", "total = len(items)\n"),
        ("java", "int total = items.size();\n"),
    ] {
        let with_bom = format!("\u{feff}{code}");
        let plain = tokenize_to_detection(format, code, &opts(Mode::Mild));
        let bom = tokenize_to_detection(format, &with_bom, &opts(Mode::Mild));
        assert_tokens_sit_in_source(format, &with_bom, &bom);
        // Whatever the BOM itself becomes, the code after it is the same.
        let code_tokens: Vec<&str> = texts(&with_bom, &bom)
            .into_iter()
            .filter(|t| *t != "\u{feff}")
            .collect();
        assert_eq!(code_tokens, texts(code, &plain), "{format}");
    }
}

#[test]
fn multibyte_characters_keep_tokens_on_their_own_text() {
    let js = "const ключ = 'значение'; // коммент\nlet ü = ключ;\n";
    let tokens = tokenize_to_detection("javascript", js, &opts(Mode::Mild));
    assert_tokens_sit_in_source("js", js, &tokens);
    let t = texts(js, &tokens);
    assert!(t.contains(&"ключ"), "{t:?}");
    assert!(t.contains(&"'значение'"), "{t:?}");
    assert!(t.contains(&"ü"), "{t:?}");

    let py = "naïve = \"日本語\"  # ç\nprint(naïve)\n";
    let tokens = tokenize_to_detection("python", py, &opts(Mode::Mild));
    assert_tokens_sit_in_source("py", py, &tokens);
    assert!(texts(py, &tokens).contains(&"naïve"));
    assert!(texts(py, &tokens).contains(&"\"日本語\""));

    // A fence after multibyte prose: the embedded tokens still point at
    // their own bytes in the host file.
    let md = "# Заголовок ✓\n\nТекст — ещё.\n\n```js\nconst é = 'ü';\n```\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    let js_map = map(&maps, "javascript").expect("a javascript map");
    assert_tokens_sit_in_source("md", md, &js_map.tokens);
    assert_eq!(texts(md, &js_map.tokens), ["const", "é", "=", "'ü'", ";"]);

    let vue = "<template><p>Привет, мир ✓</p></template>\n<script>\nconst é = 'ü';\n</script>\n";
    let maps = tokenize_to_detection_maps("vue", vue, &opts(Mode::Mild));
    let js_map = map(&maps, "javascript").expect("a javascript map");
    assert_tokens_sit_in_source("vue", vue, &js_map.tokens);
    assert_eq!(texts(vue, &js_map.tokens), ["const", "é", "=", "'ü'", ";"]);
}

#[test]
fn an_unterminated_string_or_comment_still_yields_the_code_before_it() {
    // JavaScript the parser gives up on falls back to a word split that
    // still sees the code.
    let js = "function keep(a) { return a + 1; }\nconst broken = \"never closed\n";
    let t = texts(
        js,
        &tokenize_to_detection("javascript", js, &opts(Mode::Mild)),
    );
    assert!(t.contains(&"keep"), "{t:?}");
    assert!(t.contains(&"return"), "{t:?}");

    // In a C-style language an open block comment hides what follows it, and
    // only that: the code before it is untouched.
    let java = "int kept = 1;\n/* open\nint hidden = 2;\n";
    let weak = tokenize_to_detection("java", java, &opts(Mode::Weak));
    let t = texts(java, &weak);
    assert!(t.contains(&"kept"), "{t:?}");
    assert!(!t.contains(&"hidden"), "{t:?}");

    // An unterminated Python string ends with its line.
    let py = "a = 'open\nb = 2\n";
    let t = texts(py, &tokenize_to_detection("python", py, &opts(Mode::Mild)));
    assert!(t.contains(&"b"), "{t:?}");
}

#[test]
fn an_unclosed_markdown_fence_is_prose_not_code() {
    let md = "Intro.\n\n```js\nconst open = 1;\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    assert!(
        map(&maps, "javascript").is_none(),
        "{:?}",
        formats_of(&maps)
    );
    let prose = map(&maps, "markdown").expect("prose");
    assert!(texts(md, &prose.tokens).contains(&"open"));
}

/// A `---` on the first line is front matter only when the next line is not
/// blank and a line that is `---` or `...` from its first column closes it
/// before any code fence opens (issue #1223). Otherwise it is a thematic
/// break, and the fences stay fences: front matter and a fence used to
/// overlap, and blanking them panicked.
#[test]
fn front_matter_holds_no_fence() {
    // A thematic break, then the format and a word of the map that the
    // page's code lands in.
    let rules = [
        // The closing `---` inside a fence.
        (
            "---\n```js\n---\nconst a = 1;\n```\n",
            "javascript",
            "const",
        ),
        // The issue's page: the `...` of a Python stub inside a fence.
        (
            "---\n\nThe protocol a store implements.\n\n```python\nclass Store(Protocol):\n    def get(self, key: str) -> bytes:\n        ...\n```\n",
            "python",
            "Protocol",
        ),
        // A fence inside what would be front matter.
        (
            "---\n```js\nconst a = 1;\n```\n---\n\nprose\n",
            "javascript",
            "const",
        ),
        // A fence that opens inside front matter and holds its closer.
        (
            "---\ntitle: x\n```js\n---\nlet b = 2;\n```\n\nprose\n",
            "javascript",
            "let",
        ),
        // The `...` of a stub in a fence of a list item, indented.
        (
            "---\nSteps:\n\n1. Implement the store:\n\n   ```python\n   class Store(Protocol):\n       def get(self, key: str) -> bytes:\n           ...\n   ```\n",
            "markdown",
            "Protocol",
        ),
        // A fence that never closes, before a closer.
        (
            "---\ntitle: x\n```js\nconst a = 1;\n...\n",
            "markdown",
            "const",
        ),
        // A second rule before the first fence.
        (
            "---\n\n# Title\n\nIntro text.\n\n---\n\n## Usage\n\n```js\nconst a = 1;\n```\n",
            "markdown",
            "Intro",
        ),
    ];
    for (md, format, word) in rules {
        for mode in [Mode::Mild, Mode::Weak, Mode::Strict] {
            let maps = tokenize_to_detection_maps("markdown", md, &opts(mode));
            assert!(
                map(&maps, "yaml").is_none(),
                "{md:?}: {:?}",
                formats_of(&maps)
            );
            let code = map(&maps, format).expect("the code");
            assert!(texts(md, &code.tokens).contains(&word), "{md:?}");
            // Strict keeps the whitespace that blanks a fence in the prose.
            if mode != Mode::Strict {
                assert_maps_apart(md, &maps);
            }
            let display = tokenize("markdown", md, mode);
            assert!(display.iter().any(|t| t.value == word), "{md:?}");
        }
    }

    // Front matter that closes before the first fence, and front matter
    // whose block scalar holds an indented `...`.
    let md = "---\ntitle: Guide\n---\n\nSee the code.\n\n```js\nconst a = 1;\n```\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    assert_eq!(formats_of(&maps), ["markdown", "javascript", "yaml"]);
    assert_maps_apart(md, &maps);
    let md = "---\ntitle: Stubs\nexample: |\n  def get(self, key):\n      ...\nauthor: someone\n---\n\nProse.\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    let yaml = texts(md, &map(&maps, "yaml").expect("front matter").tokens);
    assert!(
        yaml.contains(&"author") && yaml.contains(&"someone"),
        "{yaml:?}"
    );
    assert_eq!(
        texts(md, &map(&maps, "markdown").expect("prose").tokens),
        ["Prose", "."]
    );
}

/// No byte of `source` lies in the tokens of two maps.
fn assert_maps_apart(source: &str, maps: &[TokenMap]) {
    let mut owner: Vec<Option<&str>> = vec![None; source.len()];
    for m in maps {
        for t in &m.tokens {
            for (byte, slot) in owner
                .iter_mut()
                .enumerate()
                .take(t.range[1])
                .skip(t.range[0])
            {
                match *slot {
                    Some(other) if other != m.format => panic!(
                        "{source:?}: byte {byte} is in a {other} token and a {} token",
                        m.format
                    ),
                    _ => *slot = Some(&m.format),
                }
            }
        }
    }
}

#[test]
fn an_unclosed_component_block_leaves_the_file_as_markup() {
    let vue = "<template><p>hi</p></template>\n<script>\nconst unclosed = 1;\n";
    let maps = tokenize_to_detection_maps("vue", vue, &opts(Mode::Mild));
    assert!(
        map(&maps, "javascript").is_none(),
        "{:?}",
        formats_of(&maps)
    );
    // A file whose blocks never close is all markup.
    let svelte = "<script>\nlet a = 1;\n";
    let maps = tokenize_to_detection_maps("svelte", svelte, &opts(Mode::Mild));
    assert_eq!(formats_of(&maps), ["html"]);
}

// ── modes ────────────────────────────────────────────────────────────────

#[test]
fn modes_decide_whether_comments_and_whitespace_tell_code_apart() {
    let a = "x = 1  # first note\ny = x\n";
    let b = "x = 1  # second note\ny = x\n";
    assert_eq!(
        hashes("python", a, &opts(Mode::Weak)),
        hashes("python", b, &opts(Mode::Weak)),
        "weak mode ignores comments"
    );
    assert_ne!(
        hashes("python", a, &opts(Mode::Mild)),
        hashes("python", b, &opts(Mode::Mild)),
        "mild mode keeps comments"
    );

    let tight = "x = 1\ny = x\n";
    let loose = "x  =   1\ny =  x\n";
    assert_eq!(
        hashes("python", tight, &opts(Mode::Mild)),
        hashes("python", loose, &opts(Mode::Mild)),
        "mild mode ignores whitespace"
    );
    assert_ne!(
        hashes("python", tight, &opts(Mode::Strict)),
        hashes("python", loose, &opts(Mode::Strict)),
        "strict mode keeps whitespace"
    );

    // The display path filters the same way.
    let kinds = |mode| -> Vec<TokenKind> {
        tokenize("python", a, mode)
            .into_iter()
            .map(|t| t.kind)
            .collect()
    };
    assert!(!kinds(Mode::Weak).contains(&TokenKind::Comment));
    assert!(kinds(Mode::Mild).contains(&TokenKind::Comment));
    assert!(!kinds(Mode::Mild).contains(&TokenKind::Whitespace));
    assert!(kinds(Mode::Strict).contains(&TokenKind::Whitespace));
}

#[test]
fn ignore_case_folds_identifiers() {
    let mut folded = opts(Mode::Mild);
    folded.ignore_case = true;
    assert_eq!(
        hashes("sql", "SELECT Name FROM Users\n", &folded),
        hashes("sql", "select name from users\n", &folded)
    );
    assert_ne!(
        hashes("sql", "SELECT Name FROM Users\n", &opts(Mode::Mild)),
        hashes("sql", "select name from users\n", &opts(Mode::Mild))
    );
}

// ── ignore patterns and markers ─────────────────────────────────────────

#[test]
fn an_ignore_pattern_drops_exactly_the_code_it_matches() {
    let source = "import * as _ from 'lodash';\nconst x = 1;\n";
    let regexes = [regex::Regex::new(r"import\s+\*\s+as\s+\w+\s+from\s+'[^']*';").unwrap()];
    let mut options = opts(Mode::Mild);
    options.ignore_ranges = code_ignore_ranges(source, &regexes);
    let t = texts(
        source,
        &tokenize_to_detection("javascript", source, &options),
    );
    assert_eq!(t, ["const", "x", "=", "1", ";"]);
}

/// `ignored_ranges` (used by similarity) must agree with detection, which
/// drops code between the markers inside a component's script.
#[test]
#[ignore = "known bug: ignored_ranges is empty for vue/svelte/astro because the display tokenizer filters the Ignore tokens it looks for"]
fn ignore_markers_in_a_component_script_cover_the_marked_code() {
    let vue = "<template><p>hi</p></template>\n<script>\nconst kept = 1;\n// jscpd:ignore-start\nconst hidden = 2;\n// jscpd:ignore-end\nconst after = 3;\n</script>\n";
    let ranges = ignored_ranges("vue", vue);
    let covered = |needle: &str| {
        let at = vue.find(needle).unwrap();
        ranges.iter().any(|[s, e]| *s <= at && at < *e)
    };
    assert!(covered("hidden"), "{ranges:?}");
    assert!(!covered("kept"), "{ranges:?}");
    assert!(!covered("after"), "{ranges:?}");

    let without = "<script>\nconst a = 1;\n</script>\n";
    assert!(ignored_ranges("vue", without).is_empty());
}

#[test]
fn ignore_patterns_that_do_not_compile_are_skipped() {
    let compiled = compile_ignore_patterns(&[
        r"console\.log\([^)]*\);".to_string(),
        "(unclosed".to_string(),
    ]);
    assert_eq!(compiled.len(), 1);
    let source = "console.log(1);\nreturn 2;\n";
    let mut options = opts(Mode::Mild);
    options.ignore_ranges = code_ignore_ranges(source, &compiled);
    let t = texts(
        source,
        &tokenize_to_detection("javascript", source, &options),
    );
    assert_eq!(t, ["return", "2", ";"]);
}

#[test]
fn ignore_markers_in_razor_markup_cover_the_marked_markup() {
    let razor = "<p>kept</p>\n<!-- jscpd:ignore-start -->\n<p>hidden</p>\n<!-- jscpd:ignore-end -->\n<p>after</p>\n";
    let ranges = ignored_ranges("razor", razor);
    let covered = |needle: &str| {
        let at = razor.find(needle).unwrap();
        ranges.iter().any(|[s, e]| *s <= at && at < *e)
    };
    assert!(covered("hidden"), "{ranges:?}");
    assert!(!covered("kept"), "{ranges:?}");
    assert!(!covered("after"), "{ranges:?}");
}

/// Detection and `ignored_ranges` both leave out every `hidden` in `source`
/// and keep every `kept`.
fn assert_marked_regions(format: &str, source: &str) {
    let t = texts(
        source,
        &tokenize_to_detection(format, source, &opts(Mode::Mild)),
    );
    assert!(
        !t.iter().any(|text| text.contains("hidden")),
        "{format} {source:?}: {t:?}"
    );
    let kept = t.iter().filter(|text| text.contains("kept")).count();
    assert_eq!(
        kept,
        source.matches("kept").count(),
        "{format} {source:?}: {t:?}"
    );

    let ranges = ignored_ranges(format, source);
    let covered = |at: usize| ranges.iter().any(|[s, e]| *s <= at && at < *e);
    for (at, _) in source.match_indices("hidden") {
        assert!(covered(at), "{format} {source:?}: {ranges:?}");
    }
    for (at, _) in source.match_indices("kept") {
        assert!(!covered(at), "{format} {source:?}: {ranges:?}");
    }
}

/// A `/*` or `//` inside a string, template, regular expression or JSX
/// text is no comment, so it cannot end a marked region early (issue
/// #1257).
#[test]
fn comment_openers_inside_literals_leave_marked_regions_whole() {
    let sources = [
        (
            "typescript",
            "/* jscpd:ignore-start */\nconst p: string = 'a /* b';\nhidden(1);\n/* jscpd:ignore-end */\nkept(2);\n",
        ),
        (
            "javascript",
            "// jscpd:ignore-start\nconst t = `a /* b`;\nhidden(1);\n// jscpd:ignore-end\nkept(2);\n",
        ),
        (
            "javascript",
            "// jscpd:ignore-start\nconst r = /a\\/*b/;\nhidden(1);\n/* jscpd:ignore-end */\nkept(2);\n",
        ),
        (
            "tsx",
            "// jscpd:ignore-start\nconst el = <p>src/* here</p>;\nhidden(1);\n// jscpd:ignore-end\nkept(2);\n",
        ),
        (
            "javascript",
            "const u = 'https://x'; /* jscpd:ignore-start */ hidden(0);\nhidden(1);\n/* jscpd:ignore-end */\nkept(2);\n",
        ),
    ];
    for (format, source) in sources {
        assert_marked_regions(format, source);
    }
}

/// The markers count wherever their text stands, as in every other
/// language, also where the parser sees no comment.
#[test]
fn markers_count_wherever_their_text_stands() {
    let sources = [
        // The CSS of a styled component: the template is one token.
        (
            "javascript",
            "const Button = styled.button`\n  /* jscpd:ignore-start */\n  hidden: 1px;\n  /* jscpd:ignore-end */\n`;\nkept(2);\n",
        ),
        // `jscpd --lsp` writes the markers on lines of their own, which
        // are JSX text among a component's children.
        (
            "jsx",
            "const List = () => (\n  <ul>\n    // jscpd:ignore-start\n    <li>hidden</li>\n    // jscpd:ignore-end\n  </ul>\n);\nkept(2);\n",
        ),
        // One marker of each pair is JSX text. Read from the comments
        // alone, Alpha's start would pair with Beta's end and hide kept().
        (
            "jsx",
            "function Alpha() {\n  // jscpd:ignore-start\n  hidden(1);\n  return (\n    <div>\n      // jscpd:ignore-end\n    </div>\n  );\n}\nkept(2);\nfunction Beta() {\n  return (\n    <div>\n      // jscpd:ignore-start\n      <p>hidden</p>\n    </div>\n  );\n  // jscpd:ignore-end\n}\n",
        ),
        (
            "javascript",
            "const s = '/* jscpd:ignore-start */';\nhidden(1);\n/* jscpd:ignore-end */\nkept(2);\n",
        ),
        // The lexer stops at the unterminated string, the end marker
        // after it still closes the region.
        (
            "javascript",
            "kept(0);\n// jscpd:ignore-start\nhidden(1);\n'unterminated\n// jscpd:ignore-end\n",
        ),
        // A second start inside a region changes nothing.
        (
            "javascript",
            "// jscpd:ignore-start\nhidden(1);\n// jscpd:ignore-start\nhidden(2);\n// jscpd:ignore-end\nkept(3);\n",
        ),
    ];
    for (format, source) in sources {
        assert_marked_regions(format, source);
    }
}

#[test]
fn markdown_code_blocks_are_the_fences_detection_splits_out() {
    let md = "---\ntitle: x\n---\n\n```ts\nconst a = 1;\n```\n\n```python\n```\n\n<!-- jscpd:ignore-start -->\n```js\nhidden();\n```\n<!-- jscpd:ignore-end -->\n\n```weird-lang\nstuff\n```\n";
    let blocks = code_blocks(md);
    let shown: Vec<(&str, &str)> = blocks
        .iter()
        .map(|(format, range)| (format.as_str(), &md[range.clone()]))
        .collect();
    // Front matter, the empty fence and the ignored fence are left out; an
    // unknown language is plain text.
    assert_eq!(shown, [("typescript", "const a = 1;"), ("text", "stuff")]);
}

#[test]
fn component_script_blocks_are_the_code_a_function_can_live_in() {
    let vue = "<template><p>{{ a }}</p></template>\n<script>\nexport default {};\n</script>\n<script setup lang=\"ts\">\nconst a = 1;\n</script>\n<style>p { color: red }</style>\n";
    let blocks: Vec<(String, &str)> = script_blocks(vue, "vue")
        .into_iter()
        .map(|(format, range)| (format, &vue[range]))
        .collect();
    assert_eq!(
        blocks,
        [
            ("javascript".to_string(), "\nexport default {};\n"),
            ("typescript".to_string(), "\nconst a = 1;\n"),
        ]
    );

    let astro =
        "---\nconst title = 'x';\n---\n<h1>{title}</h1>\n<script>\nconsole.log(1);\n</script>\n";
    let formats: Vec<String> = script_blocks(astro, "astro")
        .into_iter()
        .map(|(format, _)| format)
        .collect();
    assert_eq!(formats, ["typescript", "javascript"]);
    assert!(script_blocks("<p>no code</p>", "svelte").is_empty());
}

#[test]
fn markdown_embedded_blocks_are_the_front_matter_and_the_fences() {
    let md = "---\ntitle: x\n---\n\n```ts\nconst a = 1;\n```\n\n<!-- jscpd:ignore-start -->\n```js\nhidden();\n```\n<!-- jscpd:ignore-end -->\n";
    let blocks: Vec<(String, &str)> = markdown::embedded_blocks(md)
        .into_iter()
        .map(|(format, range)| (format, &md[range]))
        .collect();
    assert_eq!(
        blocks,
        [
            ("yaml".to_string(), "title: x"),
            ("typescript".to_string(), "const a = 1;"),
        ]
    );
    // Detection gives the front matter to the yaml source of the file too.
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    assert!(maps.iter().any(|map| map.format == "yaml"));
}

#[test]
fn component_embedded_blocks_are_every_block_detection_splits_out() {
    let vue = "<template><p>{{ a }}</p></template>\n<script setup lang=\"ts\">\nconst a = 1;\n</script>\n<style lang=\"scss\">p { color: red }</style>\n";
    let blocks: Vec<(String, &str)> = sfc::embedded_blocks(vue, "vue")
        .into_iter()
        .map(|(format, range)| (format, &vue[range]))
        .collect();
    assert_eq!(
        blocks,
        [
            ("html".to_string(), "<p>{{ a }}</p>"),
            ("typescript".to_string(), "\nconst a = 1;\n"),
            ("scss".to_string(), "p { color: red }"),
        ]
    );
    let maps = tokenize_to_detection_maps("vue", vue, &opts(Mode::Mild));
    for (format, _) in &blocks {
        assert!(maps.iter().any(|map| &map.format == format), "{format}");
    }
}

// ── multi-format files ─────────────────────────────────────────────────

#[test]
fn a_single_format_file_is_one_map_of_its_own_format() {
    let maps = tokenize_to_detection_maps("python", "x = 1\n", &opts(Mode::Mild));
    assert_eq!(formats_of(&maps), ["python"]);
    let razor = "<p>@Model.Name</p>\n@{\n    var total = 1;\n}\n";
    let maps = tokenize_to_detection_maps("razor", razor, &opts(Mode::Mild));
    assert!(
        formats_of(&maps).contains(&"csharp"),
        "{:?}",
        formats_of(&maps)
    );
}

#[test]
fn a_component_script_language_decides_the_stream_it_joins() {
    let detect = |attrs: &str| {
        let vue =
            format!("<template><p>hi</p></template>\n<script{attrs}>\nconst a = 1;\n</script>\n");
        let maps = tokenize_to_detection_maps("vue", &vue, &opts(Mode::Mild));
        formats_of(&maps)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert!(detect(" lang=\"ts\"").contains(&"typescript".to_string()));
    assert!(detect(" lang='tsx'").contains(&"tsx".to_string()));
    assert!(detect(" lang=\"jsx\"").contains(&"jsx".to_string()));
    // An unknown or unquoted language reads as JavaScript.
    assert!(detect(" lang=\"klingon\"").contains(&"javascript".to_string()));
    assert!(detect(" lang=ts").contains(&"javascript".to_string()));
    // A language that names a known format by name.
    assert!(detect(" lang=\"python\"").contains(&"python".to_string()));
}

#[test]
fn a_fenced_component_in_markdown_joins_the_component_stream() {
    let md =
        "Docs.\n\n```svelte\n<script>\nlet count = 0;\n</script>\n<button>{count}</button>\n```\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Mild));
    let svelte = map(&maps, "svelte").expect("a svelte map");
    assert!(texts(md, &svelte.tokens).contains(&"count"));
}

// ── TypeScript stripped for cross-format detection ─────────────────────

fn stripped(source: &str) -> Vec<u64> {
    let mut options = opts(Mode::Mild);
    options.strip_types_formats = ["typescript".to_string()].into_iter().collect();
    hashes("typescript", source, &options)
}

#[track_caller]
fn assert_twins(ts: &str, js: &str) {
    let js_hashes = hashes("javascript", js, &opts(Mode::Mild));
    assert!(!js_hashes.is_empty());
    assert_eq!(stripped(ts), js_hashes, "\nTS: {ts}\nJS: {js}");
}

#[test]
fn type_only_imports_and_exports_vanish_from_stripped_typescript() {
    assert_twins(
        "import { a, type B } from './m';\nuse(a);\n",
        "import { a, } from './m';\nuse(a);\n",
    );
    assert_twins("export type { T };\nrun();\n", "run();\n");
    assert_twins(
        "const a = 1;\nexport { a, type T };\n",
        "const a = 1;\nexport { a, };\n",
    );
    assert_twins(
        "export default interface Shape { x: number }\nrun();\n",
        "run();\n",
    );
    assert_twins(
        "export default function f(a: number): number { return a; }\n",
        "export default function f(a) { return a; }\n",
    );
}

#[test]
fn ambient_declarations_vanish_from_stripped_typescript() {
    assert_twins(
        "declare enum E { A }\ndeclare module 'm' { const x: number; }\ndeclare namespace N { const y: number; }\ndeclare const z: number;\ndeclare class K {}\nrun();\n",
        "run();\n",
    );
    // A namespace with a body is runtime code: it stays, and only its
    // types go.
    assert_eq!(
        stripped("namespace N { export const a: number = 1; }\n"),
        stripped("namespace N { export const a = 1; }\n"),
    );
    assert_ne!(
        stripped("namespace N { export const a = 1; }\n"),
        stripped("export const a = 1;\n"),
    );
}

#[test]
fn definite_and_optional_markers_vanish_from_stripped_typescript() {
    assert_twins("let x!: number;\nx = 1;\n", "let x;\nx = 1;\n");
    assert_twins(
        "class A {\n  name?: string;\n  id!: number;\n  ready?(): void {}\n  count = 0;\n}\n",
        "class A {\n  name;\n  id;\n  ready() {}\n  count = 0;\n}\n",
    );
    assert_twins(
        "abstract class Base {\n  abstract run(): void;\n  go() {}\n}\n",
        "class Base {\n  go() {}\n}\n",
    );
    assert_twins(
        "function f(this: Window) { return 1; }\n",
        "function f() { return 1; }\n",
    );
    assert_twins("const v = <number>value;\n", "const v = value;\n");
}

/// A type-only re-export names nothing at runtime, like the local form
/// `export type { T };` that is stripped.
#[test]
#[ignore = "known bug: `export type { T } from '...'` survives TypeScript stripping"]
fn a_type_only_re_export_vanishes_from_stripped_typescript() {
    assert_twins("export type { T } from './t';\nrun();\n", "run();\n");
}

// ── formats ─────────────────────────────────────────────────────────────

#[test]
fn a_shebang_names_the_interpreter_format() {
    for (line, want) in [
        ("#!/usr/bin/env python3", Some("python")),
        ("#!/usr/bin/env node", Some("javascript")),
        ("#!/usr/bin/ruby", Some("ruby")),
        ("#!/bin/bash", Some("bash")),
        ("#!/bin/sh", Some("bash")),
        ("#!/usr/bin/perl -w", Some("perl")),
        ("#!/usr/bin/php", Some("php")),
        ("#!/usr/bin/env fortune", None),
        ("plain first line", None),
    ] {
        assert_eq!(get_format_by_shebang(line), want, "{line}");
    }
    assert_eq!(resolve_format("ZSH"), Some("bash"));
}

// ── known bugs ──────────────────────────────────────────────────────────

/// A bare fence holds shell or plain text, not C: `/*` in a path glob must
/// not open a comment that swallows the rest of the block.
#[test]
#[ignore = "known bug: a bare fence falls back to the unregistered `text` format, tokenized with C comments"]
fn a_glob_in_a_bare_fence_does_not_swallow_the_block_as_a_comment() {
    let md = "Build:\n\n```\n$ cp src/*.js build/\n$ echo finished\n```\n";
    let maps = tokenize_to_detection_maps("markdown", md, &opts(Mode::Weak));
    let code: Vec<&str> = maps
        .iter()
        .filter(|m| m.format != "markdown")
        .flat_map(|m| texts(md, &m.tokens))
        .collect();
    assert!(code.contains(&"echo"), "{code:?}");
    assert!(code.contains(&"finished"), "{code:?}");

    let display = tokenize("markdown", md, Mode::Weak);
    assert!(display.iter().any(|t| t.value == "finished"), "{display:?}");
}
