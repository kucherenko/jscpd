//! Boundaries (issue #1225): rules in the config file that keep the code of
//! a module, a folder, a file or a method inside its boundary.
//!
//! ```json
//! {
//!   "boundaries": [
//!     {
//!       "name": "money",
//!       "code": ["backend/billing/**", "backend/loans/Interest.kt#(*Payments)"],
//!       "notIn": ["web/**", "mobile/**"],
//!       "allow": ["web/e2e/**"],
//!       "thresholds": { "minTokens": 30, "semanticThreshold": 0.35 }
//!     }
//!   ]
//! }
//! ```
//!
//! A clone run checks every rule ([`check`]): a copy of protected code in a
//! place the rule keeps it out of, whether copied, edited or written again
//! in another language, is a violation, and a violation fails the run.
//! [`rules`] reads the key; [`Checker`] finds the copies.

mod check;
mod rules;

pub use check::Checker;

use cpd_semantic::SemanticOptions;
use std::path::PathBuf;

/// The `boundaries` key of a config file, with what checking it needs.
#[derive(Debug, Clone)]
pub struct Setting {
    /// The rules as the file writes them. A clone run parses them, so a
    /// mistake in them stops that run, and only that run: `--mcp` and the
    /// other modes do not check the rules.
    pub raw: serde_json::Value,
    /// The folder the rules' relative paths start from: the config file's
    /// folder for a file named with `--config`, the working directory
    /// otherwise, as for every other path in the file.
    pub base: PathBuf,
    /// The model the rules compare functions with: the `semantic` settings
    /// of the run, whether `--semantic` is on or not.
    pub semantic: SemanticOptions,
}
