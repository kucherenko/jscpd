use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Keyword,
    Identifier,
    Literal,
    Operator,
    Punctuation,
    Comment,
    BlockComment,
    Whitespace,
    Ignore,
    Other,
}

impl TokenKind {
    /// Return a stable byte discriminant for use in token hashing.
    pub fn discriminant(&self) -> u8 {
        match self {
            Self::Keyword => 1,
            Self::Identifier => 2,
            Self::Literal => 3,
            Self::Operator => 4,
            Self::Punctuation => 5,
            Self::Comment => 6,
            Self::BlockComment => 7,
            Self::Whitespace => 8,
            Self::Ignore => 9,
            Self::Other => 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub start: Location,
    pub end: Location,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlameEntry {
    pub commit_sha: String,
    pub author: String,
    pub timestamp: i64,
}

/// How the two fragments of a clone relate at the token level (issue #998).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloneKind {
    /// The fragments are token-for-token identical.
    #[default]
    Exact,
    /// The fragments match only after identifier, literal or annotation
    /// normalization (`--ignore-identifiers`, `--ignore-literals`,
    /// `--ignore-annotations`): a Type-2 clone.
    Renamed,
    /// A Type-3 near-miss clone: two or more matches of the same file pair
    /// merged across a gap of unmatched lines (`--max-gap-lines`), or a pair
    /// of structurally similar functions (`--similarity`). `similar` takes
    /// precedence over `renamed`: a merge of renamed halves is `similar`.
    Similar,
}

impl CloneKind {
    pub fn is_renamed(self) -> bool {
        matches!(self, CloneKind::Renamed)
    }

    pub fn is_similar(self) -> bool {
        matches!(self, CloneKind::Similar)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CloneKind::Exact => "exact",
            CloneKind::Renamed => "renamed",
            CloneKind::Similar => "similar",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fragment {
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    pub start: Location,
    pub end: Location,
    pub range: [u32; 2],
    pub blame: Option<BlameEntry>,
}

/// How a `similar` clone was produced (issue #999).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimilarityMethod {
    /// Exact matches merged across a gap of unmatched lines
    /// (`--max-gap-lines`); `similarity` is matched tokens over the span.
    Gap,
    /// Whole functions compared by syntax-tree structure (`--similarity`);
    /// `similarity` is the weighted Jaccard index of node-type shingles.
    Ast,
}

impl SimilarityMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            SimilarityMethod::Gap => "gap",
            SimilarityMethod::Ast => "ast",
        }
    }
}

/// One `--kind` value: a clone kind, or one of the two mechanisms that find
/// `similar` clones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    Exact,
    Renamed,
    /// Every `similar` clone, whichever mechanism found it.
    Similar,
    /// `similar` clones merged across a gap (`--max-gap-lines`).
    Gap,
    /// `similar` function pairs compared by syntax tree (`--similarity`).
    Ast,
}

impl KindFilter {
    pub const NAMES: &'static str = "exact, renamed, similar, gap, ast";

    pub fn as_str(self) -> &'static str {
        match self {
            KindFilter::Exact => "exact",
            KindFilter::Renamed => "renamed",
            KindFilter::Similar => "similar",
            KindFilter::Gap => "gap",
            KindFilter::Ast => "ast",
        }
    }

    pub fn matches(self, clone: &CpdClone) -> bool {
        match self {
            KindFilter::Exact => clone.kind == CloneKind::Exact,
            KindFilter::Renamed => clone.kind == CloneKind::Renamed,
            KindFilter::Similar => clone.kind == CloneKind::Similar,
            KindFilter::Gap => clone.similarity_method == Some(SimilarityMethod::Gap),
            KindFilter::Ast => clone.similarity_method == Some(SimilarityMethod::Ast),
        }
    }
}

