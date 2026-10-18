use std::str::FromStr;

use cpd_core::hash::hash_token;
use cpd_core::models::{DetectionToken, MEMBER_NAME, Token, TokenKind};

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
    tokens_to_detection(raw, options, NameRules::of(format))
}

/// [`tokenize_format_to_detection`] for the block of a host file that
/// starts at byte `offset`, such as a Markdown fence or a component's
/// script: the host's `ignore_ranges` apply at the block's own offsets.
pub fn tokenize_block_to_detection(
    format: &str,
    block: &str,
    offset: usize,
    options: &TokenizeOptions,
) -> Vec<DetectionToken> {
    if options.ignore_ranges.is_empty() {
        return tokenize_format_to_detection(format, block, options);
    }
    // The expressions are spent: their ranges are all a block needs.
    let local = TokenizeOptions {
        mode: options.mode,
        ignore_case: options.ignore_case,
        ignore_ranges: ranges_in(&options.ignore_ranges, offset..offset + block.len()),
        ignore_identifiers: options.ignore_identifiers,
        ignore_literals: options.ignore_literals,
        ignore_annotations: options.ignore_annotations,
        code_ignore_regexes: Vec::new(),
        strip_types_formats: options.strip_types_formats.clone(),
    };
    tokenize_format_to_detection(format, block, &local)
}

/// The parts of a host file's byte ranges `ranges` that fall in its block
/// `block`, by the block's own offsets.
pub fn ranges_in(ranges: &[[usize; 2]], block: std::ops::Range<usize>) -> Vec<[usize; 2]> {
    ranges
        .iter()
        .filter(|[start, end]| *start < block.end && *end > block.start)
        .map(|[start, end]| {
            [
                (*start).max(block.start) - block.start,
                (*end).min(block.end) - block.start,
            ]
        })
        .collect()
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
    fold_names: bool,
    member: bool,
) -> bool {
    // Drop Ignore-marked tokens in all modes.
    if kind == TokenKind::Ignore {
        return false;
    }
    // Drop tokens in Ignore byte ranges.
    // This covers both jscpd:ignore-start/end markers and code-level ignorePattern
    // regex ranges (which are computed from source text before tokenization).
    if options
        .ignore_ranges
        .iter()
        .any(|[rs, re]| byte_start < *re && byte_end > *rs)
    {
        return false;
    }
    // Mode-based filtering:
    match options.mode {
        Mode::Mild => {
            if kind == TokenKind::Whitespace {
                return false;
            }
        }
        Mode::Weak => {
            if matches!(
                kind,
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment
            ) {
                return false;
            }
        }
        Mode::Strict => {} // keep everything (except Ignore, handled above)
    }
    let raw_hash = hash_token(kind.discriminant(), value, options.ignore_case);
    let (hash, name) = match normalized_value(&kind, value, options) {
        Some(placeholder) => {
            // The language decides whether a name keeps its case, whatever
            // --ignore-case says, and a member is another name than a local
            // of its text; 0 marks the tokens that are not renamed
            // identifiers.
            let name = match placeholder {
                RENAMED_IDENTIFIER => {
                    let name = match fold_names == options.ignore_case {
                        true => raw_hash,
                        false => hash_token(kind.discriminant(), value, fold_names),
                    };
                    match member {
                        true => name | MEMBER_NAME,
                        false => (name & !MEMBER_NAME).max(1),
                    }
                }
                _ => 0,
            };
            (hash_token(kind.discriminant(), placeholder, false), name)
        }
        None => (raw_hash, 0),
    };
    tokens.push(DetectionToken {
        hash,
        raw_hash,
        name,
        start,
        end,
        range: [byte_start, byte_end],
    });
    true
}

/// Whether the names of `format` ignore case, so that `Customer` and
/// `customer` are one name to consistent matching.
pub fn names_ignore_case(format: &str) -> bool {
    matches!(
        format,
        "abap"
            | "ada"
            | "apex"
            | "autohotkey"
            | "autoit"
            | "basic"
            | "batch"
            | "cfml"
            | "cfscript"
            | "cobol"
            | "excel-formula"
            | "fortran"
            | "ichigojam"
            | "pascal"
            | "plsql"
            | "powershell"
            | "sas"
            | "sql"
            | "vbnet"
            | "vhdl"
            | "visual-basic"
            | "xojo"
    )
}

