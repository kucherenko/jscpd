//! Semantic clones for jscpd (`--semantic`, experimental): functions that
//! do the same job written differently — renamed, restructured, or in
//! another language, like a rule a Rust backend enforces and a Svelte
//! frontend repeats.
//!
//! - [`extract`] and [`units`] find the functions of a source and the text a
//!   model sees for each;
//! - [`embed`] turns those texts into vectors: a model run in-process
//!   ([`Provider::Local`]) or an OpenAI-compatible API ([`Provider::Http`]),
//!   behind an on-disk cache;
//! - [`search`] pairs the functions whose vectors point the same way;
//! - [`SemanticPass`] runs all of it as a clone pass of the finder
//!   ([`cpd_finder::pass`]).
//!
//! Everything `--semantic` needs lives here, with every dependency it
//! brings — the model runtime, the model's tokenizer, the tree-sitter
//! grammars, the HTTP client — so the core, the tokenizer and the finder
//! compile without them.

pub mod embed;
pub mod extract;
mod pass;
pub mod search;
pub mod units;

pub use embed::{
    API_KEY_ENV, DEFAULT_HTTP_MODEL, DEFAULT_LOCAL_MODEL, DEFAULT_THRESHOLD, DEFAULT_URL, Provider,
    SemanticOptions, download, embedder,
};
pub use pass::SemanticPass;
pub use search::{Embedder, SemanticScope};
