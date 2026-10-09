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

mod bert;
mod cache;
pub mod catalog;
mod http;
mod jina_bert;
mod local;
pub mod models;
mod nomic_bert;
#[cfg(test)]
pub(crate) mod test_server;

use crate::search::{Embedder, SemanticScope};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub const DEFAULT_URL: &str = "http://localhost:11434/v1";
/// The default model of the local provider, by its name in
/// [`catalog::KNOWN_MODELS`].
pub const DEFAULT_LOCAL_MODEL: &str = "CodeRankEmbed";
/// The default model of an embeddings API: jina-embeddings-v2-base-code
/// under the name Ollama serves it by. Ollama's library has no copy of the
/// local default.
pub const DEFAULT_HTTP_MODEL: &str = "unclemusclez/jina-embeddings-v2-base-code";
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
    /// Lowest cosine similarity of a pair across languages.
    pub threshold: f32,
    /// Lowest cosine similarity of a pair within one language; `None`
    /// keeps the model's gap above `threshold` (see [`catalog::thresholds`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_threshold: Option<f32>,
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
    /// Put before every function text; `None` is the prefix of a model in
    /// [`catalog`], or none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
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
    /// `--semantic-rebuild-cache`: embed every function again and replace
    /// the cached vectors of this model and request shape.
    #[serde(skip)]
    pub rebuild_cache: bool,
}

fn scope_name<S: serde::Serializer>(scope: &SemanticScope, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(scope.as_str())
}

impl Default for SemanticOptions {
    fn default() -> Self {
        Self {
            provider: Provider::Local,
            threshold: catalog::thresholds(DEFAULT_LOCAL_MODEL, None, None).across,
            same_threshold: None,
            scope: SemanticScope::All,
            model: DEFAULT_LOCAL_MODEL.to_string(),
            url: DEFAULT_URL.to_string(),
            dimensions: None,
            params: Map::new(),
            prefix: None,
            cache: true,
            url_from_config: false,
            on_command_line: false,
            rebuild_cache: false,
        }
    }
}