/// How the names of a format compare when `--ignore-identifiers` matches
/// them consistently (see [`DetectionToken::name`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NameRules {
    /// `Customer` and `customer` are one name ([`names_ignore_case`]).
    pub fold_case: bool,
    /// A name after `.` is a member, another name than a local of the same
    /// text: `this.url = url` copied as `this.url = link` pairs each name
    /// with one name.
    pub members: bool,
    /// A name after `->` is a member, as in C, C++ and PHP.
    pub arrows: bool,
}

impl NameRules {
    /// The rules of `format`: `.` reaches a member in the languages of
    /// [`DOT_MEMBERS`], `->` in those of [`ARROW_MEMBERS`]. In other formats
    /// a name after them is a name like any other: PHP and Perl join
    /// strings with `.`, Fortran compares with `.gt.`, Haskell composes
    /// functions, and prose ends a sentence.
    pub fn of(format: &str) -> Self {
        Self {
            fold_case: names_ignore_case(format),
            members: DOT_MEMBERS.contains(&format),
            arrows: ARROW_MEMBERS.contains(&format),
        }
    }
}

/// The formats where `.` reaches a member of what comes before it.
const DOT_MEMBERS: &[&str] = &[
    "actionscript",
    "apex",
    "arduino",
    "bison",
    "c",
    "c-header",
    "cfscript",
    "clike",
    "coffeescript",
    "cpp",
    "cpp-header",
    "crystal",
    "csharp",
    "d",
    "dart",
    "flow",
    "fsharp",
    "gdscript",
    "glsl",
    "go",
    "groovy",
    "haxe",
    "hlsl",
    "java",
    "javascript",
    "jsx",
    "julia",
    "kotlin",
    "livescript",
    "lua",
    "n4js",
    "nim",
    "objectivec",
    "ocaml",
    "odin",
    "opencl",
    "powershell",
    "processing",
    "python",
    "reason",
    "rescript",
    "ruby",
    "rust",
    "scala",
    "solidity",
    "swift",
    "tsx",
    "typescript",
    "unrealscript",
    "vala",
    "vbnet",
    "visual-basic",
    "wgsl",
    "xojo",
    "zig",
];

/// The formats where `->` reaches a member of what comes before it.
const ARROW_MEMBERS: &[&str] = &[
    "arduino",
    "bison",
    "c",
    "c-header",
    "clike",
    "cpp",
    "cpp-header",
    "objectivec",
    "opencl",
    "perl",
    "php",
];

/// Whether each name of a token stream is reached as a member, by the
/// [`NameRules`] of its format: after a single `.` or `?.` (most tokenizers
/// give `..` and `...`, ranges, spreads, varargs and Dart's cascades, as one
/// dot after another), after the `.` that ends a number such as Rust's
/// `x.0.url`, or after `->` given as `-` and `>`, but not `-->`. The tokens
/// it is shown are the ones detection keeps, without comments.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MemberAccess {
    rules: NameRules,
    /// The last three tokens shown, oldest first: what each is and where it
    /// starts and ends.
    last: [(Access, usize, usize); 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Dot,
    /// A number that a single dot ends, as `0.` in `x.0.url`.
    NumberDot,
    Minus,
    Greater,
    Other,
}

impl MemberAccess {
    pub(crate) fn new(rules: NameRules) -> Self {
        Self {
            rules,
            last: [(Access::Other, 0, 0); 3],
        }
    }

    /// Whether the rules can make any name a member's.
    pub(crate) fn tracks(&self) -> bool {
        self.rules.members || self.rules.arrows
    }

    /// Whether a name that comes next is a member's.
    pub(crate) fn next_is_member(&self) -> bool {
        let [
            (oldest, _, oldest_end),
            (older, older_start, older_end),
            (newer, newer_start, _),
        ] = self.last;
        match newer {
            Access::Dot => {
                self.rules.members && !(older == Access::Dot && older_end == newer_start)
            }
            Access::NumberDot => self.rules.members,
            Access::Greater => {
                self.rules.arrows
                    && older == Access::Minus
                    && older_end == newer_start
                    && !(oldest == Access::Minus && oldest_end == older_start)
            }
            Access::Minus | Access::Other => false,
        }
    }

