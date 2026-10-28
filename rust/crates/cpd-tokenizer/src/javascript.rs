// cpd-tokenizer: OXC-based tokenizer for JavaScript/TypeScript/JSX/TSX.
// Dispatched for formats: javascript, typescript, jsx, tsx.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    JSXClosingElement, JSXClosingFragment, JSXElement, JSXExpressionContainer, JSXFragment,
    JSXOpeningElement, JSXOpeningFragment, JSXSpreadChild,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{Kind, Parser, config::TokensParserConfig};
use oxc_span::{SourceType, Span};

use cpd_core::models::{Token, TokenKind};

use crate::line_index::LineIndex;

// ── fallback tokenizer ────────────────────────────────────────────────────────

mod fallback {
    use super::{LineIndex, find_ignore_ranges, in_ignore};
    use cpd_core::models::{Token, TokenKind};

    /// Simple word-split fallback tokenizer. Never panics.
    pub fn tokenize(source: &str, _format: &str) -> Vec<Token> {
        let ignore_ranges = find_ignore_ranges(source);
        let bytes = source.as_bytes();
        let line_index = LineIndex::new(bytes);
        let mut tokens = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let ch = match source[i..].chars().next() {
                Some(c) => c,
                None => break,
            };
            if ch.is_whitespace() {
                i += ch.len_utf8();
                continue;
            }
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                let start = i;
                while i < bytes.len() {
                    let c = source[i..].chars().next().unwrap_or('\0');
                    if c.is_alphanumeric() || c == '_' || c == '$' {
                        i += c.len_utf8();
                    } else {
                        break;
                    }
                }
                let kind = if in_ignore(start, i, &ignore_ranges) {
                    TokenKind::Ignore
                } else {
                    TokenKind::Other
                };
                tokens.push(Token {
                    kind,
                    value: source[start..i].to_string(),
                    start: line_index.location(start),
                    end: line_index.location(i),
                });
            } else {
                let start = i;
                i += ch.len_utf8();
                let kind = if in_ignore(start, i, &ignore_ranges) {
                    TokenKind::Ignore
                } else {
                    TokenKind::Other
                };
                tokens.push(Token {
                    kind,
                    value: ch.to_string(),
                    start: line_index.location(start),
                    end: line_index.location(i),
                });
            }
        }
        tokens
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// The byte ranges between `jscpd:ignore-start` and `jscpd:ignore-end`,
/// whose tokens [`tokenize_js`] marks as ignored. Each runs from the end of
/// a start marker to the next end marker: a second start inside it changes
/// nothing, and a start with no end after it marks nothing. The markers are
/// found by their text, as the generic tokenizer finds them, so a `/*` or
/// `//` in a string, template or regular expression has no say in where a
/// range ends (issue #1257), and a marker inside a template literal or JSX
/// text still counts.
pub(crate) fn find_ignore_ranges(source: &str) -> Vec<[usize; 2]> {
    const START: &str = "jscpd:ignore-start";
    const END: &str = "jscpd:ignore-end";
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(at) = source[from..].find(START) {
        let start = from + at + START.len();
        let Some(length) = source[start..].find(END) else {
            break;
        };
        ranges.push([start, start + length]);
        from = start + length + END.len();
    }
    ranges
}

fn in_ignore(offset: usize, end: usize, ranges: &[[usize; 2]]) -> bool {
    ranges.iter().any(|[rs, re]| offset < *re && end > *rs)
}

const fn map_kind(kind: Kind) -> TokenKind {
    if matches!(kind, Kind::Ident) {
        return TokenKind::Identifier;
    }
    if kind.is_any_keyword() {
        return TokenKind::Keyword;
    }
    if kind.is_literal() {
        return TokenKind::Literal;
    }
    if kind.is_assignment_operator() {
        return TokenKind::Operator;
    }
    if kind.is_binary_operator()
        || kind.is_logical_operator()
        || kind.is_unary_operator()
        || kind.is_update_operator()
    {
        return TokenKind::Operator;
    }
    match kind {
        Kind::Arrow => TokenKind::Operator,
        Kind::Semicolon
        | Kind::Comma
        | Kind::Dot
        | Kind::Dot3
        | Kind::Colon
        | Kind::LParen
        | Kind::RParen
        | Kind::LCurly
        | Kind::RCurly
        | Kind::LBrack
        | Kind::RBrack
        | Kind::At => TokenKind::Punctuation,
        Kind::QuestionDot => TokenKind::Punctuation,
        _ => TokenKind::Other,
    }
}

