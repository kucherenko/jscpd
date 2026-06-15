// cpd-tokenizer: generic whitespace-and-punctuation tokenizer for non-JS/TS formats.
// Handles comment styles, ignore regions, and per-line token scanning without regex.

use cpd_core::models::{Location, Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentStyle {
    /// Single-line `//`, block `/* */`
    CStyle,
    /// Single-line `#`
    Hash,
    /// Single-line `--`
    DoubleDash,
    /// Single-line `--`, block `--[[ ]]`
    Lua,
    /// Single-line `;`
    Semicolon,
    /// Single-line `'`
    VisualBasic,
    /// No comments (fallback for unrecognised formats)
    #[allow(dead_code)]
    None,
}

fn comment_style(format: &str) -> CommentStyle {
    match format {
        "c" | "c-header" | "cpp" | "cpp-header" | "csharp" | "java" | "go" | "rust" | "swift"
        | "kotlin" | "scala" | "dart" | "php" | "typescript" | "jsx" | "tsx" | "javascript"
        | "groovy" | "d" | "glsl" | "hlsl" | "wgsl" | "openqasm" | "solidity" | "bicep" | "hcl"
        | "json5" | "less" | "scss" | "css" | "objectivec" | "protobuf" | "apex" | "verilog"
        | "zig" | "odin" | "fsharp" | "actionscript" | "cfscript" => CommentStyle::CStyle,

        "python" | "ruby" | "perl" | "bash" | "sh" | "zsh" | "fish" | "r" | "julia" | "yaml"
        | "toml" | "dockerfile" | "makefile" | "cmake" | "coffeescript" | "crystal" | "nim"
        | "gdscript" | "elixir" | "awk" | "tcl" | "powershell" | "puppet" | "ignore" => {
            CommentStyle::Hash
        }

        "sql" | "haskell" | "elm" | "ada" | "plsql" => CommentStyle::DoubleDash,

        "lua" => CommentStyle::Lua,

        "ini" | "properties" | "asm6502" | "nasm" => CommentStyle::Semicolon,

        "vb" | "vbs" | "basic" | "vbnet" | "visual-basic" => CommentStyle::VisualBasic,

        _ => CommentStyle::CStyle,
    }
}

fn is_ignore_start(text: &str) -> bool {
    text.contains("jscpd:ignore-start")
}

fn is_ignore_end(text: &str) -> bool {
    text.contains("jscpd:ignore-end")
}

fn comment_kind(in_ignore: bool) -> TokenKind {
    if in_ignore {
        TokenKind::Ignore
    } else {
        TokenKind::Comment
    }
}

fn make_token(kind: TokenKind, value: &str, line: u32, col: u32, offset: u32) -> Token {
    let len = value.len() as u32;
    Token {
        kind,
        value: value.to_string(),
        start: Location {
            line,
            column: col,
            offset,
        },
        end: Location {
            line,
            column: col + len,
            offset: offset + len,
        },
    }
}

fn classify_word(word: &str) -> TokenKind {
    if word.chars().all(|c| c.is_ascii_digit()) {
        return TokenKind::Literal;
    }
    if word.chars().all(|c| c.is_ascii_punctuation()) {
        return TokenKind::Punctuation;
    }
    TokenKind::Identifier
}

fn tokenize_line_content(
    line: &str,
    line_num: u32,
    line_offset: u32,
    style: CommentStyle,
    in_ignore: bool,
    in_block_comment: &mut bool,
) -> Vec<Token> {
    let mut tokens = Vec::new();

    // Collect (byte_offset, char) pairs once — zero heap allocation vs chars().collect().
    // `char_indices()` returns (byte_index, char) which gives us correct UTF-8 byte offsets
    // for column accounting while avoiding a Vec<char> heap allocation per line.
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let n = chars.len();
    let mut i = 0usize;

    // col is in bytes (UTF-8 units), consistent with char.len_utf8() increments below.
    let mut col = 0u32;

    macro_rules! offset {
        () => {
            line_offset + col
        };
    }

    while i < n {
        let (_, ch) = chars[i];

        // Handle block comment end
        if *in_block_comment {
            let kind = comment_kind(in_ignore);
            if matches!(style, CommentStyle::CStyle)
                && i + 1 < n
                && ch == '*'
                && chars[i + 1].1 == '/'
            {
                let start_col = col;
                let start_off = offset!();
                col += 2;
                i += 2;
                tokens.push(make_token(kind, "*/", line_num, start_col, start_off));
                *in_block_comment = false;
                continue;
            }
            // Still inside block comment — consume char
            let start_col = col;
            let start_off = offset!();
            let mut s = String::new();
            s.push(ch);
            col += ch.len_utf8() as u32;
            i += 1;
            tokens.push(make_token(kind, &s, line_num, start_col, start_off));
            continue;
        }

        // Lua long block comment --[[
        if matches!(style, CommentStyle::Lua)
            && i + 3 < n
            && ch == '-'
            && chars[i + 1].1 == '-'
            && chars[i + 2].1 == '['
            && chars[i + 3].1 == '['
        {
            let rest = &line[chars[i].0..];
            tokens.push(make_token(
                comment_kind(in_ignore),
                rest,
                line_num,
                col,
                offset!(),
            ));
            break;
        }

        // C-style block comment open /*
        if matches!(style, CommentStyle::CStyle) && i + 1 < n && ch == '/' && chars[i + 1].1 == '*'
        {
            *in_block_comment = true;
            let start_col = col;
            let start_off = offset!();
            col += 2;
            i += 2;
            tokens.push(make_token(
                comment_kind(in_ignore),
                "/*",
                line_num,
                start_col,
                start_off,
            ));
            continue;
        }

        // Line comment — check current position directly without allocating
        let is_comment = match style {
            CommentStyle::CStyle => i + 1 < n && ch == '/' && chars[i + 1].1 == '/',
            CommentStyle::Hash => ch == '#',
            CommentStyle::DoubleDash | CommentStyle::Lua => {
                i + 1 < n && ch == '-' && chars[i + 1].1 == '-'
            }
            CommentStyle::Semicolon => ch == ';',
            CommentStyle::VisualBasic => ch == '\'',
            CommentStyle::None => false,
        };

        if is_comment {
            let rest = &line[chars[i].0..];
            tokens.push(make_token(
                comment_kind(in_ignore),
                rest,
                line_num,
                col,
                offset!(),
            ));
            break;
        }

        // String literals (double-quote or single-quote)
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start_col = col;
            let start_off = offset!();
            let mut j = chars[i].0; // byte start of string in `line`
            let str_start = j;
            col += 1;
            i += 1;
            j += 1;
            while i < n && chars[i].1 != quote {
                if chars[i].1 == '\\' && i + 1 < n {
                    col += chars[i].1.len_utf8() as u32 + chars[i + 1].1.len_utf8() as u32;
                    i += 2;
                } else {
                    col += chars[i].1.len_utf8() as u32;
                    i += 1;
                }
            }
            if i < n {
                col += 1;
                i += 1;
            }
            let str_end = if i < n {
                chars[i - 1].0 + chars[i - 1].1.len_utf8()
            } else {
                line.len()
            };
            let _ = (j, str_start); // byte indices computed above but using slice below
            let s = &line[str_start..str_end];
            let kind = if in_ignore {
                TokenKind::Ignore
            } else {
                TokenKind::Literal
            };
            tokens.push(make_token(kind, s, line_num, start_col, start_off));
            continue;
        }

        // Whitespace
        if ch.is_whitespace() {
            let start_col = col;
            let start_off = offset!();
            let byte_start = chars[i].0;
            while i < n && chars[i].1.is_whitespace() {
                col += chars[i].1.len_utf8() as u32;
                i += 1;
            }
            let byte_end = if i < n { chars[i].0 } else { line.len() };
            let kind = if in_ignore {
                TokenKind::Ignore
            } else {
                TokenKind::Whitespace
            };
            tokens.push(make_token(
                kind,
                &line[byte_start..byte_end],
                line_num,
                start_col,
                start_off,
            ));
            continue;
        }

        // Numbers
        if ch.is_ascii_digit() {
            let start_col = col;
            let start_off = offset!();
            let byte_start = chars[i].0;
            while i < n && (chars[i].1.is_ascii_digit() || chars[i].1 == '.') {
                col += 1;
                i += 1;
            }
            let byte_end = if i < n { chars[i].0 } else { line.len() };
            let kind = if in_ignore {
                TokenKind::Ignore
            } else {
                TokenKind::Literal
            };
            tokens.push(make_token(
                kind,
                &line[byte_start..byte_end],
                line_num,
                start_col,
                start_off,
            ));
            continue;
        }

        // Identifiers / keywords
        if ch.is_alphabetic() || ch == '_' {
            let start_col = col;
            let start_off = offset!();
            let byte_start = chars[i].0;
            while i < n && (chars[i].1.is_alphanumeric() || chars[i].1 == '_') {
                col += chars[i].1.len_utf8() as u32;
                i += 1;
            }
            let byte_end = if i < n { chars[i].0 } else { line.len() };
            let s = &line[byte_start..byte_end];
            let kind = if in_ignore {
                TokenKind::Ignore
            } else {
                classify_word(s)
            };
            tokens.push(make_token(kind, s, line_num, start_col, start_off));
            continue;
        }

        // Operators / punctuation (single char)
        let start_col = col;
        let start_off = offset!();
        let byte_start = chars[i].0;
        col += ch.len_utf8() as u32;
        i += 1;
        let byte_end = if i < n { chars[i].0 } else { line.len() };
        let kind = if in_ignore {
            TokenKind::Ignore
        } else {
            TokenKind::Punctuation
        };
        tokens.push(make_token(
            kind,
            &line[byte_start..byte_end],
            line_num,
            start_col,
            start_off,
        ));
    }

    tokens
}