    /// Takes in the token `value` at `start..end`.
    pub(crate) fn see(&mut self, value: &str, start: usize, end: usize) {
        let access = match value {
            "." | "?." => Access::Dot,
            "-" => Access::Minus,
            ">" => Access::Greater,
            _ if value.ends_with('.')
                && !value.ends_with("..")
                && value.starts_with(|c: char| c.is_ascii_digit()) =>
            {
                Access::NumberDot
            }
            _ => Access::Other,
        };
        self.last = [self.last[1], self.last[2], (access, start, end)];
    }
}

/// The detection tokens of `raw`, the tokens of a source whose names compare
/// by `rules`. A token that `annotation` marks is a dropped annotation: its
/// text goes into the raw hash of the token before it. A clone whose
/// matched run contains that token then classifies as `renamed` when the
/// annotations differ, while a run that starts after the annotation is
/// unaffected and stays `exact`: the reported fragment text really is
/// identical there.
pub(crate) fn detection_tokens(
    raw: Vec<Token>,
    options: &TokenizeOptions,
    rules: NameRules,
    annotation: &[bool],
) -> Vec<DetectionToken> {
    let mut members = MemberAccess::new(rules);
    let track = options.ignore_identifiers && members.tracks();
    let mut detection: Vec<DetectionToken> = Vec::with_capacity(raw.len());
    for (i, t) in raw.into_iter().enumerate() {
        if annotation.get(i).copied().unwrap_or(false) {
            if let Some(prev) = detection.last_mut() {
                let h = hash_token(t.kind.discriminant(), &t.value, false);
                prev.raw_hash = prev.raw_hash.rotate_left(7) ^ h;
            }
            continue;
        }
        let byte_start = t.start.offset as usize;
        let byte_end = t.end.offset as usize;
        let comment = matches!(t.kind, TokenKind::Comment | TokenKind::BlockComment);
        let member = track && members.next_is_member();
        let kept = push_token(
            &mut detection,
            t.kind,
            &t.value,
            byte_start,
            byte_end,
            t.start,
            t.end,
            options,
            rules.fold_case,
            member,
        );
        if track && kept && !comment {
            members.see(&t.value, byte_start, byte_end);
        }
    }
    detection
}

/// The placeholder every identifier hashes as under `--ignore-identifiers`.
const RENAMED_IDENTIFIER: &str = "$id";

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
                Some(RENAMED_IDENTIFIER)
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
        // `@` followed by an identifier — but never by a keyword: Java's
        // `@interface Name { … }` declares an annotation type rather than
        // applying one, and the generic tokenizer reports `interface` as an
        // identifier.
        let starts_annotation = tokens[i].value == "@"
            && tokens[i].kind != TokenKind::Ignore
            && tokens
                .get(i + 1)
                .is_some_and(|t| t.kind == TokenKind::Identifier && !is_common_keyword(&t.value));
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
    detection_tokens(raw, options, NameRules::of(format), &annotation)
}

/// The byte ranges of `source` whose tokens the tokenizer of `format` drops
/// for `jscpd:ignore-start` and `jscpd:ignore-end` markers, sorted and
/// disjoint, so the code inside a range is left out of detection. The
/// languages read the markers as their tokenizers do, without tokenizing
/// the source again. Empty without a marker.
pub fn ignored_ranges(format: &str, source: &str) -> Vec<[usize; 2]> {
    if !source.contains("jscpd:ignore") {
        return Vec::new();
    }
    let ranges = match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            crate::javascript::find_ignore_ranges(source)
        }
        "markdown" | "md" => crate::markdown::collect_ignore_byte_ranges(source),
        "vue" | "svelte" | "astro" | "razor" => ignored_tokens(format, source),
        _ => crate::generic::ignore_line_ranges(source),
    };
    merge_ranges(ranges)
}

/// The byte ranges of the tokens the tokenizer of a container format marks
/// as ignored, one per run of them: blanks and comments between ignored
/// tokens keep a run whole.
fn ignored_tokens(format: &str, source: &str) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    let mut open: Option<[usize; 2]> = None;
    for token in dispatch_tokenizer(format, source, Mode::Mild) {
        match token.kind {
            TokenKind::Ignore => {
                let (start, end) = (token.start.offset as usize, token.end.offset as usize);
                match &mut open {
                    Some(range) => range[1] = end,
                    None => open = Some([start, end]),
                }
            }
            TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment => {}
            _ => ranges.extend(open.take()),
        }
    }
    ranges.extend(open);
    ranges
}