/// The oxc source type a jscpd format parses as.
pub fn source_type_for_format(format: &str) -> SourceType {
    let filename = match format {
        "typescript" => "input.ts",
        "tsx" => "input.tsx",
        _ => "input.jsx", // javascript + jsx both use jsx
    };
    SourceType::from_path(Path::new(filename)).unwrap_or_default()
}

// ── public API ───────────────────────────────────────────────────────────────

/// Tokenize JS/TS/JSX/TSX source. Never panics.
pub fn tokenize_js(source: &str, format: &str) -> Vec<Token> {
    tokenize_js_impl(source, format, false)
}

/// Tokenize TS/TSX source with erasable TypeScript-only syntax stripped from
/// the token stream (cross-format detection). Token locations still reference
/// the original source. Never panics.
///
/// Sources the parser gives up on fall back to the word-split tokenizer
/// WITHOUT stripping — they simply won't cross-match JavaScript files.
/// Recoverable errors keep the oxc token stream (and best-effort stripping
/// from the partial AST).
pub fn tokenize_js_stripped(source: &str, format: &str) -> Vec<Token> {
    tokenize_js_impl(source, format, true)
}

fn tokenize_js_impl(source: &str, format: &str, strip_types: bool) -> Vec<Token> {
    if source.is_empty() {
        return Vec::new();
    }

    match catch_unwind(AssertUnwindSafe(|| {
        parse_with_oxc(source, format, strip_types)
    })) {
        Ok(Some(tokens)) => tokens,
        Ok(None) => {
            log::debug!("cpd-tokenizer: OXC parse errors in {format} source, using fallback");
            fallback::tokenize(source, format)
        }
        Err(_) => {
            log::debug!("cpd-tokenizer: OXC panicked on {format} source, using fallback");
            fallback::tokenize(source, format)
        }
    }
}

fn parse_with_oxc(source: &str, format: &str, strip_types: bool) -> Option<Vec<Token>> {
    let allocator = Allocator::new();
    let source_type = source_type_for_format(format);

    let parser_return = Parser::new(&allocator, source, source_type)
        .with_config(TokensParserConfig)
        .parse();

    // Parse diagnostics (redeclarations, recoverable syntax errors) do not
    // touch the token stream: tokens come from the lexer. Only a parser that
    // gave up, or one that produced no tokens, sends the file to the
    // word-split fallback. Anything else would tokenize a file with one
    // error differently from every other file, so it could never match
    // them (issue #1023).
    if parser_return.fatal_error || parser_return.tokens.is_empty() {
        return None;
    }

    let erasable = if strip_types {
        crate::ts_strip::collect_erasable_spans(&parser_return.program, source)
    } else {
        crate::ts_strip::ErasableSpans::default()
    };

    let ignore_ranges = find_ignore_ranges(source);
    let bytes = source.as_bytes();
    // Build LineIndex once — O(n) — then all location calls are O(log n).
    let line_index = LineIndex::new(bytes);

    let tokens = parser_return
        .tokens
        .into_iter()
        .filter_map(|token| {
            let start = (token.start() as usize).min(source.len());
            let end = (token.end() as usize).min(source.len());
            if start >= end {
                return None;
            }
            let kind = token.kind();
            if matches!(kind, Kind::Eof | Kind::Undetermined | Kind::Skip) {
                return None;
            }
            let value = &source[start..end];
            if !erasable.is_empty()
                && (erasable.intersects_range(start as u32, end as u32)
                    || erasable.is_modifier_token(start as u32, end as u32, value))
            {
                return None;
            }
            let token_kind = if in_ignore(start, end, &ignore_ranges) {
                TokenKind::Ignore
            } else {
                map_kind(kind)
            };

            Some(Token {
                kind: token_kind,
                value: value.to_string(),
                start: line_index.location(start),
                end: line_index.location(end),
            })
        })
        .collect::<Vec<Token>>();

    Some(tokens)
}

// ── ignore markers in JSX ─────────────────────────────────────────────────────

/// Where a `jscpd:ignore` marker goes in a JavaScript source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerSpot {
    /// The byte the marker goes at.
    pub offset: usize,
    /// The marker stands among the children of a JSX element or fragment,
    /// where `//` and `/* */` are text on the page, so it has to be
    /// `{/* … */}`.
    pub jsx_child: bool,
}