impl std::str::FromStr for KindFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "exact" => Ok(KindFilter::Exact),
            "renamed" => Ok(KindFilter::Renamed),
            "similar" => Ok(KindFilter::Similar),
            "gap" => Ok(KindFilter::Gap),
            "ast" => Ok(KindFilter::Ast),
            other => Err(format!(
                "unknown clone kind '{other}': must be one of: {}",
                KindFilter::NAMES
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpdClone {
    pub format: String,
    pub fragment_a: Fragment,
    pub fragment_b: Fragment,
    pub token_count: u32,
    /// True when the clone is absent from the configured baseline (issue #944).
    /// Always false when no baseline is in use.
    #[serde(default)]
    pub is_new: bool,
    /// `exact` when the raw tokens of both fragments are identical, `renamed`
    /// when they match only after normalization (issue #998). Always `exact`
    /// when no normalization option is on.
    #[serde(default)]
    pub kind: CloneKind,
    /// For `similar` clones: matched tokens divided by the tokens of the
    /// longer merged span, in `(0, 1)`. `None` for exact and renamed clones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    /// Which mechanism produced a `similar` clone; the two scores are not
    /// on the same scale, so reporters show it next to the value.
    #[serde(default, rename = "method", skip_serializing_if = "Option::is_none")]
    pub similarity_method: Option<SimilarityMethod>,
    /// Lines inside each fragment's span that are not duplicated code, for
    /// `fragment_a` and `fragment_b` in that order. Two things land here: the
    /// lines a `--max-gap-lines` merge left unmatched between its halves, and,
    /// for an embedded block, the host language's lines lying between two
    /// blocks of the same fragment (issue #1090). Statistics subtract them
    /// from the fragment's span.
    #[serde(skip)]
    pub unmatched_lines: [u32; 2],
}

impl Location {
    pub fn new(line: u32, column: u32, offset: u32) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

impl Fragment {
    /// A fragment with no scan root and no blame data, the shape detection
    /// produces before enrichment.
    pub fn new(
        source_id: impl Into<String>,
        start: Location,
        end: Location,
        range: [u32; 2],
    ) -> Self {
        Self {
            source_id: source_id.into(),
            source_root: None,
            start,
            end,
            range,
            blame: None,
        }
    }

    pub fn with_blame(mut self, blame: BlameEntry) -> Self {
        self.blame = Some(blame);
        self
    }
}

impl CpdClone {
    /// Duplicated lines this clone adds to the statistics: the matched lines
    /// of its primary fragment.
    pub fn matched_lines(&self) -> u64 {
        self.fragment_lines(0)
    }

    /// Matched lines of one fragment — 0 is A, 1 is B. Per-file summaries need
    /// both, and they must not drift apart.
    ///
    /// The span is inclusive, so a clone of lines 10 through 19 is ten lines,
    /// the same count the reporters print next to it. Whatever the span covers
    /// but does not duplicate — gap-merge lines, host-language lines between
    /// two embedded blocks — is already in `unmatched_lines`.
    pub fn fragment_lines(&self, index: usize) -> u64 {
        let fragment = if index == 0 {
            &self.fragment_a
        } else {
            &self.fragment_b
        };
        let span = fragment.end.line.saturating_sub(fragment.start.line) + 1;
        span.saturating_sub(self.unmatched_lines[index]) as u64
    }

    /// An exact clone with no baseline, similarity or gap metadata.
    pub fn exact(
        format: impl Into<String>,
        fragment_a: Fragment,
        fragment_b: Fragment,
        token_count: u32,
    ) -> Self {
        Self {
            format: format.into(),
            fragment_a,
            fragment_b,
            token_count,
            is_new: false,
            kind: CloneKind::default(),
            similarity: None,
            similarity_method: None,
            unmatched_lines: [0, 0],
        }
    }
    /// `similarity` rounded to three decimals as an f64, the form reporters
    /// print (an f32 widened to JSON would print as `0.8510638475418091`).
    pub fn similarity_rounded(&self) -> Option<f64> {
        self.similarity
            .map(|s| (f64::from(s) * 1000.0).round() / 1000.0)
    }
}

/// Internal detection unit — no heap allocation per token.
///
/// Produced by the tokenizer's detection path at tokenize time.
/// `Token` is used for display, blame, and reporter output;
/// `DetectionToken` is used only during the clone detection hot path.
/// The token's value string is not stored — only its pre-computed hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionToken {
    /// Pre-computed hash of (kind, value) — detection never re-hashes.
    /// When a normalization option rewrote the value (`$id`, `$str`, `$num`)
    /// this is the hash of the placeholder.
    pub hash: u64,
    /// Hash of the original (kind, value). Equal to `hash` unless a
    /// normalization option applied; lets detection tell exact clones from
    /// renamed ones without re-tokenizing.
    pub raw_hash: u64,
    pub start: Location,
    pub end: Location,
    /// Byte range in the source content: `[start_byte, end_byte]`.
    pub range: [usize; 2],
}

/// A source file with pre-tokenized tokens, ready for clone detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    pub id: String,
    pub format: String,
    pub tokens: Vec<Token>,
    /// File size in bytes. Zero for synthetic sub-format sources
    /// (embedded code blocks) whose bytes are counted by the parent file.
    #[serde(default)]
    pub bytes: u64,
}

impl SourceFile {
    /// True for a synthetic sub-format source: one embedded language inside a
    /// host file, stored under `<path>:<format>` (issue #1090). Its tokens keep
    /// the host file's line numbers, so the lines between two blocks belong to
    /// the host and not to this format.
    pub fn is_embedded(&self) -> bool {
        self.id
            .strip_suffix(self.format.as_str())
            .and_then(|rest| rest.strip_suffix(':'))
            .is_some_and(|path| !path.is_empty())
    }

    /// Lines of this source that carry code of its own format. For an ordinary
    /// file that is the last line holding a token — near enough to the file's
    /// length, and what jscpd has always counted. For an embedded block it is
    /// only the lines the blocks themselves occupy.
    pub fn line_count(&self) -> u64 {
        if self.is_embedded() {
            covered_lines(self.tokens.iter().map(|t| (t.start.line, t.end.line))) as u64
        } else {
            self.tokens.iter().map(|t| t.start.line).max().unwrap_or(0) as u64
        }
    }
}

/// How many distinct lines a run of tokens sits on.
///
/// The tokens come in source order and one token may cover several lines, so
/// this sweeps once and never counts a line twice. For a contiguous run the
/// answer is the line span; for an embedded block it is much smaller, because
/// the host language's lines between two blocks hold no token of this source.
pub fn covered_lines(spans: impl IntoIterator<Item = (u32, u32)>) -> u32 {
    let mut covered = 0;
    // Lines are 1-based, so 0 reads as "nothing counted yet".
    let mut last = 0;
    for (start, end) in spans {
        let from = if start > last { start } else { last + 1 };
        if end >= from {
            covered += end - from + 1;
            last = end;
        }
    }
    covered
}

/// Per-format or total statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatRow {
    pub lines: u64,
    pub tokens: u64,
    pub sources: u64,
    pub clones: u64,
    pub duplicated_lines: u64,
    pub duplicated_tokens: u64,
    pub percentage: f64,
    pub percentage_tokens: f64,
    #[serde(default)]
    pub new_duplicated_lines: u64,
    #[serde(default)]
    pub new_clones: u64,
}