/// Tokenize source in the given format. Never panics on empty input.
pub fn tokenize_generic(source: &str, format: &str) -> Vec<Token> {
    if source.is_empty() {
        return Vec::new();
    }

    let style = comment_style(format);
    let mut tokens = Vec::new();
    let mut in_ignore = false;
    let mut in_block_comment = false;
    let mut offset = 0u32;

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx as u32 + 1;
        let trimmed = line.trim();

        if is_ignore_start(trimmed) {
            in_ignore = true;
        }
        if is_ignore_end(trimmed) {
            in_ignore = false;
            // Advance offset past this line and continue
            offset += line.len() as u32 + 1;
            continue;
        }

        let line_tokens = tokenize_line_content(
            line,
            line_num,
            offset,
            style,
            in_ignore,
            &mut in_block_comment,
        );
        tokens.extend(line_tokens);
        offset += line.len() as u32 + 1;
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_produces_tokens() {
        let tokens = tokenize_generic("def hello():\n    return 42\n", "python");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn python_hash_comment_marked_as_comment() {
        let tokens = tokenize_generic("# this is a comment\nx = 1\n", "python");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(has_comment, "Python # comments must be Comment kind");
    }

    #[test]
    fn go_c_style_comment_recognized() {
        let tokens = tokenize_generic("// hello\nfunc main() {}\n", "go");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(has_comment);
    }

    #[test]
    fn empty_input_returns_empty() {
        let tokens = tokenize_generic("", "python");
        assert!(
            tokens.is_empty(),
            "empty input must return empty vec, not panic"
        );
    }

    #[test]
    fn unknown_format_does_not_panic() {
        let result =
            std::panic::catch_unwind(|| tokenize_generic("hello world", "unknown_format_xyz"));
        assert!(result.is_ok());
    }

    #[test]
    fn ignore_region_tokens_marked_as_ignore() {
        let source = "x = 1\n# jscpd:ignore-start\ny = 2\n# jscpd:ignore-end\nz = 3\n";
        let tokens = tokenize_generic(source, "python");
        let has_ignore = tokens.iter().any(|t| t.kind == TokenKind::Ignore);
        assert!(has_ignore, "tokens in ignore region must be Ignore kind");
    }

    #[test]
    fn sql_double_dash_comment_recognized() {
        let tokens = tokenize_generic("-- a comment\nSELECT * FROM foo;\n", "sql");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(has_comment);
    }

    #[test]
    fn c_block_comment_recognized() {
        let tokens = tokenize_generic("/* block */\nint x = 1;\n", "c");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(has_comment);
    }

    #[test]
    fn location_line_numbers_are_1_based() {
        let tokens = tokenize_generic("x = 1\ny = 2\n", "python");
        let first = tokens.first().expect("at least one token");
        assert_eq!(first.start.line, 1);
    }
}
