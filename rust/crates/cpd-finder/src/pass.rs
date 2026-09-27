//! Clone passes that look at whole files next to token detection.
//!
//! A pass sees every file it asks for while the finder holds the file in
//! memory, then runs once after the token passes, with the clones they
//! found. `--semantic` is one: it lives in the cpd-semantic crate with
//! everything it depends on, and the finder knows nothing about it beyond
//! this trait.

use cpd_core::detect::PathLabel;
use cpd_core::models::{CpdClone, Location};

/// One detection source of a file, as a pass sees it: the whole file, or one
/// block embedded in it (a component's script, a Markdown code block).
pub struct PassSource<'a> {
    /// Source id; an embedded block's has the `path:format` form.
    pub id: &'a str,
    pub format: &'a str,
    /// Start and end of every detection token, in order.
    pub spans: &'a [(Location, Location)],
}

/// What a pass gets when it runs.
pub struct PassContext<'a> {
    /// Every clone the token passes found.
    pub existing: &'a [CpdClone],
    /// `--min-tokens` and `--min-lines`.
    pub min_tokens: usize,
    pub min_lines: usize,
    /// A source's place for the path filters (`--skip-local`,
    /// `--skip-isolated`): pairs whose labels skip each other are dropped.
    pub label: &'a (dyn Fn(&str) -> PathLabel + Sync),
}

/// A clone detection pass over whole files.
pub trait ClonePass: Send + Sync {
    /// The option that turns the pass on, for messages (`--semantic`).
    fn name(&self) -> &'static str;
    /// Whether the pass reads files of `format`; others are not shown to it.
    fn reads(&self, format: &str) -> bool;
    /// One file, while the finder holds it: its format, its whole content
    /// and the detection sources it was split into. Called from worker
    /// threads, in no particular order.
    fn read(&self, format: &str, content: &str, sources: &[PassSource<'_>]);
    /// The clones the pass finds among the files it read.
    fn find(&self, context: &PassContext<'_>) -> Result<Vec<CpdClone>, String>;
}

impl std::fmt::Debug for dyn ClonePass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
