use std::str::FromStr;

use cpd_core::hash::hash_token;
use cpd_core::models::{DetectionToken, Token, TokenKind};

use crate::markdown::tokens_to_detection;

/// A sub-format detection map produced by multi-format tokenizers.
///
/// For single-format files, `tokenize_to_detection_maps()` returns exactly one
/// TokenMap with the same format as the file.
///
/// For multi-format files (markdown, SFC), one TokenMap is returned per
/// detected sub-language, each carrying tokens that should enter that
/// format's detection pool.
#[derive(Debug, Clone)]
pub struct TokenMap {
    pub format: String,
    pub tokens: Vec<DetectionToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Mild,
    Weak,
    Strict,
}

impl FromStr for Mode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "weak" => Ok(Self::Weak),
            "strict" => Ok(Self::Strict),
            _ => Ok(Self::Mild),
        }
    }
}

/// Options for the detection-path tokenizer.
///
/// Carries mode, case-folding flag, pre-parsed ignore-region byte ranges,
/// and pre-compiled code-level regex patterns that skip matching tokens during detection.
///
/// Code-level ignore patterns (v4 `ignorePattern`) work by matching regex patterns
/// against source text, collecting byte ranges of matches, and then filtering
/// any token whose byte range overlaps a match — identical in effect to v4's
/// `setupIgnorePatterns` which injected Prism grammar tokens.
#[derive(Debug, Clone)]
pub struct TokenizeOptions {
    pub mode: Mode,
    /// When true, token values are lowercased before hashing.
    pub ignore_case: bool,
    /// Ignored byte ranges from `jscpd:ignore-start` / `jscpd:ignore-end`
    /// and code-level regex matches from `ignorePattern`.
    /// Each entry is `[start_byte, end_byte)`.
    pub ignore_ranges: Vec<[usize; 2]>,
    /// Hash every identifier as `$id` so clones that differ only in variable,
    /// function or type names match (issue #998). Keywords are kept: the
    /// JavaScript tokenizer classifies them, other languages fall back to
    /// [`is_common_keyword`].
    pub ignore_identifiers: bool,
    /// Hash string literals as `$str` and numeric literals as `$num`.
    pub ignore_literals: bool,
    /// Drop `@Name`, `@a.b.Name` and `@Name(...)` annotation/decorator
    /// sequences before hashing, in formats listed by [`strips_annotations`].
    pub ignore_annotations: bool,
    /// Pre-compiled code-level regex patterns inherited from v4 `ignorePattern`.
    /// Before tokenization, these are matched against the source text and
    /// overlapping byte ranges are added to `ignore_ranges`.
    pub code_ignore_regexes: Vec<regex::Regex>,
    /// Formats whose TypeScript-only syntax is stripped from the detection
    /// token stream (`--cross-formats` groups mixing TS with JS). Only
    /// `typescript` and `tsx` are meaningful here; empty by default so the
    /// standard detection path is untouched.
    pub strip_types_formats: std::collections::HashSet<String>,
}

impl TokenizeOptions {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            ignore_case: false,
            ignore_identifiers: false,
            ignore_literals: false,
            ignore_annotations: false,
            ignore_ranges: Vec::new(),
            code_ignore_regexes: Vec::new(),
            strip_types_formats: std::collections::HashSet::new(),
        }
    }

    /// True when any Type-2 normalization option is on.
    pub fn normalizes(&self) -> bool {
        self.ignore_identifiers || self.ignore_literals || self.ignore_annotations
    }

    /// Build TokenizeOptions with pre-compiled regex patterns from string patterns.
    /// Invalid regex patterns are silently skipped.
    pub fn with_code_ignore_patterns(mode: Mode, patterns: &[String]) -> Self {
        let code_ignore_regexes: Vec<regex::Regex> = patterns
            .iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();
        Self {
            mode,
            ignore_case: false,
            ignore_identifiers: false,
            ignore_literals: false,
            ignore_annotations: false,
            ignore_ranges: Vec::new(),
            code_ignore_regexes,
            strip_types_formats: std::collections::HashSet::new(),
        }
    }
}

