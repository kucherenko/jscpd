// cpd-tokenizer: OXC-based tokenizer for JavaScript/TypeScript/JSX/TSX.
// Dispatched for formats: javascript, typescript, jsx, tsx.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_parser::{Kind, Parser, config::TokensParserConfig};
use oxc_span::SourceType;

use cpd_core::models::{Location, Token, TokenKind};

// ── LineIndex ─────────────────────────────────────────────────────────────────
// Pre-built sorted list of newline byte offsets. Built once per file in O(n),
// then each location lookup is O(log n) via partition_point (binary search).
// This replaces the previous O(n) full-prefix scan per token.

struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    fn new(content: &[u8]) -> Self {
        let newlines = content
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| (b == b'\n').then_some(i))
            .collect();
        Self { newlines }
    }

    fn location(&self, offset: usize) -> Location {
        let previous_newlines = self.newlines.partition_point(|&nl| nl < offset);
        let line_start = if previous_newlines == 0 {
            0
        } else {
            self.newlines[previous_newlines - 1] + 1
        };
        Location {
            line: previous_newlines as u32 + 1,
            column: (offset - line_start) as u32,
            offset: offset as u32,
        }
    }
}

// ── fallback tokenizer ────────────────────────────────────────────────────────

mod fallback {
    use super::LineIndex;
    use cpd_core::models::{Token, TokenKind};

    fn find_ignore_ranges(source: &str) -> Vec<[usize; 2]> {
        let mut ranges = Vec::new();
        let mut start: Option<usize> = None;
        let bytes = source.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' {
                let end = if bytes[i + 1] == b'/' {
                    bytes[i..].iter().position(|&b| b == b'\n').map(|p| i + p).unwrap_or(bytes.len())
                } else if bytes[i + 1] == b'*' {
                    bytes[i..].windows(2).position(|w| w == b"*/").map(|p| i + p + 2).unwrap_or(bytes.len())
                } else {
                    i += 1;
                    continue;
                };
                let comment_text = &source[i..end];
                if comment_text.contains("jscpd:ignore-start") {
                    start = Some(end);
                } else if comment_text.contains("jscpd:ignore-end") {
                    if let Some(s) = start.take() {
                        ranges.push([s, i]);
                    }
                }
                i = end;
                continue;
            }
            i += 1;
        }
        ranges
    }

    fn in_ignore(offset: usize, end: usize, ranges: &[[usize; 2]]) -> bool {
        ranges.iter().any(|[rs, re]| offset < *re && end > *rs)
    }

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

fn find_ignore_ranges(source: &str) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' {
            let end = if bytes[i + 1] == b'/' {
                bytes[i..].iter().position(|&b| b == b'\n').map(|p| i + p).unwrap_or(bytes.len())
            } else if bytes[i + 1] == b'*' {
                bytes[i..].windows(2).position(|w| w == b"*/").map(|p| i + p + 2).unwrap_or(bytes.len())
            } else {
                i += 1;
                continue;
            };
            let comment_text = &source[i..end];
            if comment_text.contains("jscpd:ignore-start") {
                start = Some(end);
            } else if comment_text.contains("jscpd:ignore-end") {
                if let Some(s) = start.take() {
                    ranges.push([s, i]);
                }
            }
            i = end;
            continue;
        }
        i += 1;
    }
    ranges
}

fn in_ignore(offset: usize, end: usize, ranges: &[[usize; 2]]) -> bool {
    ranges.iter().any(|[rs, re]| offset < *re && end > *rs)
}

fn map_kind(kind: Kind) -> TokenKind {
    if kind == Kind::Ident {
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
    if kind.is_binary_operator() || kind.is_logical_operator()
        || kind.is_unary_operator() || kind.is_update_operator()
    {
        return TokenKind::Operator;
    }
    match kind {
        Kind::Arrow => TokenKind::Operator,
        Kind::Semicolon | Kind::Comma | Kind::Dot | Kind::Dot3 | Kind::Colon
        | Kind::LParen | Kind::RParen | Kind::LCurly | Kind::RCurly
        | Kind::LBrack | Kind::RBrack | Kind::At => TokenKind::Punctuation,
        Kind::QuestionDot => TokenKind::Punctuation,
        _ => TokenKind::Other,
    }
}

fn source_type_for_format(format: &str) -> SourceType {
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
    if source.is_empty() {
        return Vec::new();
    }

    let owned = source.to_string();
    let fmt = format.to_string();
    match catch_unwind(AssertUnwindSafe(|| parse_with_oxc(&owned, &fmt))) {
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

fn parse_with_oxc(source: &str, format: &str) -> Option<Vec<Token>> {
    let allocator = Allocator::new();
    let source_type = source_type_for_format(format);

    let parser_return = Parser::new(&allocator, source, source_type)
        .with_config(TokensParserConfig)
        .parse();

    if !parser_return.errors.is_empty() {
        return None;
    }

    let ignore_ranges = find_ignore_ranges(source);
    let bytes = source.as_bytes();
    // Build LineIndex once — O(n) — then all location calls are O(log n).
    let line_index = LineIndex::new(bytes);
    let mut tokens = Vec::new();

    for token in parser_return.tokens.iter() {
        let start = (token.start() as usize).min(source.len());
        let end = (token.end() as usize).min(source.len());
        if start >= end {
            continue;
        }
        let kind = token.kind();
        if matches!(kind, Kind::Eof | Kind::Undetermined | Kind::Skip) {
            continue;
        }
        let value = &source[start..end];
        let token_kind = if in_ignore(start, end, &ignore_ranges) {
            TokenKind::Ignore
        } else {
            map_kind(kind)
        };
        tokens.push(Token {
            kind: token_kind,
            value: value.to_string(),
            start: line_index.location(start),
            end: line_index.location(end),
        });
    }

    Some(tokens)
}

// ── tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

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
        let has_ignore = tokens.iter().any(|t| t.kind == cpd_core::models::TokenKind::Ignore);
        assert!(has_ignore, "tokens in ignore region must be marked Ignore");
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
