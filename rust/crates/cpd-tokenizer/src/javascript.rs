// cpd-tokenizer: OXC-based tokenizer for JavaScript/TypeScript/JSX/TSX.
// Dispatched for formats: javascript, typescript, jsx, tsx.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_parser::{Kind, Parser, ParserReturn, config::TokensParserConfig};
use oxc_span::SourceType;

use cpd_core::models::{Token, TokenKind};

use crate::line_index::LineIndex;

// ── fallback tokenizer ────────────────────────────────────────────────────────

mod fallback {
    use super::{LineIndex, in_ignore, scan_ignore_ranges};
    use cpd_core::models::{Token, TokenKind};

    /// Simple word-split fallback tokenizer. Never panics.
    pub fn tokenize(source: &str, _format: &str) -> Vec<Token> {
        let ignore_ranges = scan_ignore_ranges(source);
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

/// The byte ranges between `jscpd:ignore-start` and `jscpd:ignore-end`
/// comments, whose tokens [`tokenize_js`] marks as ignored. The comments
/// are the ones oxc lexes, so a `/*` or `//` inside a string, template or
/// regular expression is not one (issue #1257). A source the parser gives
/// up on is scanned byte by byte, as the word-split fallback reads it.
pub(crate) fn find_ignore_ranges(source: &str, format: &str) -> Vec<[usize; 2]> {
    let lexed = catch_unwind(AssertUnwindSafe(|| {
        let allocator = Allocator::new();
        parse(&allocator, source, format).map(|parsed| comment_ignore_ranges(source, &parsed))
    }));
    match lexed {
        Ok(Some(ranges)) => ranges,
        _ => scan_ignore_ranges(source),
    }
}

/// The ignored ranges from the comments oxc collected.
fn comment_ignore_ranges(source: &str, parsed: &ParserReturn<'_>) -> Vec<[usize; 2]> {
    let spans = parsed.program.comments.iter().map(|comment| {
        let span = comment.span;
        [span.start as usize, span.end as usize]
    });
    marker_ranges(source, spans)
}

/// The ignored ranges with comments found byte by byte: a `/*` or `//`
/// opens one wherever it stands, inside a string literal too.
fn scan_ignore_ranges(source: &str) -> Vec<[usize; 2]> {
    let mut comments = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' {
            let end = if bytes[i + 1] == b'/' {
                bytes[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|p| i + p)
                    .unwrap_or(bytes.len())
            } else if bytes[i + 1] == b'*' {
                bytes[i..]
                    .windows(2)
                    .position(|w| w == b"*/")
                    .map(|p| i + p + 2)
                    .unwrap_or(bytes.len())
            } else {
                i += 1;
                continue;
            };
            comments.push([i, end]);
            i = end;
            continue;
        }
        i += 1;
    }
    marker_ranges(source, comments)
}