impl SemanticOptions {
    /// The thresholds of the rules: the model's (see [`catalog`]), with
    /// [`Self::threshold`] and [`Self::same_threshold`] put in.
    pub fn thresholds(&self) -> crate::search::Thresholds {
        catalog::thresholds(&self.model, Some(self.threshold), self.same_threshold)
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

/// The embedder `options` asks for, keeping its vectors in the cache folder
/// of the `scanned` paths. The local provider's model must be downloaded
/// already (see [`download`]); this fails before any scanning starts when it
/// is not.
pub fn embedder(
    options: &SemanticOptions,
    scanned: &[PathBuf],
    quiet: bool,
) -> Result<Arc<dyn Embedder>, String> {
    let root = cache::root();
    let backend: Arc<dyn Backend> = match options.provider {
        Provider::Http => Arc::new(http::HttpBackend::new(options)?),
        Provider::Local => {
            let (known, model) = local_model(options)?;
            let root = root.clone().ok_or_else(no_cache_dir)?;
            let dir = model.dir(&root);
            if !model.is_downloaded(&dir) {
                let download = match known.name == DEFAULT_LOCAL_MODEL {
                    true => "jscpd --semantic-download".to_string(),
                    false => format!("jscpd --semantic-download {}", known.name),
                };
                return Err(format!(
                    "the model {} is not downloaded yet. Run `{download}` once ({:.0} MB into {}), or use an embeddings API with --semantic-url",
                    model.id,
                    model.size() as f64 / 1e6,
                    dir.display()
                ));
            }
            local_backend(model, dir)
        }
    };
    let prefix = match &options.prefix {
        Some(prefix) => prefix.clone(),
        None => catalog::find(&options.model)
            .map_or("", |m| m.prefix)
            .to_string(),
    };
    let cache_file = match options.cache {
        true => root.map(|r| {
            cache::project_dir(&r, scanned).join(cache::file_name(
                backend.model_name(),
                &cache_identity(backend.as_ref(), &prefix),
            ))
        }),
        false => None,
    };
    Ok(Arc::new(Cached {
        backend,
        prefix,
        cache_file,
        rebuild: options.rebuild_cache,
        quiet,
    }))
}

/// The backend of the local model in `dir`, shared by every embedder of
/// the process: an embedder made again, for other paths or for the next
/// run of a server, uses the weights already loaded instead of loading
/// them again.
fn local_backend(model: &'static models::LocalModel, dir: PathBuf) -> Arc<dyn Backend> {
    static LOADED: OnceLock<Mutex<HashMap<PathBuf, Arc<local::LocalBackend>>>> = OnceLock::new();
    let mut loaded = LOADED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loaded
        .entry(dir.clone())
        .or_insert_with(|| Arc::new(local::LocalBackend::new(model, dir)))
        .clone()
}

/// Everything that changes the vectors of a text: what the backend says,
/// and the prefix when there is one.
fn cache_identity(backend: &dyn Backend, prefix: &str) -> Value {
    let mut identity = backend.cache_identity();
    if let (false, Value::Object(fields)) = (prefix.is_empty(), &mut identity) {
        fields.insert("prefix".into(), Value::from(prefix));
    }
    identity
}

/// The local model `options` needs and has not downloaded yet: its id and
/// its size in bytes, so a front end can ask before fetching it. `None` for
/// an embeddings API, and once the model is there.
pub fn missing_model(options: &SemanticOptions) -> Option<(String, u64)> {
    if options.provider != Provider::Local {
        return None;
    }
    let (_, model) = local_model(options).ok()?;
    let dir = model.dir(&cache::root()?);
    (!model.is_downloaded(&dir)).then(|| (model.id.to_string(), model.size()))
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
    let (_, model) = local_model(options)?;
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

/// `--semantic-models`: the models jscpd has thresholds for, as a table,
/// with where each one runs and what the columns mean.
pub fn model_list() -> String {
    let root = cache::root();
    let mut rows = vec![["MODEL", "CROSS", "SAME", "LICENSE", "RUNS"].map(String::from)];
    for model in catalog::KNOWN_MODELS {
        let name = match model.name == DEFAULT_LOCAL_MODEL {
            true => format!("{} (default)", model.name),
            false => model.name.to_string(),
        };
        let runs = match (model.local, model.ollama.first()) {
            (Some(local), _) => {
                let downloaded = root
                    .as_ref()
                    .is_some_and(|r| local.is_stamped(&local.dir(r)));
                format!(
                    "in jscpd, {:.0} MB{}",
                    local.size() as f64 / 1e6,
                    if downloaded { ", downloaded" } else { "" }
                )
            }
            (None, Some(ollama)) => format!("API (Ollama: {ollama})"),
            (None, None) => "API".to_string(),
        };
        rows.push([
            name,
            model.threshold.to_string(),
            model.same_threshold.to_string(),
            model.license.to_string(),
            runs,
        ]);
    }
    let widths: Vec<usize> = (0..5)
        .map(|c| rows.iter().map(|r| r[c].len()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, &width)| format!("{cell:width$}"))
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out.push_str(
        "\nCROSS is the default --semantic-threshold, for pairs across languages, and SAME\n\
         the default --semantic-same-threshold, for pairs within one language.\n\
         --semantic-model takes the name in the MODEL column, in any letter case, or\n\
         the model's Hugging Face id.\n\
         jscpd runs the models marked \"in jscpd\" on this machine once\n\
         `jscpd --semantic-download <model>` has fetched them; the\n\
         others need an embeddings API that serves them, given with --semantic-url.\n",
    );
    out
}

/// The model the local provider runs for `options`, and its files.
fn local_model(
    options: &SemanticOptions,
) -> Result<(&'static catalog::KnownModel, &'static models::LocalModel), String> {
    match catalog::find(&options.model) {
        Some(
            known @ catalog::KnownModel {
                local: Some(model), ..
            },
        ) => Ok((known, model)),
        Some(known) => Err(format!(
            "jscpd does not run {} itself (it runs {}); serve it with an embeddings API and pass --semantic-url",
            known.name,
            catalog::local_names()
        )),
        None => Err(format!(
            "jscpd runs {} itself, not '{}' (`jscpd --semantic-models` lists the models it knows); for another model use an embeddings API with --semantic-url",
            catalog::local_names(),
            options.model
        )),
    }
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
    backend: Arc<dyn Backend>,
    /// Put before every text the backend embeds; see [`catalog`].
    prefix: String,
    cache_file: Option<PathBuf>,
    /// Read nothing from `cache_file`, and replace it with this run's
    /// vectors.
    rebuild: bool,
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
    /// Embeds `texts` with the backend alone: the cache neither has them
    /// nor gets them, and keeps the vectors of the last [`Embedder::embed`].
    fn embed_once(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        let prefixed: Vec<String> = texts
            .iter()
            .map(|t| format!("{}{}", self.prefix, clip(t, MAX_TEXT_BYTES)))
            .collect();
        let batch: Vec<&str> = prefixed.iter().map(String::as_str).collect();
        self.backend.embed(&batch, &|_| {})
    }

    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        use xxhash_rust::xxh3::xxh3_128;
        let texts: Vec<&str> = texts.iter().map(|t| clip(t, MAX_TEXT_BYTES)).collect();
        let keys: Vec<u128> = texts.iter().map(|t| xxh3_128(t.as_bytes())).collect();
        let mut cache = match &self.cache_file {
            Some(_) if self.rebuild => cache::Cache::replacing(),
            Some(path) => cache::load(path),
            None => cache::Cache::default(),
        };
        let mut announced = false;
        loop {
            let mut seen = std::collections::HashSet::new();
            let missing: Vec<usize> = (0..keys.len())
                .filter(|&i| !cache.vectors.contains_key(&keys[i]) && seen.insert(keys[i]))
                .collect();
            if !announced {
                let distinct = keys.iter().collect::<std::collections::HashSet<_>>().len();
                let mut line = announcement(
                    texts.len(),
                    distinct - missing.len(),
                    missing.len(),
                    &self.backend.label(),
                    self.backend.model_name(),
                );
                if self.rebuild && self.cache_file.is_some() {
                    line.push_str(", rebuilding the cache");
                }
                self.note(&line);
                announced = true;
            }
            if missing.is_empty() {
                return Ok(keys.iter().map(|k| cache.vectors[k].clone()).collect());
            }
            let prefixed: Vec<String> = missing
                .iter()
                .map(|&i| format!("{}{}", self.prefix, texts[i]))
                .collect();
            let batch: Vec<&str> = prefixed.iter().map(String::as_str).collect();
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

/// A fresh directory in the system temp dir for one test of this module.
#[cfg(test)]
pub(crate) fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("jscpd-semantic-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

    /// A backend that remembers the texts it was given.
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

    impl Backend for Recorder {
        fn label(&self) -> String {
            "recorder".into()
        }

        fn model_name(&self) -> &str {
            "recorder"
        }

        fn cache_identity(&self) -> Value {
            serde_json::json!({"model": "recorder"})
        }

        fn embed(&self, texts: &[&str], _: &dyn Fn(usize)) -> Result<Vec<Vec<f32>>, String> {
            let mut seen = self.0.lock().unwrap();
            seen.extend(texts.iter().map(|t| t.to_string()));
            Ok(texts.iter().map(|t| vec![t.len() as f32, 1.0]).collect())
        }
    }

    #[test]
    fn the_prefix_goes_before_every_text_and_into_the_cache_key() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let cached = Cached {
            backend: Arc::new(Recorder(seen.clone())),
            prefix: "Code: ".into(),
            cache_file: None,
            rebuild: false,
            quiet: true,
        };
        assert_eq!(cached.embed(&["a", "bb", "a"]).unwrap().len(), 3);
        assert_eq!(*seen.lock().unwrap(), ["Code: a", "Code: bb"]);

        let backend = Recorder(seen);
        assert_eq!(
            cache_identity(&backend, ""),
            backend.cache_identity(),
            "the caches of a model without a prefix stay valid"
        );
        assert_eq!(cache_identity(&backend, "Code: ")["prefix"], "Code: ");
    }

    #[test]
    fn a_text_embedded_once_stays_out_of_the_cache() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let dir = test_dir("embed-once");
        let cached = Cached {
            backend: Arc::new(Recorder(seen.clone())),
            prefix: "Code: ".into(),
            cache_file: Some(dir.join("vectors.bin")),
            rebuild: false,
            quiet: true,
        };
        cached.embed(&["a", "bb"]).unwrap();
        assert_eq!(cached.embed_once(&["ccc"]).unwrap(), [vec![9.0, 1.0]]);
        assert_eq!(*seen.lock().unwrap(), ["Code: a", "Code: bb", "Code: ccc"]);
        // The project's vectors are all still cached, and the snippet's is
        // not: it neither replaced them nor joined them.
        cached.embed(&["a", "bb"]).unwrap();
        cached.embed(&["a", "bb", "ccc"]).unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            ["Code: a", "Code: bb", "Code: ccc", "Code: ccc"]
        );
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
        let err = embedder(&options, &[], true).err().unwrap();
        assert!(
            err.contains("jscpd runs CodeRankEmbed, jina-embeddings-v2-base-code itself, not 'some/other-model'"),
            "{err}"
        );
        assert!(err.contains("--semantic-url"), "{err}");
        let api_only = SemanticOptions {
            model: "qwen3-embedding:0.6b".into(),
            ..SemanticOptions::default()
        };
        let err = embedder(&api_only, &[], true).err().unwrap();
        assert!(
            err.contains("jscpd does not run Qwen3-Embedding-0.6B itself"),
            "{err}"
        );
        let http = SemanticOptions {
            provider: Provider::Http,
            ..SemanticOptions::default()
        };
        assert!(download(&http, true).unwrap_err().contains("needs none"));
    }

    /// A backend whose vectors have as many dimensions as `dims` says, and
    /// that counts the texts it embeds.
    struct Resizable {
        dims: Arc<std::sync::atomic::AtomicUsize>,
        embedded: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Backend for Resizable {
        fn label(&self) -> String {
            "resizable".into()
        }

        fn model_name(&self) -> &str {
            "resizable"
        }

        fn cache_identity(&self) -> Value {
            serde_json::json!({"model": "resizable"})
        }

        fn embed(&self, texts: &[&str], _: &dyn Fn(usize)) -> Result<Vec<Vec<f32>>, String> {
            use std::sync::atomic::Ordering::SeqCst;
            self.embedded.fetch_add(texts.len(), SeqCst);
            let dims = self.dims.load(SeqCst);
            Ok(texts.iter().map(|t| vec![t.len() as f32; dims]).collect())
        }
    }

    #[test]
    fn vectors_of_a_changed_model_never_mix_with_cached_ones() {
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        let dims = Arc::new(AtomicUsize::new(2));
        let embedded = Arc::new(AtomicUsize::new(0));
        let dir = test_dir("embed-resized");
        let cached = Cached {
            backend: Arc::new(Resizable {
                dims: dims.clone(),
                embedded: embedded.clone(),
            }),
            prefix: String::new(),
            cache_file: Some(dir.join("vectors.bin")),
            rebuild: false,
            quiet: true,
        };
        assert_eq!(cached.embed(&["a"]).unwrap(), [vec![1.0; 2]]);
        dims.store(3, SeqCst);
        let after = cached.embed(&["a", "bb"]).unwrap();
        assert_eq!(after, [vec![1.0; 3], vec![2.0; 3]], "all of the new model");
        assert_eq!(embedded.load(SeqCst), 1 + 1 + 2, "the cached one again too");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_cache_that_cannot_be_written_does_not_fail_the_embedding() {
        let dir = test_dir("embed-unwritable");
        let blocker = dir.join("not-a-dir");
        std::fs::write(&blocker, "file").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let cached = Cached {
            backend: Arc::new(Recorder(seen.clone())),
            prefix: String::new(),
            cache_file: Some(blocker.join("vectors.bin")),
            rebuild: false,
            quiet: true,
        };
        assert_eq!(cached.embed(&["abc"]).unwrap(), [vec![3.0, 1.0]]);
        assert_eq!(cached.embed(&["abc"]).unwrap(), [vec![3.0, 1.0]]);
        assert_eq!(seen.lock().unwrap().len(), 2, "nothing was cached");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_long_text_is_embedded_by_its_head() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let cached = Cached {
            backend: Arc::new(Recorder(seen.clone())),
            prefix: "P: ".into(),
            cache_file: None,
            rebuild: false,
            quiet: true,
        };
        let long = "é".repeat(MAX_TEXT_BYTES);
        cached.embed(&[&long]).unwrap();
        cached.embed_once(&[&long]).unwrap();
        let seen = seen.lock().unwrap();
        let head = format!("P: {}", "é".repeat(MAX_TEXT_BYTES / 2));
        assert_eq!(*seen, [head.clone(), head], "cut at a char boundary");
    }
}