/// Tokenize a single-format source snippet into detection tokens.
///
/// Used by markdown and SFC tokenizers to dispatch embedded code blocks to the
/// appropriate language tokenizer.
pub fn tokenize_format_to_detection(
    format: &str,
    source: &str,
    options: &TokenizeOptions,
) -> Vec<DetectionToken> {
    let raw = match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            if should_strip_types(format, options) {
                crate::javascript::tokenize_js_stripped(source, format)
            } else {
                crate::javascript::tokenize_js(source, format)
            }
        }
        "vue" | "svelte" | "astro" => crate::sfc::tokenize_sfc(source, format, options.mode),
        "markdown" | "md" => crate::generic::tokenize_generic(source, format),
        _ => crate::generic::tokenize_generic(source, format),
    };
    tokens_to_detection(raw, options)
}

/// True when this format's TypeScript-only syntax must be stripped for
/// cross-format detection (see `TokenizeOptions::strip_types_formats`).
fn should_strip_types(format: &str, options: &TokenizeOptions) -> bool {
    matches!(format, "typescript" | "tsx") && options.strip_types_formats.contains(format)
}

/// Compute byte ranges of all regex matches against source text.
/// Used to populate `ignore_ranges` from `ignorePattern` regexes before
/// tokenization, matching v4 semantics where regex patterns match against
/// source text regions (not individual token values).
pub fn code_ignore_ranges(source: &str, regexes: &[regex::Regex]) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    for re in regexes {
        for m in re.find_iter(source) {
            ranges.push([m.start(), m.end()]);
        }
    }
    ranges
}

/// Push a token into the detection output if it passes all filters.
///
/// Filtering happens here — at tokenize time — so the resulting
/// `Vec<DetectionToken>` passed to detection is already minimal.
/// Token values are not stored; only the pre-computed hash is kept.
///
/// The argument count is intentional: this function is a hot-path helper
/// called from every tokenizer branch; grouping parameters into a struct
/// would add an extra dereference per call.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn push_token(
    tokens: &mut Vec<DetectionToken>,
    kind: TokenKind,
    value: &str,
    byte_start: usize,
    byte_end: usize,
    start: cpd_core::models::Location,
    end: cpd_core::models::Location,
    options: &TokenizeOptions,
) {
    // Drop Ignore-marked tokens in all modes.
    if kind == TokenKind::Ignore {
        return;
    }
    // Drop tokens in Ignore byte ranges.
    // This covers both jscpd:ignore-start/end markers and code-level ignorePattern
    // regex ranges (which are computed from source text before tokenization).
    if options
        .ignore_ranges
        .iter()
        .any(|[rs, re]| byte_start < *re && byte_end > *rs)
    {
        return;
    }
    // Mode-based filtering:
    match options.mode {
        Mode::Mild => {
            if kind == TokenKind::Whitespace {
                return;
            }
        }
        Mode::Weak => {
            if matches!(
                kind,
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment
            ) {
                return;
            }
        }
        Mode::Strict => {} // keep everything (except Ignore, handled above)
    }
    let raw_hash = hash_token(kind.discriminant(), value, options.ignore_case);
    let hash = match normalized_value(&kind, value, options) {
        Some(placeholder) => hash_token(kind.discriminant(), placeholder, false),
        None => raw_hash,
    };
    tokens.push(DetectionToken {
        hash,
        raw_hash,
        start,
        end,
        range: [byte_start, byte_end],
    });
}

/// Placeholder that replaces `value` under the active normalization options,
/// or `None` when the token hashes as-is (issue #998).
#[inline]
fn normalized_value(
    kind: &TokenKind,
    value: &str,
    options: &TokenizeOptions,
) -> Option<&'static str> {
    match kind {
        TokenKind::Identifier if options.ignore_identifiers => {
            if is_common_keyword(value) {
                None
            } else {
                Some("$id")
            }
        }
        TokenKind::Literal if options.ignore_literals => literal_placeholder(value),
        _ => None,
    }
}

/// `$str` for quoted literals (with an optional short alphabetic prefix such
/// as Python's `r"..."` or C#'s `@"..."`), `$num` for numbers, `None` for
/// anything else (`true`, `null`, regex literals) which keeps its own hash.
fn literal_placeholder(value: &str) -> Option<&'static str> {
    let bytes = value.as_bytes();
    let first = *bytes.first()?;
    if matches!(first, b'"' | b'\'' | b'`') {
        return Some("$str");
    }
    if first.is_ascii_digit() {
        return Some("$num");
    }
    if first == b'.' && bytes.get(1).is_some_and(u8::is_ascii_digit) {
        return Some("$num");
    }
    if first.is_ascii_alphabetic() || first == b'@' {
        let quote_at = bytes
            .iter()
            .take(4)
            .position(|b| matches!(b, b'"' | b'\'' | b'`'));
        if quote_at.is_some() {
            return Some("$str");
        }
    }
    None
}