/// Where the markers around the code from byte `start` to byte `end` of
/// `source` go. A marker stays at its byte unless the byte is inside a JSX
/// tag: then the start marker goes before the tag and the end marker after
/// it, as TypeScript reads no comment between the `<` and the `/` of
/// `</ul>`. `None` when the parser gives up on the source.
pub fn ignore_marker_spots(
    source: &str,
    format: &str,
    start: usize,
    end: usize,
) -> Option<[MarkerSpot; 2]> {
    let asked = [u32::try_from(start).ok()?, u32::try_from(end).ok()?];
    catch_unwind(AssertUnwindSafe(|| {
        let allocator = Allocator::new();
        let parsed = Parser::new(&allocator, source, source_type_for_format(format)).parse();
        if parsed.fatal_error {
            return None;
        }
        let places_of = |offsets: [u32; 2]| {
            let mut places = Places {
                offsets,
                places: [Place::Code; 2],
            };
            places.visit_program(&parsed.program);
            places.places
        };
        let [first, last] = places_of(asked);
        let offsets = [
            match first {
                Place::Tag(tag) => tag.start,
                _ => asked[0],
            },
            match last {
                Place::Tag(tag) => tag.end,
                _ => asked[1],
            },
        ];
        // A marker out of a tag stands where it lands: after `<li>` it is
        // among the children of the `li`, before `</ul>` among those of the
        // `ul`.
        let places = match offsets == asked {
            true => [first, last],
            false => places_of(offsets),
        };
        Some([0, 1].map(|n| MarkerSpot {
            offset: offsets[n] as usize,
            jsx_child: places[n] == Place::Children,
        }))
    }))
    .ok()
    .flatten()
}

/// What a byte of a JSX source stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Code,
    /// Among the children of an element or fragment.
    Children,
    /// Inside the tag at this span.
    Tag(Span),
}

/// The [`Place`] of two bytes. The visitor meets the nodes that hold a byte
/// outermost first, so the last place it marks is the innermost one.
struct Places {
    offsets: [u32; 2],
    places: [Place; 2],
}

impl Places {
    /// The bytes from `from` to `to`, both included, the end of an opening
    /// tag and the start of its closing tag, are among children.
    fn children(&mut self, from: u32, to: u32) {
        for (offset, place) in self.offsets.iter().zip(&mut self.places) {
            if from <= *offset && *offset <= to {
                *place = Place::Children;
            }
        }
    }

    /// The bytes strictly inside `span`, a tag or a `{…}` of code.
    fn inside(&mut self, span: Span, tag: bool) {
        for (offset, place) in self.offsets.iter().zip(&mut self.places) {
            if span.start < *offset && *offset < span.end {
                *place = match tag {
                    true => Place::Tag(span),
                    false => Place::Code,
                };
            }
        }
    }
}

