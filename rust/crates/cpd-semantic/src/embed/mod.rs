// embed — the embedding side of `--semantic` (experimental).
//
// The search lives in `crate::search`; this module turns function texts
// into vectors with one of two providers behind one on-disk vector cache:
//
// - `local` (the default): a model downloaded once with
//   `--semantic-download` and run in-process on the CPU, so a scan makes no
//   network call;
// - `http`: any OpenAI-compatible embeddings API — Ollama, LM Studio,
//   llama.cpp's llama-server, text-embeddings-inference, hosted APIs. The
//   API key, when one is needed, comes from the environment only, never
//   from a flag or a config file that could be committed.

mod cache;
mod http;
mod jina_bert;
mod local;
pub mod models;

use crate::search::{Embedder, SemanticScope};
use serde::Serialize;
use serde_json::{Map, Value};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

pub const DEFAULT_URL: &str = "http://localhost:11434/v1";
/// The default model of the local provider.
pub const DEFAULT_LOCAL_MODEL: &str = models::JINA_V2_BASE_CODE.id;
/// The same model under the name Ollama serves it by.
pub const DEFAULT_HTTP_MODEL: &str = "unclemusclez/jina-embeddings-v2-base-code";
/// Cosine floor calibrated for the default model; see `crate::search`
/// for the rules that make one floor work across languages.
pub const DEFAULT_THRESHOLD: f32 = 0.6;
pub const API_KEY_ENV: &str = "JSCPD_SEMANTIC_API_KEY";

/// Longest function text embedded, in bytes; a longer function is embedded
/// by its head, which is where its signature and main logic are.
const MAX_TEXT_BYTES: usize = 8_000;

/// Where vectors come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// A downloaded model run in-process.
    Local,
    /// An OpenAI-compatible embeddings API.
    Http,
}

impl std::str::FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Provider::Local),
            "http" => Ok(Provider::Http),
            other => Err(format!("unknown provider '{other}': must be local or http")),
        }
    }
}

/// What `--semantic` runs with, after merging flags and the config file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticOptions {
    pub provider: Provider,
    pub threshold: f32,
    #[serde(serialize_with = "scope_name")]
    pub scope: SemanticScope,
    pub model: String,
    /// The embeddings API; only the `http` provider uses it.
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// Extra request fields for an API, e.g. `{"task": "code2code.query"}`.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
    pub cache: bool,
    /// Whether `url` came from a config file rather than the command line.
    /// A config file is shared, and can arrive with the code being scanned,
    /// so such a URL, unless it is on this machine, gets no API key, and
    /// gets code only when `--semantic` itself was typed.
    #[serde(skip)]
    pub url_from_config: bool,
    /// Whether `--semantic` was given on the command line, rather than
    /// turned on by a config file.
    #[serde(skip)]
    pub on_command_line: bool,
}

fn scope_name<S: serde::Serializer>(scope: &SemanticScope, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(scope.as_str())
}

impl Default for SemanticOptions {
    fn default() -> Self {
        Self {
            provider: Provider::Local,
            threshold: DEFAULT_THRESHOLD,
            scope: SemanticScope::All,
            model: DEFAULT_LOCAL_MODEL.to_string(),
            url: DEFAULT_URL.to_string(),
            dimensions: None,
            params: Map::new(),
            cache: true,
            url_from_config: false,
            on_command_line: false,
        }
    }
}

/// One way of turning texts into vectors.
trait Backend: Send + Sync {
    /// The model and where it runs, for the progress line.
    fn label(&self) -> String;
    /// The model's name, for the cache file name.
    fn model_name(&self) -> &str;
    /// Everything that changes the vectors of a text, for the cache key.
    fn cache_identity(&self) -> Value;
    /// One vector per text, in order; `progress` gets the number done.
    fn embed(&self, texts: &[&str], progress: &dyn Fn(usize)) -> Result<Vec<Vec<f32>>, String>;
}

/// The embedder `options` asks for. The local provider's model must be
/// downloaded already (see [`download`]); this fails before any scanning
/// starts when it is not.
pub fn embedder(options: &SemanticOptions, quiet: bool) -> Result<Arc<dyn Embedder>, String> {
    let root = cache::root();
    let backend: Box<dyn Backend> = match options.provider {
        Provider::Http => Box::new(http::HttpBackend::new(options)?),
        Provider::Local => {
            let model = local_model(options)?;
            let root = root.clone().ok_or_else(no_cache_dir)?;
            let dir = model.dir(&root);
            if !model.is_downloaded(&dir) {
                return Err(format!(
                    "the model {} is not downloaded yet. Run `jscpd --semantic-download` once ({:.0} MB into {}), or use an embeddings API with --semantic-url",
                    model.id,
                    model.size() as f64 / 1e6,
                    dir.display()
                ));
            }
            Box::new(local::LocalBackend::new(model, dir))
        }
    };
    let cache_file = match options.cache {
        true => root.map(|r| {
            r.join("embeddings").join(cache::file_name(
                backend.model_name(),
                &backend.cache_identity(),
            ))
        }),
        false => None,
    };
    Ok(Arc::new(Cached {
        backend,
        cache_file,
        quiet,
    }))
}

/// `--semantic-download`: fetch the local model's files that are missing,
/// verified against their pinned checksums. Returns their directory.
pub fn download(options: &SemanticOptions, quiet: bool) -> Result<PathBuf, String> {
    if options.provider == Provider::Http {
        return Err(
            "--semantic-download fetches a model for the local provider; an embeddings API needs none"
                .to_string(),
        );
    }
    let model = local_model(options)?;
    let dir = model.dir(&cache::root().ok_or_else(no_cache_dir)?);
    if model.is_downloaded(&dir) {
        if !quiet {
            eprintln!("{} is already downloaded: {}", model.id, dir.display());
        }
        return Ok(dir);
    }
    model.download(&dir, &http::agent(), quiet)?;
    Ok(dir)
}