/// The byte ranges from the end of each comment with `jscpd:ignore-start`
/// to the start of the next one with `jscpd:ignore-end`, given the spans of
/// the comments of `source` in order.
fn marker_ranges(source: &str, comments: impl IntoIterator<Item = [usize; 2]>) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    for [from, to] in comments {
        let Some(comment_text) = source.get(from..to) else {
            continue;
        };
        if comment_text.contains("jscpd:ignore-start") {
            start = Some(to);
        } else if comment_text.contains("jscpd:ignore-end")
            && let Some(s) = start.take()
        {
            ranges.push([s, from]);
        }
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

/// oxc's parse of `source` with its token stream, or `None` when the
/// word-split fallback tokenizes it instead.
fn parse<'a>(allocator: &'a Allocator, source: &'a str, format: &str) -> Option<ParserReturn<'a>> {
    let parser_return = Parser::new(allocator, source, source_type_for_format(format))
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
    Some(parser_return)
}

fn parse_with_oxc(source: &str, format: &str, strip_types: bool) -> Option<Vec<Token>> {
    let allocator = Allocator::new();
    let parser_return = parse(&allocator, source, format)?;

    let erasable = if strip_types {
        crate::ts_strip::collect_erasable_spans(&parser_return.program, source)
    } else {
        crate::ts_strip::ErasableSpans::default()
    };

    let ignore_ranges = if source.contains("jscpd:ignore") {
        comment_ignore_ranges(source, &parser_return)
    } else {
        Vec::new()
    };
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

    /// The values of the tokens of `source` that are not marked Ignore.
    fn kept(source: &str, format: &str) -> Vec<String> {
        tokenize_js(source, format)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Ignore)
            .map(|t| t.value)
            .collect()
    }

    /// Issue #1257: a `/*` inside a string literal is not a comment, so it
    /// must not swallow the code up to the `*/` of the end marker.
    #[test]
    fn block_comment_opener_in_a_string_does_not_end_the_region() {
        let source = "/* jscpd:ignore-start */\nconst p = 'a /* b';\nfunction f() {\n  return compute(first, second);\n}\n/* jscpd:ignore-end */\nconst after = 1;\n";
        assert_eq!(
            kept(source, "typescript"),
            ["const", "after", "=", "1", ";"]
        );
        let ranges = find_ignore_ranges(source, "typescript");
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        assert!(source[ranges[0][0]..ranges[0][1]].contains("return compute"));
    }

    /// Issue #1257: a `//` inside a string is not a comment, so a region
    /// opened later on the same line starts at its marker, not at the end
    /// of the line.
    #[test]
    fn line_comment_opener_in_a_string_does_not_hide_a_marker() {
        let source = "const url = \"https://example.com\"; /* jscpd:ignore-start */ const hidden = 2;\n// jscpd:ignore-end\nconst after = 3;\n";
        let kept = kept(source, "javascript");
        assert!(!kept.contains(&"hidden".to_string()), "{kept:?}");
        assert!(kept.contains(&"url".to_string()), "{kept:?}");
        assert!(kept.contains(&"after".to_string()), "{kept:?}");
    }

    #[test]
    fn comment_openers_in_templates_and_regexes_are_not_comments() {
        let template = "// jscpd:ignore-start\nconst t = `a /* ${b}`;\nconst hidden = 2;\n/* jscpd:ignore-end */\nconst after = 3;\n";
        assert_eq!(
            kept(template, "javascript"),
            ["const", "after", "=", "3", ";"]
        );
        let regex = "// jscpd:ignore-start\nconst r = /a\\/*b/g;\nconst hidden = 2;\n/* jscpd:ignore-end */\nconst after = 3;\n";
        assert_eq!(kept(regex, "javascript"), ["const", "after", "=", "3", ";"]);
    }

    /// A marker written inside a string is text, not a marker.
    #[test]
    fn markers_inside_strings_are_not_markers() {
        let source =
            "const s = '/* jscpd:ignore-start */';\nconst kept = 2;\n// jscpd:ignore-end\n";
        assert!(find_ignore_ranges(source, "javascript").is_empty());
        assert!(kept(source, "javascript").contains(&"kept".to_string()));
    }

    /// Sources oxc gives up on still honour the markers through the byte
    /// scan the word-split fallback uses.
    #[test]
    fn unparsable_sources_keep_the_scanned_markers() {
        let source = "let x = {{{\n// jscpd:ignore-start\nhidden\n// jscpd:ignore-end\nafter\n";
        assert_eq!(
            find_ignore_ranges(source, "javascript"),
            scan_ignore_ranges(source)
        );
        assert_eq!(
            tokenize_js(source, "javascript")[0].kind,
            TokenKind::Other,
            "word-split fallback"
        );
        let kept = kept(source, "javascript");
        assert!(!kept.contains(&"hidden".to_string()), "{kept:?}");
        assert!(kept.contains(&"after".to_string()), "{kept:?}");
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
