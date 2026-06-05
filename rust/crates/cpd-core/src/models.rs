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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fragment {
    pub source_id: String,
    pub start: Location,
    pub end: Location,
    pub range: [u32; 2],
    pub blame: Option<BlameEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpdClone {
    pub format: String,
    pub fragment_a: Fragment,
    pub fragment_b: Fragment,
    pub token_count: u32,
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
    pub hash: u64,
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
}

/// Per-format or total statistics row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatRow {
    pub lines: u64,
    pub tokens: u64,
    pub sources: u64,
    pub clones: u64,
    pub duplicated_lines: u64,
    pub duplicated_tokens: u64,
    pub percentage: f64,
    pub percentage_tokens: f64,
}

/// Aggregated detection statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
            total: StatRow {
                lines: 0,
                tokens: 0,
                sources: 0,
                clones: 0,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                percentage: 0.0,
                percentage_tokens: 0.0,
            },
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
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 5],
            blame: None,
        };
        let json = serde_json::to_string(&frag).unwrap();
        assert!(json.contains("\"blame\":null"));
    }
}
