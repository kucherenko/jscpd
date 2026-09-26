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
    /// No comments
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

        "ini" | "properties" | "asm6502" | "nasm" | "lisp" | "clojure" | "scheme" | "racket" => {
            CommentStyle::Semicolon
        }

        "vb" | "vbs" | "basic" | "vbnet" | "visual-basic" => CommentStyle::VisualBasic,

        // Markdown has no comment of its own. CommonMark defines only the HTML
        // comment, as HTML block type 2. Without this arm Markdown prose falls
        // to the C style below, and then a `/*` that prose can hold — inside a
        // glob such as `docs/**`, or in a code span — opens a block comment
        // that never closes. Every later clone in that file goes unreported.
        "markdown" | "md" => CommentStyle::None,

        // Plain text, logs and CSV have no comment syntax either, and take the
        // same loss through the C fallback: a path glob such as `logs/**`
        // hides the rest of the file, and `https://` hides the rest of a line.
        "txt" | "log" | "csv" => CommentStyle::None,

        _ => CommentStyle::CStyle,
    }
}

fn is_ignore_start(text: &str) -> bool {
    text.contains("jscpd:ignore-start")
}

fn is_ignore_end(text: &str) -> bool {
    text.contains("jscpd:ignore-end")
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

/// A line being cut into tokens: its characters, how far the cut has reached,
/// and the column that position sits at. Columns count UTF-8 bytes, as token
/// offsets do.
struct LineCursor<'a> {
    line: &'a str,
    chars: Vec<(usize, char)>,
    at: usize,
    col: u32,
    line_num: u32,
    line_offset: u32,
    in_ignore: bool,
    tokens: Vec<Token>,
}

impl<'a> LineCursor<'a> {
    fn new(line: &'a str, line_num: u32, line_offset: u32, in_ignore: bool) -> Self {
        Self {
            line,
            chars: line.char_indices().collect(),
            at: 0,
            col: 0,
            line_num,
            line_offset,
            in_ignore,
            tokens: Vec::new(),
        }
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).map(|&(_, c)| c)
    }

    /// Whether the characters from the cursor on spell `text`.
    fn looking_at(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(ahead, c)| self.peek(ahead) == Some(c))
    }

    /// `kind`, unless the line lies inside a `jscpd:ignore` region.
    fn kind(&self, kind: TokenKind) -> TokenKind {
        match self.in_ignore {
            true => TokenKind::Ignore,
            false => kind,
        }
    }

    fn advance(&mut self) {
        if let Some(&(_, c)) = self.chars.get(self.at) {
            self.col += c.len_utf8() as u32;
            self.at += 1;
        }
    }

    fn byte(&self, index: usize) -> usize {
        self.chars
            .get(index)
            .map_or(self.line.len(), |&(byte, _)| byte)
    }

    /// Emit the characters from `start`, which sat at column `start_col`, up
    /// to the cursor.
    fn emit_from(&mut self, kind: TokenKind, start: usize, start_col: u32) {
        let text = &self.line[self.byte(start)..self.byte(self.at)];
        self.tokens.push(make_token(
            kind,
            text,
            self.line_num,
            start_col,
            self.line_offset + start_col,
        ));
    }

    /// The next `count` characters, as one token.
    fn take(&mut self, kind: TokenKind, count: usize) {
        let (start, start_col) = (self.at, self.col);
        for _ in 0..count {
            self.advance();
        }
        self.emit_from(kind, start, start_col);
    }

    /// Characters for as long as `keep` holds, as one token.
    fn take_while(&mut self, kind: TokenKind, keep: impl Fn(char) -> bool) {
        let (start, start_col) = (self.at, self.col);
        while self.peek(0).is_some_and(&keep) {
            self.advance();
        }
        self.emit_from(kind, start, start_col);
    }

    /// Everything left on the line, as one token.
    fn take_rest(&mut self, kind: TokenKind) {
        let (start, start_col) = (self.at, self.col);
        self.at = self.chars.len();
        self.emit_from(kind, start, start_col);
    }

    /// A quoted string up to and including its closing quote, or to the end
    /// of the line when it has none. A backslash escapes the character after
    /// it.
    fn take_string(&mut self) {
        let (start, start_col) = (self.at, self.col);
        let quote = self.peek(0);
        self.advance();
        while let Some(c) = self.peek(0) {
            if Some(c) == quote {
                break;
            }
            if c == '\\' && self.peek(1).is_some() {
                self.advance();
            }
            self.advance();
        }
        self.advance();
        let kind = self.kind(TokenKind::Literal);
        self.emit_from(kind, start, start_col);
    }

    /// An identifier or keyword; a word of digits alone is a literal.
    fn take_word(&mut self) {
        let (start, start_col) = (self.at, self.col);
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.advance();
        }
        let kind = match self.in_ignore {
            true => TokenKind::Ignore,
            false => classify_word(&self.line[self.byte(start)..self.byte(self.at)]),
        };
        self.emit_from(kind, start, start_col);
    }
}