impl Default for StatRow {
    fn default() -> Self {
        Self {
            lines: 0,
            tokens: 0,
            sources: 0,
            clones: 0,
            duplicated_lines: 0,
            duplicated_tokens: 0,
            percentage: 0.0,
            percentage_tokens: 0.0,
            new_duplicated_lines: 0,
            new_clones: 0,
        }
    }
}

/// Aggregated detection statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub total: StatRow,
    pub formats: HashMap<String, StatRow>,
    pub detection_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn statistics_default_total_is_zero() {
        let stats = Statistics {
            total: StatRow::default(),
            formats: HashMap::new(),
            detection_date: "2026-01-01T00:00:00Z".to_string(),
        };
        assert_eq!(stats.total.clones, 0);
    }

    #[test]
    fn token_serializes_and_deserializes() {
        let token = Token {
            kind: TokenKind::Keyword,
            value: "function".to_string(),
            start: Location {
                line: 1,
                column: 0,
                offset: 0,
            },
            end: Location {
                line: 1,
                column: 8,
                offset: 8,
            },
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: Token = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }

    #[test]
    fn cpd_clone_serializes_with_blame() {
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let blame = BlameEntry {
            commit_sha: "abc123".to_string(),
            author: "Alice".to_string(),
            timestamp: 1700000000,
        };
        let frag = Fragment::new("a.js", loc.clone(), loc, [0, 10]).with_blame(blame);
        let clone = CpdClone::exact("javascript", frag.clone(), frag, 50);
        let json = serde_json::to_string(&clone).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("fragment_a"));
    }

    #[test]
    fn fragment_blame_none_serializes_as_null() {
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        let frag = Fragment {
            source_id: "b.js".to_string(),
            source_root: None,
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 5],
            blame: None,
        };
        let json = serde_json::to_string(&frag).unwrap();
        assert!(json.contains("\"blame\":null"));
    }

    #[test]
    fn covered_lines_counts_a_contiguous_run_once() {
        assert_eq!(covered_lines([(1, 1), (1, 1), (2, 2), (3, 3)]), 3);
        assert_eq!(covered_lines(std::iter::empty()), 0);
    }

    #[test]
    fn covered_lines_skips_the_gap_between_two_blocks() {
        // Lines 17-26 and 43-52: twenty lines of code, whatever sits between.
        let block = |from: u32, to: u32| (from..=to).map(|l| (l, l));
        assert_eq!(covered_lines(block(17, 26).chain(block(43, 52))), 20);
    }

    #[test]
    fn covered_lines_handles_a_token_spanning_several_lines() {
        // A template literal running from line 4 to line 9, then code after it.
        assert_eq!(covered_lines([(4, 9), (9, 9), (10, 10)]), 7);
    }

    #[test]
    fn an_embedded_source_is_recognised_by_its_id() {
        let source = |id: &str, format: &str| SourceFile {
            id: id.to_string(),
            format: format.to_string(),
            tokens: vec![],
            bytes: 0,
        };
        assert!(source("guide.md:typescript", "typescript").is_embedded());
        assert!(!source("app.ts", "typescript").is_embedded());
        // The host's own map is not embedded in anything.
        assert!(!source("guide.md", "markdown").is_embedded());
        // A file whose whole name is the format is still a file.
        assert!(!source("typescript", "typescript").is_embedded());
    }
}
