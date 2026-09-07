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

    pub fn is_exact(self) -> bool {
        matches!(self, CloneKind::Exact)
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
    /// Lines inside each fragment's span that the gap merge (`--max-gap-lines`)
    /// left unmatched, for `fragment_a` and `fragment_b` in that order.
    /// Statistics subtract them so gap lines do not count as duplicated.
    /// `[0, 0]` for every clone the merge pass did not produce.
    #[serde(skip)]
    pub unmatched_lines: [u32; 2],
}

impl CpdClone {
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
        let frag = Fragment {
            source_id: "a.js".to_string(),
            source_root: None,
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 10],
            blame: Some(blame),
        };
        let clone = CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 50,
            is_new: false,
            kind: Default::default(),
            similarity: None,
            similarity_method: None,
            unmatched_lines: [0, 0],
        };
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
}