fn local_model(options: &SemanticOptions) -> Result<&'static models::LocalModel, String> {
    models::find(&options.model).ok_or_else(|| {
        format!(
            "the local provider runs {}; for '{}' use an embeddings API with --semantic-url",
            models::names(),
            options.model
        )
    })
}

fn no_cache_dir() -> String {
    format!(
        "no cache directory to keep the model in: set {}",
        cache::CACHE_DIR_ENV
    )
}

/// A backend behind the vector cache: identical texts and texts embedded by
/// an earlier run are never embedded again.
struct Cached {
    backend: Box<dyn Backend>,
    cache_file: Option<PathBuf>,
    quiet: bool,
}

impl Cached {
    fn note(&self, message: &str) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }
}

impl Embedder for Cached {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        use xxhash_rust::xxh3::xxh3_128;
        let texts: Vec<&str> = texts.iter().map(|t| clip(t, MAX_TEXT_BYTES)).collect();
        let keys: Vec<u128> = texts.iter().map(|t| xxh3_128(t.as_bytes())).collect();
        let mut cache = self
            .cache_file
            .as_deref()
            .map(cache::load)
            .unwrap_or_default();
        let mut announced = false;
        loop {
            let mut seen = std::collections::HashSet::new();
            let missing: Vec<usize> = (0..keys.len())
                .filter(|&i| !cache.vectors.contains_key(&keys[i]) && seen.insert(keys[i]))
                .collect();
            if !announced {
                let distinct = keys.iter().collect::<std::collections::HashSet<_>>().len();
                self.note(&announcement(
                    texts.len(),
                    distinct - missing.len(),
                    missing.len(),
                    &self.backend.label(),
                    self.backend.model_name(),
                ));
                announced = true;
            }
            if missing.is_empty() {
                return Ok(keys.iter().map(|k| cache.vectors[k].clone()).collect());
            }
            let batch: Vec<&str> = missing.iter().map(|&i| texts[i]).collect();
            let live = !self.quiet && std::io::stderr().is_terminal();
            let progress = |done: usize| {
                if live {
                    eprint!("\r  embedded {done} of {}", batch.len());
                }
            };
            let vectors = self.backend.embed(&batch, &progress)?;
            if live {
                eprintln!();
            }
            let dims = vectors.first().map_or(0, Vec::len);
            if cache.dims != 0 && cache.dims != dims {
                // Same name, other vectors: the model changed. Nothing
                // cached can be mixed with the new ones.
                cache = cache::Cache::default();
                continue;
            }
            cache.dims = dims;
            let fresh: Vec<(u128, Vec<f32>)> =
                missing.iter().map(|&i| keys[i]).zip(vectors).collect();
            if let Some(path) = &self.cache_file
                && let Err(e) = cache::save(path, &mut cache, &fresh, &keys)
            {
                self.note(&format!(
                    "Warning: --semantic: cache {} not written: {e}",
                    path.display()
                ));
            }
            cache.vectors.extend(fresh);
        }
    }
}

/// The line announcing a run's embedding: of `total` functions, the
/// distinct texts found in the cache and the ones to embed. Identical texts
/// are embedded once, which is not caching.
fn announcement(total: usize, cached: usize, todo: usize, label: &str, model: &str) -> String {
    match (todo, cached) {
        (0, _) => {
            format!(
                "Semantic clones (experimental): {total} functions, all embeddings cached ({model})"
            )
        }
        (_, 0) => {
            format!("Semantic clones (experimental): embedding {total} functions with {label}")
        }
        _ => format!(
            "Semantic clones (experimental): embedding {todo} of {total} functions with {label}, the rest cached"
        ),
    }
}

/// The longest prefix of `text` of at most `max` bytes, cut at a char
/// boundary.
fn clip(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_cuts_at_a_char_boundary() {
        assert_eq!(clip("abc", 10), "abc");
        assert_eq!(clip("aé", 2), "a");
        assert_eq!(clip("aéb", 3), "aé");
    }

    #[test]
    fn the_announcement_counts_duplicates_as_embedded_not_cached() {
        let line = |cached, todo| announcement(897, cached, todo, "m here", "m");
        assert!(line(0, 890).ends_with("embedding 897 functions with m here"));
        assert!(
            line(10, 880).ends_with("embedding 880 of 897 functions with m here, the rest cached")
        );
        assert!(line(890, 0).ends_with("897 functions, all embeddings cached (m)"));
    }

    #[test]
    fn provider_names() {
        assert_eq!("LOCAL".parse::<Provider>(), Ok(Provider::Local));
        assert_eq!("http".parse::<Provider>(), Ok(Provider::Http));
        assert!(
            "grpc"
                .parse::<Provider>()
                .unwrap_err()
                .contains("local or http")
        );
    }

    #[test]
    fn an_unknown_local_model_names_the_known_ones() {
        let options = SemanticOptions {
            model: "some/other-model".into(),
            ..SemanticOptions::default()
        };
        let err = embedder(&options, true).err().unwrap();
        assert!(err.contains("jinaai/jina-embeddings-v2-base-code"), "{err}");
        assert!(err.contains("--semantic-url"), "{err}");
        let http = SemanticOptions {
            provider: Provider::Http,
            ..SemanticOptions::default()
        };
        assert!(download(&http, true).unwrap_err().contains("needs none"));
    }
}