impl<'a> Visit<'a> for Places {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        if let Some(closing) = &it.closing_element {
            self.children(it.opening_element.span.end, closing.span.start);
        }
        walk::walk_jsx_element(self, it);
    }

    fn visit_jsx_fragment(&mut self, it: &JSXFragment<'a>) {
        self.children(it.opening_fragment.span.end, it.closing_fragment.span.start);
        walk::walk_jsx_fragment(self, it);
    }

    fn visit_jsx_opening_element(&mut self, it: &JSXOpeningElement<'a>) {
        self.inside(it.span, true);
        walk::walk_jsx_opening_element(self, it);
    }

    fn visit_jsx_closing_element(&mut self, it: &JSXClosingElement<'a>) {
        self.inside(it.span, true);
    }

    fn visit_jsx_opening_fragment(&mut self, it: &JSXOpeningFragment) {
        self.inside(it.span, true);
    }

    fn visit_jsx_closing_fragment(&mut self, it: &JSXClosingFragment) {
        self.inside(it.span, true);
    }

    fn visit_jsx_expression_container(&mut self, it: &JSXExpressionContainer<'a>) {
        self.inside(it.span, false);
        walk::walk_jsx_expression_container(self, it);
    }

    fn visit_jsx_spread_child(&mut self, it: &JSXSpreadChild<'a>) {
        self.inside(it.span, false);
        walk::walk_jsx_spread_child(self, it);
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #1023: a redeclaration is a parse diagnostic, not a lexing
    /// failure, so the file must keep the oxc token stream and stay
    /// comparable with files that parse cleanly.
    #[test]
    fn parse_diagnostics_keep_the_oxc_token_stream() {
        let clean = "export function save(row, db) {\n  db.put(row.id, row);\n  return row;\n}\n";
        let twice = format!("{clean}\n{clean}");
        let a = tokenize_js(clean, "javascript");
        let b = tokenize_js(&twice, "javascript");
        assert!(a.len() > 10);
        assert_eq!(b.len(), a.len() * 2, "both copies lexed by oxc");
        assert_eq!(
            a.iter().map(|t| (&t.kind, &t.value)).collect::<Vec<_>>(),
            b[..a.len()]
                .iter()
                .map(|t| (&t.kind, &t.value))
                .collect::<Vec<_>>(),
            "the first copy must tokenize exactly like the clean file"
        );
        assert_eq!(
            b[0].kind,
            TokenKind::Keyword,
            "`export` classified by oxc, not word-split"
        );
    }

    #[test]
    fn empty_and_unlexable_sources_still_fall_back_safely() {
        assert!(tokenize_js("", "javascript").is_empty());
        // A stray token soup parses with errors but lexes fine: still tokens.
        assert!(!tokenize_js("function (", "javascript").is_empty());
    }

    #[test]
    fn valid_js_produces_tokens() {
        let tokens = tokenize_js("function hello() { return 42; }", "javascript");
        assert!(!tokens.is_empty(), "valid JS must produce tokens");
    }

    #[test]
    fn typescript_produces_tokens() {
        let tokens = tokenize_js("const x: number = 5;", "typescript");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn malformed_js_does_not_panic() {
        let result = std::panic::catch_unwind(|| tokenize_js("let x = {{{", "javascript"));
        assert!(result.is_ok(), "malformed JS must not panic");
    }

    #[test]
    fn empty_source_returns_empty() {
        let tokens = tokenize_js("", "javascript");
        drop(tokens);
    }

    #[test]
    fn ignore_region_tokens_marked_as_ignore() {
        let source = r#"
const a = 1;
// jscpd:ignore-start
const b = 2;
// jscpd:ignore-end
const c = 3;
"#;
        let tokens = tokenize_js(source, "javascript");
        let has_ignore = tokens
            .iter()
            .any(|t| t.kind == cpd_core::models::TokenKind::Ignore);
        assert!(has_ignore, "tokens in ignore region must be marked Ignore");
    }

    /// A source oxc gives up on goes to the word-split fallback, which finds
    /// the markers the same way: a `/*` in a string does not end the region
    /// there either.
    #[test]
    fn unparsable_sources_keep_their_marked_regions() {
        // The type annotation in a .js file makes the parser give up.
        let source = "/* jscpd:ignore-start */\nconst p: string = 'a /* b';\nhidden(1);\n/* jscpd:ignore-end */\nafter(2);\n";
        let tokens = tokenize_js(source, "javascript");
        assert_eq!(tokens[0].kind, TokenKind::Other, "word-split fallback");
        let kept: Vec<&str> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Ignore)
            .map(|t| t.value.as_str())
            .collect();
        assert!(!kept.contains(&"hidden"), "{kept:?}");
        assert!(kept.contains(&"after"), "{kept:?}");
    }

    /// The markers [`ignore_marker_spots`] places around the code from `‹`
    /// to `›` of `marked`, drawn into the source: `/*S*/` and `/*E*/` in
    /// code, `{/*S*/}` and `{/*E*/}` among JSX children.
    fn placed(format: &str, marked: &str) -> String {
        let start = marked.find('‹').expect("a start");
        let source = marked.replacen('‹', "", 1);
        let end = source.find('›').expect("an end");
        let mut source = source.replacen('›', "", 1);
        let spots = ignore_marker_spots(&source, format, start, end).expect("a parsed source");
        for (spot, word) in spots.into_iter().zip(["S", "E"]).rev() {
            let marker = match spot.jsx_child {
                true => format!("{{/*{word}*/}}"),
                false => format!("/*{word}*/"),
            };
            source.insert_str(spot.offset, &marker);
        }
        source
    }

    /// Among the children of a JSX element a comment is text on the page,
    /// and TypeScript reads none between the `<` and `/` of a closing tag.
    #[test]
    fn markers_in_jsx_go_among_children_in_braces_and_never_inside_a_tag() {
        for (format, marked, expected) in [
            // Code, also the code around an element: the bytes asked for.
            (
                "javascript",
                "f();\n‹g();\nh();›\n",
                "f();\n/*S*/g();\nh();/*E*/\n",
            ),
            (
                "jsx",
                "const a = (\n  ‹<ul>\n    <li>a</li>\n  </ul>›\n);\n",
                "const a = (\n  /*S*/<ul>\n    <li>a</li>\n  </ul>/*E*/\n);\n",
            ),
            // Among children, on lines of their own or beside text.
            (
                "jsx",
                "const a = <ul>\n  ‹<li>a</li>\n  <li>b</li>›\n</ul>;\n",
                "const a = <ul>\n  {/*S*/}<li>a</li>\n  <li>b</li>{/*E*/}\n</ul>;\n",
            ),
            (
                "tsx",
                "const a = <p>Hi ‹<b>x</b> and <i>y</i>›!</p>;\n",
                "const a = <p>Hi {/*S*/}<b>x</b> and <i>y</i>{/*E*/}!</p>;\n",
            ),
            (
                "javascript",
                "const a = <>\n  ‹<b />\n  <i />›\n</>;\n",
                "const a = <>\n  {/*S*/}<b />\n  <i />{/*E*/}\n</>;\n",
            ),
            // Inside a tag: before the tag at the start, after it at the end.
            (
                "jsx",
                "const a = (\n  <ul className=\"a\"‹>\n    <li>a</li><›/ul>\n);\n",
                "const a = (\n  /*S*/<ul className=\"a\">\n    <li>a</li></ul>/*E*/\n);\n",
            ),
            (
                "jsx",
                "const a = <nav>\n  <ul id=\"a\"‹>\n    <li>a</li>\n  </›ul>\n</nav>;\n",
                "const a = <nav>\n  {/*S*/}<ul id=\"a\">\n    <li>a</li>\n  </ul>{/*E*/}\n</nav>;\n",
            ),
            // Out of a tag, a marker stands where it lands: after an opening
            // tag among the children of its element, before a closing tag too.
            (
                "jsx",
                "const a = (\n  ‹f(),\n  <ul›>\n    <li>a</li>\n  </ul>\n);\n",
                "const a = (\n  /*S*/f(),\n  <ul>{/*E*/}\n    <li>a</li>\n  </ul>\n);\n",
            ),
            (
                "jsx",
                "const a = (\n  <ul>\n    <li>a</li>\n  </ul‹>,\n  f()›\n);\n",
                "const a = (\n  <ul>\n    <li>a</li>\n  {/*S*/}</ul>,\n  f()/*E*/\n);\n",
            ),
            (
                "tsx",
                "const a = (\n  ‹f(),\n  <›>\n    <b />\n  </>\n);\n",
                "const a = (\n  /*S*/f(),\n  <>{/*E*/}\n    <b />\n  </>\n);\n",
            ),
            // A `{…}` holds code, in a tag or among children, and the JSX in
            // that code has children of its own.
            (
                "jsx",
                "const a = <b onClick={() => {\n  ‹f();\n  g();›\n}} />;\n",
                "const a = <b onClick={() => {\n  /*S*/f();\n  g();/*E*/\n}} />;\n",
            ),
            (
                "jsx",
                "const a = <ul>{items.map((i) => ‹f(i)›)}</ul>;\n",
                "const a = <ul>{items.map((i) => /*S*/f(i)/*E*/)}</ul>;\n",
            ),
            (
                "jsx",
                "const a = <ul>{items.map((i) => <li>‹{i}›</li>)}</ul>;\n",
                "const a = <ul>{items.map((i) => <li>{/*S*/}{i}{/*E*/}</li>)}</ul>;\n",
            ),
        ] {
            assert_eq!(placed(format, marked), expected, "{format} {marked:?}");
        }
        assert_eq!(ignore_marker_spots("const a = <ul>", "jsx", 0, 5), None);
    }

    #[test]
    fn jsx_produces_tokens() {
        let tokens = tokenize_js("const el = <div>hello</div>;", "jsx");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn tsx_with_type_annotation() {
        let tokens = tokenize_js("const fn = (x: React.FC): void => {};", "tsx");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn multiline_location_uses_binary_search() {
        let source = "const a = 1;\nconst b = 2;\nconst c = 3;";
        let tokens = tokenize_js(source, "javascript");
        // "b" is on line 2
        let b_token = tokens.iter().find(|t| t.value == "b");
        assert!(b_token.is_some(), "must find token b");
        assert_eq!(b_token.unwrap().start.line, 2, "b must be on line 2");
    }
}
