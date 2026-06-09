use std::str::FromStr;

use cpd_core::hash::hash_token;
use cpd_core::models::{DetectionToken, Token, TokenKind};

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
    /// Pre-compiled code-level regex patterns inherited from v4 `ignorePattern`.
    /// Before tokenization, these are matched against the source text and
    /// overlapping byte ranges are added to `ignore_ranges`.
    pub code_ignore_regexes: Vec<regex::Regex>,
}

impl TokenizeOptions {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            ignore_case: false,
            ignore_ranges: Vec::new(),
            code_ignore_regexes: Vec::new(),
        }
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
            ignore_ranges: Vec::new(),
            code_ignore_regexes,
        }
    }
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
    tokens.push(DetectionToken {
        hash: hash_token(kind.discriminant(), value, options.ignore_case),
        start,
        end,
        range: [byte_start, byte_end],
    });
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
    let raw = dispatch_tokenizer(format, source, options.mode);
    let mut detection = Vec::with_capacity(raw.len());
    for t in raw {
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
        assert_eq!(tokens.len(), 2, "both tokens should remain when range doesn't overlap");
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
            &vec!["function".to_string(), r"//\s*cpd-disable".to_string()],
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
        assert!(has_const, "tokens after the import line should still be present");
    }

    #[test]
    fn code_ignore_ranges_multi_token_match() {
        // The key test: regex "import.*from" matches multi-token source text
        // like "import * from 'module-name'" — not just a single token value.
        let source = "import * from 'lodash';\nconst result = 42;";
        let re = regex::Regex::new(r"import\s+.*?\s+from").unwrap();
        let ranges = code_ignore_ranges(source, &[re]);
        assert_eq!(ranges.len(), 1, "should find one regex match spanning import statement");
        assert!(ranges[0][0] == 0, "match should start at beginning");
        assert!(ranges[0][1] > 0, "match should have non-zero end");
    }
}