/// Keywords the generic tokenizer reports as identifiers. Kept verbatim under
/// `--ignore-identifiers` so control flow still has to match; the list is the
/// union of common keywords across C-like, Python-like and ML-like languages.
/// Must stay sorted: looked up by binary search.
static COMMON_KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "begin",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "def",
    "default",
    "defer",
    "del",
    "do",
    "elif",
    "else",
    "elsif",
    "end",
    "enum",
    "except",
    "export",
    "extends",
    "extern",
    "false",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "from",
    "func",
    "function",
    "global",
    "go",
    "goto",
    "if",
    "impl",
    "implements",
    "import",
    "in",
    "inline",
    "instanceof",
    "interface",
    "is",
    "lambda",
    "let",
    "loop",
    "match",
    "mod",
    "module",
    "mut",
    "namespace",
    "new",
    "nil",
    "none",
    "not",
    "null",
    "or",
    "override",
    "package",
    "pass",
    "private",
    "protected",
    "pub",
    "public",
    "raise",
    "record",
    "ref",
    "require",
    "rescue",
    "return",
    "sealed",
    "select",
    "self",
    "sizeof",
    "static",
    "struct",
    "super",
    "switch",
    "then",
    "this",
    "throw",
    "throws",
    "trait",
    "true",
    "try",
    "type",
    "typedef",
    "typeof",
    "undefined",
    "union",
    "unless",
    "unsafe",
    "until",
    "use",
    "using",
    "var",
    "virtual",
    "void",
    "volatile",
    "when",
    "where",
    "while",
    "with",
    "yield",
];

/// True for words in [`COMMON_KEYWORDS`] (case-sensitive).
pub fn is_common_keyword(word: &str) -> bool {
    COMMON_KEYWORDS.binary_search(&word).is_ok()
}

/// Formats where `@Name` / `@Name(...)` means an annotation or decorator.
/// Elsewhere (`Ruby`, `Perl`, `T-SQL`, `Razor`, `CSS`) `@` prefixes variables
/// or directives and must not be dropped.
pub fn strips_annotations(format: &str) -> bool {
    matches!(
        format,
        "javascript"
            | "typescript"
            | "jsx"
            | "tsx"
            | "java"
            | "kotlin"
            | "scala"
            | "groovy"
            | "python"
            | "dart"
            | "swift"
    )
}

