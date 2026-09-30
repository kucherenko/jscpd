// rules.rs
// The rule id of each kind of clone. The SARIF and Code Climate reporters
// and the language server (`--lsp`) share them, so a finding carries one name
// in code scanning, in a merge request and in the editor.

use cpd_core::models::{CloneKind, CpdClone, SimilarityMethod};

/// Token-for-token copies.
pub const DUPLICATE: &str = "jscpd/duplicate-code";
/// Copies that match after identifier, literal or annotation normalization.
pub const RENAMED: &str = "jscpd/renamed-code";
/// Near-miss copies: exact matches merged across a gap of changed lines
/// (`--max-gap-lines`).
pub const SIMILAR: &str = "jscpd/similar-code";
/// Functions whose syntax trees have the same shape (`--similarity`).
pub const SIMILAR_FUNCTION: &str = "jscpd/similar-function";
/// Functions that do the same job, found by an embedding model (`--semantic`).
pub const SEMANTIC: &str = "jscpd/semantic-code";

/// The rule `clone` falls under. The two mechanisms behind the `similar`
/// kind find different things, so each has a rule of its own.
pub fn rule_id(clone: &CpdClone) -> &'static str {
    match clone.kind {
        CloneKind::Exact => DUPLICATE,
        CloneKind::Renamed => RENAMED,
        CloneKind::Similar if clone.similarity_method == Some(SimilarityMethod::Ast) => {
            SIMILAR_FUNCTION
        }
        CloneKind::Similar => SIMILAR,
        CloneKind::Semantic => SEMANTIC,
    }
}