/// `ranges` sorted and merged where they overlap or touch, without the
/// empty ones.
pub fn merge_ranges(mut ranges: Vec<[usize; 2]>) -> Vec<[usize; 2]> {
    ranges.retain(|[start, end]| start < end);
    ranges.sort_unstable();
    let mut merged: Vec<[usize; 2]> = Vec::with_capacity(ranges.len());
    for [start, end] in ranges {
        match merged.last_mut() {
            Some(last) if start <= last[1] => last[1] = last[1].max(end),
            _ => merged.push([start, end]),
        }
    }
    merged
}

/// The `--ignore-pattern` expressions that compile; the others are
/// skipped, as a warning says when the run starts.
pub fn compile_ignore_patterns(patterns: &[String]) -> Vec<regex::Regex> {
    patterns
        .iter()
        .filter_map(|pattern| regex::Regex::new(pattern).ok())
        .collect()
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
    fn ignore_patterns_drop_the_code_they_match_inside_a_markdown_fence() {
        let md = "# Guide\n\nSome prose first.\n\n```js\nfunction total(items) {\n  let sum = 0;\n  console.log(\"start\");\n  for (const item of items) sum += item.price;\n  return sum;\n}\n```\n";
        let re = regex::Regex::new(r"console\.log\(.*\);").unwrap();
        let options = TokenizeOptions {
            ignore_ranges: code_ignore_ranges(md, &[re]),
            ..TokenizeOptions::new(Mode::Mild)
        };
        let maps = tokenize_to_detection_maps("markdown", md, &options);
        let js = maps.iter().find(|map| map.format == "javascript").unwrap();
        let texts: Vec<&str> = js
            .tokens
            .iter()
            .map(|t| &md[t.range[0]..t.range[1]])
            .collect();
        assert!(!texts.contains(&"console"), "{texts:?}");
        assert!(texts.contains(&"return"), "{texts:?}");
        assert_eq!(
            ranges_in(&[[2, 8], [20, 30]], 5..25),
            vec![[0, 3], [15, 20]]
        );
    }

    #[test]
    fn ignored_ranges_hold_what_the_tokenizers_drop_and_nothing_else() {
        let sources = [
            (
                "python",
                "a = 1\n# jscpd:ignore-start\nb = 2\n# jscpd:ignore-end\nc = 3\n",
            ),
            ("python", "a = 1\nb = close(a)  # jscpd:ignore-end\nc = 3\n"),
            ("python", "x = 1  # jscpd:ignore-start\ny = 2\n"),
            (
                "python",
                "a = 1\n# jscpd:ignore-start jscpd:ignore-end\nb = 2\n",
            ),
            (
                "python",
                "a = 1\r\n# jscpd:ignore-start\r\nb = 2\r\n# jscpd:ignore-end\r\nc = 3\r\n",
            ),
            (
                "javascript",
                "let a = 1;\n// jscpd:ignore-start\nlet b = 2;\n/* jscpd:ignore-end */ let c = 3;\n",
            ),
            (
                "javascript",
                "let a = 1;\n// jscpd:ignore-start\nlet b = 2;\n",
            ),
            (
                "typescript",
                "const a = 1; /* jscpd:ignore-start */ const b = 2; /* jscpd:ignore-end */ const c = 3;\n",
            ),
        ];
        for (format, source) in sources {
            let ranges = ignored_ranges(format, source);
            let all = tokenize(format, source, Mode::Strict);
            let kept = dispatch_tokenizer(format, source, Mode::Strict);
            for token in &kept {
                let (start, end) = (token.start.offset as usize, token.end.offset as usize);
                let inside = ranges.iter().any(|[s, e]| *s <= start && end <= *e);
                let overlaps = ranges.iter().any(|[s, e]| start < *e && end > *s);
                match token.kind {
                    TokenKind::Ignore => {
                        assert!(inside, "{format} {source:?}: {token:?} in {ranges:?}")
                    }
                    TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment => {}
                    _ => assert!(!overlaps, "{format} {source:?}: {token:?} in {ranges:?}"),
                }
            }
            // A token the tokenizer drops without a trace lies in a range too.
            for (at, _) in source.match_indices("close") {
                let dropped = !all.iter().any(|t| t.start.offset as usize == at);
                if dropped {
                    assert!(
                        ranges.iter().any(|[s, e]| *s <= at && at < *e),
                        "{source:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ignore_patterns_drop_the_code_they_match_in_razor_and_vue_blocks() {
        let pattern = regex::Regex::new(r"Log\.Debug\([^)]*\);").unwrap();
        let razor = "<h1>@Model.Title</h1>\n<p>Totals</p>\n@{\n    var total = Model.Lines.Sum(l => l.Price);\n    Log.Debug(\"total computed\");\n    ViewData[\"Total\"] = total;\n}\n";
        let options = TokenizeOptions {
            ignore_ranges: code_ignore_ranges(razor, std::slice::from_ref(&pattern)),
            ..TokenizeOptions::new(Mode::Mild)
        };
        let texts: Vec<&str> = tokenize_to_detection("razor", razor, &options)
            .iter()
            .map(|t| &razor[t.range[0]..t.range[1]])
            .collect();
        assert!(!texts.contains(&"Log"), "{texts:?}");
        assert!(texts.contains(&"ViewData"), "{texts:?}");

        let md = "# Guide\n\nA component:\n\n```vue\n<template><p>{{ total }}</p></template>\n<script>\nexport default {\n  computed: {\n    total() {\n      Log.Debug(\"total\");\n      return this.lines.length;\n    },\n  },\n};\n</script>\n```\n";
        let options = TokenizeOptions {
            ignore_ranges: code_ignore_ranges(md, std::slice::from_ref(&pattern)),
            ..TokenizeOptions::new(Mode::Mild)
        };
        let maps = tokenize_to_detection_maps("markdown", md, &options);
        let texts: Vec<&str> = maps
            .iter()
            .filter(|map| map.format != "markdown")
            .flat_map(|map| map.tokens.iter())
            .map(|t| &md[t.range[0]..t.range[1]])
            .collect();
        assert!(!texts.contains(&"Log"), "{texts:?}");
        assert!(texts.contains(&"return"), "{texts:?}");
    }

    #[test]
    fn ignored_ranges_cover_the_code_between_the_markers() {
        let python = "def f(x):\n    y = x\n    # jscpd:ignore-start\n    for i in x:\n        # retry\n        y += i\n    # jscpd:ignore-end\n    return y\n";
        let ranges = ignored_ranges("python", python);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        let [start, end] = ranges[0];
        let ignored = &python[start..end];
        assert!(ignored.contains("for i in x:"), "{ignored:?}");
        assert!(ignored.contains("y += i"), "{ignored:?}");
        assert!(!ignored.contains("return"), "{ignored:?}");

        let js = "function f(x) {\n  let y = x;\n  // jscpd:ignore-start\n  for (const i of x) { y += i; }\n  // jscpd:ignore-end\n  return y;\n}\n";
        let ranges = ignored_ranges("javascript", js);
        assert_eq!(ranges.len(), 1, "{ranges:?}");
        let ignored = &js[ranges[0][0]..ranges[0][1]];
        assert!(
            ignored.contains("for (const i of x) { y += i; }"),
            "{ignored:?}"
        );
        assert!(!ignored.contains("return"), "{ignored:?}");

        assert!(ignored_ranges("python", "x = 1\n").is_empty());
    }

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

    /// The text and the name of every token of `code` that has a name.
    fn names<'a>(code: &'a str, format: &str, opts: &TokenizeOptions) -> Vec<(&'a str, u64)> {
        det(code, format, opts)
            .iter()
            .filter(|t| t.name != 0)
            .map(|t| (&code[t.range[0]..t.range[1]], t.name))
            .collect()
    }

    #[test]
    fn renamed_identifiers_keep_their_names() {
        // Consistent matching (issue #1217) pairs the names of two clones.
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        for (code, format) in [("a = b + a", "python"), ("let a = b + a;", "javascript")] {
            let names = names(code, format, &opts);
            let texts: Vec<&str> = names.iter().map(|(text, _)| *text).collect();
            assert_eq!(texts, ["a", "b", "a"], "{format}: only identifiers");
            assert_eq!(names[0].1, names[2].1, "{format}");
            assert_ne!(names[0].1, names[1].1, "{format}");
        }
        opts.ignore_identifiers = false;
        assert!(names("a = b + a", "python", &opts).is_empty());
    }

    #[test]
    fn a_dropped_annotation_leaves_the_name_before_it_alone() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        opts.ignore_annotations = true;
        let code = "def f(self):\n    return result\n@staticmethod\ndef g():\n    return result\n";
        let tokens = det(code, "python", &opts);
        let results: Vec<_> = tokens
            .iter()
            .filter(|t| &code[t.range[0]..t.range[1]] == "result")
            .collect();
        assert_eq!(results.len(), 2);
        // The decorator salts the raw hash of the `result` before it.
        assert_ne!(results[0].raw_hash, results[1].raw_hash);
        assert_eq!(results[0].name, results[1].name);
    }

    #[test]
    fn names_ignore_case_where_the_language_does() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        let sql = "SELECT Customer.Name FROM Customer WHERE customer.City = 1";
        let customer: Vec<u64> = names(sql, "sql", &opts)
            .into_iter()
            .filter(|(text, _)| text.eq_ignore_ascii_case("customer"))
            .map(|(_, name)| name)
            .collect();
        assert_eq!(customer.len(), 3, "{customer:?}");
        assert!(customer.iter().all(|&name| name == customer[0]));
        let js = names("Customer = customer;", "javascript", &opts);
        assert_ne!(js[0].1, js[1].1, "JavaScript names keep their case");
        // --ignore-case compares tokens; it does not make two names one.
        opts.ignore_case = true;
        let js = names("Customer = customer;", "javascript", &opts);
        assert_ne!(js[0].1, js[1].1, "with --ignore-case too");
    }

    #[test]
    fn a_member_is_another_name_than_the_local_it_spells() {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.ignore_identifiers = true;
        let urls = |code: &str, format: &str| -> Vec<u64> {
            names(code, format, &opts)
                .into_iter()
                .filter(|(text, _)| *text == "url")
                .map(|(_, name)| name)
                .collect()
        };
        // A member, the local, and a member again.
        for (format, code) in [
            ("javascript", "this.url = url; a?.url;"),
            ("python", "self.url = url\nother.url\n"),
            ("java", "this.url = url; that.url = 1;"),
            ("kotlin", "this.url = url; a?.url"),
            ("go", "s.url = url; t.url = 1"),
            ("c", "p->url = url; q.url = 1;"),
            ("arduino", "p->url = url; q.url = 1;"),
            ("php", "<?php $this->url = $url; $o->url;"),
            ("rust", "s.0.url = url; t.url = 1;"),
        ] {
            let urls = urls(code, format);
            assert_eq!(urls.len(), 3, "{format}");
            assert_ne!(urls[0], urls[1], "{format}");
            assert_eq!(urls[0], urls[2], "{format}");
        }
        // Ranges, spreads, varargs, a Rust return type and prose.
        for (format, code) in [
            ("rust", "let r = url..url;"),
            ("ruby", "r = (url...url)"),
            ("javascript", "f(...url, url);"),
            ("java", "void f(String... url) { g(url); }"),
            ("rust", "fn f(url: u8) -> url { url }"),
            ("markdown", "Read it. url\nurl"),
            ("html", "<p>Read it. url url</p>"),
            // `.` joins strings in PHP and compares in Fortran.
            ("php", "<?php $a = $b . url($c) . url($d);"),
            ("fortran", "if (a .gt. url) url = 1"),
            // `i-- > url` written without spaces.
            ("c", "while (i-->url) url++;"),
        ] {
            let urls = urls(code, format);
            assert!(urls.len() >= 2, "{format}: {urls:?}");
            assert!(urls.iter().all(|&name| name == urls[0]), "{format}: {code}");
        }
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
        // A leading-dot number, a prefixed string and a keyword literal.
        let d = det("const a = .5; const b = 'x';", "javascript", &opts);
        assert_eq!(hashes(&a), hashes(&d), ".5 is a number too");
        let raw_a = det("p = r'one'\n", "python", &opts);
        let raw_b = det("p = r\"two\"\n", "python", &opts);
        assert_eq!(hashes(&raw_a), hashes(&raw_b), "a prefixed string folds");
        let t = det("const a = true;", "javascript", &opts);
        let f = det("const a = false;", "javascript", &opts);
        assert_ne!(hashes(&t), hashes(&f), "true and false are not folded");
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

        // `@interface` declares an annotation type; it is not an annotation use
        let decl = "public @interface Marker {\n  String value() default \"\";\n}";
        let stripped = det(decl, "java", &opts);
        let untouched = det(decl, "java", &TokenizeOptions::new(Mode::Mild));
        assert_eq!(hashes(&stripped), hashes(&untouched));

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