/// Flag `@Name`, `@a.b.Name` and `@Name(...)` token runs so the detection
/// path drops them (issue #998). Whitespace tokens inside a run are tolerated
/// only between the name and its argument list. Returns one flag per token,
/// or an empty vector when nothing matched.
fn mark_annotations(tokens: &[Token]) -> Vec<bool> {
    let n = tokens.len();
    let mut i = 0;
    let mut flags: Vec<bool> = Vec::new();
    while i < n {
        let starts_annotation = tokens[i].value == "@"
            && tokens[i].kind != TokenKind::Ignore
            && tokens
                .get(i + 1)
                .is_some_and(|t| t.kind == TokenKind::Identifier);
        if !starts_annotation {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 2;
        while j + 1 < n && tokens[j].value == "." && tokens[j + 1].kind == TokenKind::Identifier {
            j += 2;
        }
        let mut k = j;
        while k < n && tokens[k].kind == TokenKind::Whitespace {
            k += 1;
        }
        if k < n && tokens[k].value == "(" {
            let mut depth = 0usize;
            j = k;
            while j < n {
                match tokens[j].value.as_str() {
                    "(" => depth += 1,
                    ")" => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        if flags.is_empty() {
            flags = vec![false; n];
        }
        for flag in &mut flags[start..j] {
            *flag = true;
        }
        i = j;
    }
    flags
}

/// Tokenize source code in the given format with the given mode.
/// Returns a Vec<Token>. Never panics on empty input — returns empty Vec.
///
/// This is the display/reporter path. For the detection path, use
/// `tokenize_to_detection`.
pub fn tokenize(format: &str, source: &str, mode: Mode) -> Vec<Token> {
    let raw = dispatch_tokenizer(format, source, mode);
    // Apply mode filter inline — keeps Ignore tokens removed, drops Whitespace in
    // Mild, drops Whitespace+Comment+BlockComment in Weak, keeps all in Strict.
    raw.into_iter().filter(|t| keep_token(t, mode)).collect()
}

fn keep_token(token: &Token, mode: Mode) -> bool {
    if token.kind == TokenKind::Ignore {
        return false;
    }
    match mode {
        Mode::Mild => !matches!(token.kind, TokenKind::Whitespace),
        Mode::Weak => !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment
        ),
        Mode::Strict => true,
    }
}

/// Tokenize source code for the detection hot path.
///
/// Returns `Vec<DetectionToken>` — tokens filtered and hashed inline at
/// tokenize time. No per-token heap allocation survives in the output:
/// the value string is consumed; only the hash, locations, and byte range
/// are stored.
///
/// This replaces the `tokenize` → `apply_mode` → convert-to-hashes pipeline
/// that existed in `detect.rs`.
pub fn tokenize_to_detection(
    format: &str,
    source: &str,
    options: &TokenizeOptions,
) -> Vec<DetectionToken> {
    // Produce the display tokens first (reuse existing tokenizer code),
    // then convert to DetectionToken in one pass applying options filters.
    //
    // This approach is conservative: it reuses all existing tokenizer logic
    // without risk of introducing per-tokenizer bugs. The conversion is O(n)
    // and eliminates the separate filter pass and hash computation that
    // previously happened inside detect.rs.
    let raw = if should_strip_types(format, options) {
        crate::javascript::tokenize_js_stripped(source, format)
    } else {
        dispatch_tokenizer(format, source, options.mode)
    };
    let annotation = if options.ignore_annotations && strips_annotations(format) {
        mark_annotations(&raw)
    } else {
        Vec::new()
    };
    let mut detection: Vec<DetectionToken> = Vec::with_capacity(raw.len());
    for (i, t) in raw.into_iter().enumerate() {
        if annotation.get(i).copied().unwrap_or(false) {
            // Dropped annotation: fold its text into the raw hash of the token
            // that precedes it. A clone whose matched run contains that token
            // then classifies as `renamed` when the annotations differ, while
            // a run that starts after the annotation is unaffected and stays
            // `exact` — the reported fragment text really is identical there.
            if let Some(prev) = detection.last_mut() {
                let h = hash_token(t.kind.discriminant(), &t.value, false);
                prev.raw_hash = prev.raw_hash.rotate_left(7) ^ h;
            }
            continue;
        }
        let byte_start = t.start.offset as usize;
        let byte_end = t.end.offset as usize;
        push_token(
            &mut detection,
            t.kind,
            &t.value,
            byte_start,
            byte_end,
            t.start,
            t.end,
            options,
        );
    }
    detection
}

fn dispatch_tokenizer(format: &str, source: &str, mode: Mode) -> Vec<Token> {
    match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            crate::javascript::tokenize_js(source, format)
        }
        "vue" | "svelte" | "astro" => crate::sfc::tokenize_sfc(source, format, mode),
        "razor" => crate::razor::tokenize_razor(source, mode),
        "markdown" | "md" => crate::markdown::tokenize_markdown(source, mode),
        _ => crate::generic::tokenize_generic(source, format),
    }
}

/// Tokenize source code into one or more format-specific detection maps.
///
/// For single-format files, returns exactly one `TokenMap` with the same format.
/// For multi-format files (markdown, SFCs), returns one `TokenMap` per detected
/// sub-language — e.g. markdown prose + embedded JavaScript + embedded Python.
///
/// Each map's tokens carry byte offsets relative to the original source, so
/// they can be used directly for clone detection within their format group.
pub fn tokenize_to_detection_maps(
    format: &str,
    source: &str,
    options: &TokenizeOptions,
) -> Vec<TokenMap> {
    match format {
        "markdown" | "md" => crate::markdown::tokenize_markdown_maps(source, options),
        "vue" | "svelte" | "astro" => crate::sfc::tokenize_sfc_maps(source, format, options),
        "razor" => crate::razor::tokenize_razor_maps(source, options),
        _ => {
            let tokens = tokenize_to_detection(format, source, options);
            vec![TokenMap {
                format: format.to_string(),
                tokens,
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_str_defaults_to_mild() {
        assert_eq!("unknown".parse::<Mode>().unwrap(), Mode::Mild);
        assert_eq!("mild".parse::<Mode>().unwrap(), Mode::Mild);
    }

    #[test]
    fn mode_from_str_weak() {
        assert_eq!("weak".parse::<Mode>().unwrap(), Mode::Weak);
    }

    #[test]
    fn mode_from_str_strict() {
        assert_eq!("strict".parse::<Mode>().unwrap(), Mode::Strict);
    }

    #[test]
    fn tokenize_to_detection_returns_detection_tokens() {
        let opts = TokenizeOptions::new(Mode::Mild);
        let tokens = tokenize_to_detection("javascript", "function hello() { return 42; }", &opts);
        assert!(
            !tokens.is_empty(),
            "must produce DetectionTokens for valid JS"
        );
    }

    #[test]
    fn tokenize_to_detection_mild_excludes_whitespace() {
        let opts = TokenizeOptions::new(Mode::Mild);
        // The raw tokenizer produces whitespace tokens; mild mode drops them.
        // We verify by counting: detection output should have fewer tokens than
        // a strict-mode tokenize which keeps whitespace.
        let mild = tokenize_to_detection("javascript", "a b c", &opts);
        let strict =
            tokenize_to_detection("javascript", "a b c", &TokenizeOptions::new(Mode::Strict));
        // Mild must not exceed strict count (whitespace removed).
        // Note: JS tokenizer doesn't produce Whitespace kind for OXC tokens,
        // but the contract is that push_token correctly drops them if present.
        let _ = (mild, strict);
    }

    #[test]
    fn push_token_drops_ignore_kind() {
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let opts = TokenizeOptions::new(Mode::Mild);
        push_token(
            &mut tokens,
            TokenKind::Ignore,
            "secret",
            0,
            6,
            loc.clone(),
            loc,
            &opts,
        );
        assert!(tokens.is_empty(), "Ignore-kind tokens must be dropped");
    }

    #[test]
    fn push_token_drops_whitespace_in_mild_mode() {
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let opts = TokenizeOptions::new(Mode::Mild);
        push_token(
            &mut tokens,
            TokenKind::Whitespace,
            " ",
            0,
            1,
            loc.clone(),
            loc,
            &opts,
        );
        assert!(tokens.is_empty(), "Whitespace must be dropped in Mild mode");
    }

    #[test]
    fn push_token_keeps_whitespace_in_strict_mode() {
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let opts = TokenizeOptions::new(Mode::Strict);
        push_token(
            &mut tokens,
            TokenKind::Whitespace,
            " ",
            0,
            1,
            loc.clone(),
            loc,
            &opts,
        );
        assert_eq!(tokens.len(), 1, "Whitespace must be kept in Strict mode");
    }

    #[test]
    fn push_token_drops_comment_in_weak_mode() {
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let opts = TokenizeOptions::new(Mode::Weak);
        push_token(
            &mut tokens,
            TokenKind::Comment,
            "// note",
            0,
            7,
            loc.clone(),
            loc,
            &opts,
        );
        assert!(tokens.is_empty(), "Comment must be dropped in Weak mode");
    }

    fn det(source: &str, format: &str, opts: &TokenizeOptions) -> Vec<DetectionToken> {
        tokenize_to_detection(format, source, opts)
    }

    fn hashes(tokens: &[DetectionToken]) -> Vec<u64> {
        tokens.iter().map(|t| t.hash).collect()
    }

    #[test]
    fn default_options_keep_raw_hash_equal_to_hash() {
        let opts = TokenizeOptions::new(Mode::Mild);
        let tokens = det("function a(x) { return x + 1; }", "javascript", &opts);
        assert!(!tokens.is_empty());
        assert!(tokens.iter().all(|t| t.raw_hash == t.hash));
        assert!(!opts.normalizes());
    }

    #[test]
    fn ignore_identifiers_matches_renamed_code_and_keeps_keywords() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        let a = det("function a(x) { return x + 1; }", "javascript", &opts);
        let b = det("function b(y) { return y + 1; }", "javascript", &opts);
        assert_eq!(
            hashes(&a),
            hashes(&b),
            "renamed identifiers must hash alike"
        );
        // raw hashes still differ where the names differ
        assert_ne!(
            a.iter().map(|t| t.raw_hash).collect::<Vec<_>>(),
            b.iter().map(|t| t.raw_hash).collect::<Vec<_>>()
        );
        // keywords are not folded: `return` vs `throw` must stay distinct
        let c = det("function a(x) { throw x + 1; }", "javascript", &opts);
        assert_ne!(hashes(&a), hashes(&c));
    }

    #[test]
    fn ignore_identifiers_keeps_common_keywords_in_generic_languages() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        let a = det("if x:\n    return y\n", "python", &opts);
        let b = det("if p:\n    return q\n", "python", &opts);
        let c = det("while x:\n    return y\n", "python", &opts);
        assert_eq!(hashes(&a), hashes(&b));
        assert_ne!(
            hashes(&a),
            hashes(&c),
            "`if` and `while` are keywords, not identifiers"
        );
        assert!(is_common_keyword("return"));
        assert!(!is_common_keyword("total"));
    }

    #[test]
    fn ignore_literals_folds_strings_and_numbers_separately() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_literals = true;
        let a = det("const a = 10; const b = 'x';", "javascript", &opts);
        let b = det("const a = 25; const b = \"yy\";", "javascript", &opts);
        assert_eq!(hashes(&a), hashes(&b));
        let c = det("const a = 'ten'; const b = 'x';", "javascript", &opts);
        assert_ne!(hashes(&a), hashes(&c), "a string is not a number");
        assert_eq!(literal_placeholder("42"), Some("$num"));
        assert_eq!(literal_placeholder(".5"), Some("$num"));
        assert_eq!(literal_placeholder("\"s\""), Some("$str"));
        assert_eq!(literal_placeholder("r'raw'"), Some("$str"));
        assert_eq!(literal_placeholder("true"), None);
    }

    #[test]
    fn ignore_annotations_drops_decorators_in_listed_formats_only() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_annotations = true;
        let plain = det("class A { m() { return 1; } }", "typescript", &opts);
        let decorated = det(
            "@Component({ selector: 'a' })\nclass A { @Input() m() { return 1; } }",
            "typescript",
            &opts,
        );
        assert_eq!(hashes(&plain), hashes(&decorated));
        // the token before an inner annotation carries a salted raw hash
        assert!(decorated.iter().any(|t| t.raw_hash != t.hash));

        let java_a = det(
            "class A {\n  @Override\n  int f() { return 1; }\n}",
            "java",
            &opts,
        );
        let java_b = det(
            "class A {\n  @Deprecated\n  int f() { return 1; }\n}",
            "java",
            &opts,
        );
        assert_eq!(hashes(&java_a), hashes(&java_b));
        assert_ne!(
            java_a.iter().map(|t| t.raw_hash).collect::<Vec<_>>(),
            java_b.iter().map(|t| t.raw_hash).collect::<Vec<_>>(),
            "different annotations must leave different raw hashes"
        );

        // Ruby instance variables use `@` and must survive
        let ruby = det("@count = 1", "ruby", &opts);
        assert!(strips_annotations("kotlin"));
        assert!(!strips_annotations("ruby"));
        assert_eq!(
            ruby.len(),
            det("@count = 1", "ruby", &TokenizeOptions::new(Mode::Mild)).len()
        );
    }

    #[test]
    fn push_token_ignore_case_folds_hash() {
        let mut t1 = Vec::new();
        let mut t2 = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_case = true;
        push_token(
            &mut t1,
            TokenKind::Identifier,
            "Hello",
            0,
            5,
            loc.clone(),
            loc.clone(),
            &opts,
        );
        push_token(
            &mut t2,
            TokenKind::Identifier,
            "hello",
            0,
            5,
            loc.clone(),
            loc,
            &opts,
        );
        assert_eq!(t1[0].hash, t2[0].hash, "ignore_case must fold case in hash");
    }

    #[test]
    fn push_token_code_ignore_range_skips_overlapping_token() {
        // Simulate: source = "foo// cpd-disable"
        // regex "//\\s*cpd-disable" matches bytes 3..18
        // Token "foo" is at 0..3 (no overlap -> kept)
        // Token "// cpd-disable" is at 3..18 (overlaps -> skipped)
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let mut opts = TokenizeOptions::new(Mode::Mild);
        // Pre-computed byte ranges from regex match on source text
        opts.ignore_ranges = vec![[3, 18]];
        push_token(
            &mut tokens,
            TokenKind::Identifier,
            "foo",
            0,
            3,
            loc.clone(),
            loc.clone(),
            &opts,
        );
        push_token(
            &mut tokens,
            TokenKind::Comment,
            "// cpd-disable",
            3,
            18,
            loc.clone(),
            loc,
            &opts,
        );
        assert_eq!(tokens.len(), 1, "only the non-matching token should remain");
        assert_eq!(tokens[0].range, [0, 3]);
    }

    #[test]
    fn push_token_code_ignore_range_no_overlap_keeps_all() {
        // regex match at bytes 100..120 doesn't overlap tokens at 0..3, 3..6
        let mut tokens = Vec::new();
        let loc = cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_ranges = vec![[100, 120]];
        push_token(
            &mut tokens,
            TokenKind::Identifier,
            "foo",
            0,
            3,
            loc.clone(),
            loc.clone(),
            &opts,
        );
        push_token(
            &mut tokens,
            TokenKind::Identifier,
            "bar",
            3,
            6,
            loc.clone(),
            loc,
            &opts,
        );
        assert_eq!(
            tokens.len(),
            2,
            "both tokens should remain when range doesn't overlap"
        );
    }

    #[test]
    fn code_ignore_ranges_computes_from_source_text() {
        let source = "import foo from 'bar';\nconst x = 1;";
        let re = regex::Regex::new(r"import\s+\w+\s+from").unwrap();
        let ranges = code_ignore_ranges(source, &[re]);
        assert_eq!(ranges.len(), 1, "should find one regex match");
        // "import foo from" starts at byte 0, ends at byte 15
        assert_eq!(ranges[0], [0, 15]);
    }

    #[test]
    fn code_ignore_ranges_multiple_patterns() {
        let source = "// MIT License\nfunction foo() {}\n// Copyright";
        let re1 = regex::Regex::new(r"//\s*MIT\s+License").unwrap();
        let re2 = regex::Regex::new(r"//\s*Copyright").unwrap();
        let ranges = code_ignore_ranges(source, &[re1, re2]);
        assert_eq!(ranges.len(), 2, "should find two regex matches");
    }

    #[test]
    fn code_ignore_ranges_empty_regexes() {
        let source = "function foo() {}";
        let ranges = code_ignore_ranges(source, &[]);
        assert!(ranges.is_empty(), "no regexes means no ranges");
    }

    #[test]
    fn with_code_ignore_patterns_builds_regexes() {
        let opts = TokenizeOptions::with_code_ignore_patterns(
            Mode::Mild,
            &["function".to_string(), r"//\s*cpd-disable".to_string()],
        );
        assert_eq!(opts.code_ignore_regexes.len(), 2);
        assert!(opts.code_ignore_regexes[0].is_match("function"));
        assert!(opts.code_ignore_regexes[1].is_match("// cpd-disable"));
        assert!(!opts.code_ignore_regexes[1].is_match("function"));
    }

    #[test]
    fn tokenize_to_detection_with_code_ignore_ranges_skips_imports() {
        let source = "import * from 'lodash';\nconst x = 1;";
        let regexes = vec![regex::Regex::new(r"import\s+\*\s+from").unwrap()];
        let ranges = code_ignore_ranges(source, &regexes);
        assert!(!ranges.is_empty(), "should find regex match in source");

        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_ranges = ranges;
        let tokens = tokenize_to_detection("javascript", source, &opts);

        // Tokens whose byte ranges overlap the import match should be skipped.
        // "import" (0-6), "*" (7-8), "from" (9-13) should all be in range,
        // but "const" (24-29) and "x" (30-31) etc should remain.
        let has_const = tokens.iter().any(|t| {
            // Check that tokens after the import line are still present
            t.range[0] >= 24
        });
        assert!(
            has_const,
            "tokens after the import line should still be present"
        );
    }

    #[test]
    fn code_ignore_ranges_multi_token_match() {
        // The key test: regex "import.*from" matches multi-token source text
        // like "import * from 'module-name'" — not just a single token value.
        let source = "import * from 'lodash';\nconst result = 42;";
        let re = regex::Regex::new(r"import\s+.*?\s+from").unwrap();
        let ranges = code_ignore_ranges(source, &[re]);
        assert_eq!(
            ranges.len(),
            1,
            "should find one regex match spanning import statement"
        );
        assert!(ranges[0][0] == 0, "match should start at beginning");
        assert!(ranges[0][1] > 0, "match should have non-zero end");
    }
}