/// Whether a line comment in this style starts at the cursor.
///
/// Lua's long comment `--[[` starts with its line comment `--`, and is read
/// the same way: the rest of the line is the comment.
fn opens_line_comment(style: CommentStyle, cursor: &LineCursor) -> bool {
    match style {
        CommentStyle::CStyle => cursor.looking_at("//"),
        CommentStyle::Hash => cursor.looking_at("#"),
        CommentStyle::DoubleDash | CommentStyle::Lua => cursor.looking_at("--"),
        CommentStyle::Semicolon => cursor.looking_at(";"),
        CommentStyle::VisualBasic => cursor.looking_at("'"),
        CommentStyle::None => false,
    }
}

fn tokenize_line_content(
    line: &str,
    line_num: u32,
    line_offset: u32,
    style: CommentStyle,
    in_ignore: bool,
    in_block_comment: &mut bool,
) -> Vec<Token> {
    let mut cursor = LineCursor::new(line, line_num, line_offset, in_ignore);
    let c_style = style == CommentStyle::CStyle;
    while let Some(ch) = cursor.peek(0) {
        if *in_block_comment {
            // Inside `/* … */` each character is a comment token of its own.
            let closes = c_style && cursor.looking_at("*/");
            cursor.take(cursor.kind(TokenKind::Comment), if closes { 2 } else { 1 });
            *in_block_comment = !closes;
        } else if c_style && cursor.looking_at("/*") {
            cursor.take(cursor.kind(TokenKind::Comment), 2);
            *in_block_comment = true;
        } else if opens_line_comment(style, &cursor) {
            cursor.take_rest(cursor.kind(TokenKind::Comment));
            break;
        } else if ch == '"' || ch == '\'' {
            cursor.take_string();
        } else if ch.is_whitespace() {
            cursor.take_while(cursor.kind(TokenKind::Whitespace), char::is_whitespace);
        } else if ch.is_ascii_digit() {
            cursor.take_while(cursor.kind(TokenKind::Literal), |c| {
                c.is_ascii_digit() || c == '.'
            });
        } else if ch.is_alphabetic() || ch == '_' {
            cursor.take_word();
        } else {
            cursor.take(cursor.kind(TokenKind::Punctuation), 1);
        }
    }
    cursor.tokens
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

    // `split_inclusive` keeps each line's ending, so offsets advance by the
    // bytes really there: `\r\n` counts two, and CRLF files keep true byte
    // offsets (with `lines()`, every line drifted by the dropped `\r`).
    for (line_idx, raw) in source.split_inclusive('\n').enumerate() {
        let line = raw
            .strip_suffix('\n')
            .map_or(raw, |l| l.strip_suffix('\r').unwrap_or(l));
        let line_num = line_idx as u32 + 1;
        let trimmed = line.trim();

        if is_ignore_start(trimmed) {
            in_ignore = true;
        }
        if is_ignore_end(trimmed) {
            in_ignore = false;
            // Advance offset past this line and continue
            offset += raw.len() as u32;
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
        offset += raw.len() as u32;
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_offsets_are_byte_offsets() {
        let src = "let a = 1;\r\nlet b = 2;\r\n";
        for token in tokenize_generic(src, "rust") {
            let slice = &src[token.start.offset as usize..token.end.offset as usize];
            assert_eq!(slice, token.value, "{token:?}");
        }
        let b = tokenize_generic(src, "rust")
            .into_iter()
            .find(|t| t.value == "b")
            .unwrap();
        assert_eq!((b.start.line, b.start.column, b.start.offset), (2, 4, 16));
    }

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
    fn lisp_family_semicolon_comment_marked_as_comment() {
        for format in ["lisp", "clojure", "scheme", "racket"] {
            let tokens = tokenize_generic(";; a comment\n(def x 1)\n", format);
            let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
            assert!(has_comment, "{format} ; comments must be Comment kind");
        }
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
    fn markdown_has_no_block_comment() {
        // A glob in prose holds `/*`. Markdown has no block comment, so the
        // text after it must stay ordinary code tokens.
        let tokens = tokenize_generic("Ignore `docs/**` here.\nThe next line.\n", "markdown");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(
            !has_comment,
            "Markdown has no block comment, so `/*` must not open one"
        );
        let last = tokens.last().expect("at least one token");
        assert_eq!(
            last.start.line, 2,
            "text after `/*` must still produce tokens"
        );
    }

    #[test]
    fn markdown_has_no_line_comment() {
        let tokens = tokenize_generic("A URL like https://example.com/x\n", "markdown");
        let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
        assert!(!has_comment, "`//` must not open a comment in Markdown");
    }

    #[test]
    fn plain_text_formats_have_no_comments() {
        for format in ["txt", "log", "csv"] {
            let tokens = tokenize_generic(
                "Skip `docs/**` and see https://example.com/x
The next line.
",
                format,
            );
            let has_comment = tokens.iter().any(|t| t.kind == TokenKind::Comment);
            assert!(!has_comment, "{format} has no comment syntax");
            let last = tokens.last().expect("at least one token");
            assert_eq!(
                last.start.line, 2,
                "{format}: text after `/*` must still produce tokens"
            );
        }
    }

    #[test]
    fn location_line_numbers_are_1_based() {
        let tokens = tokenize_generic("x = 1\ny = 2\n", "python");
        let first = tokens.first().expect("at least one token");
        assert_eq!(first.start.line, 1);
    }
}
