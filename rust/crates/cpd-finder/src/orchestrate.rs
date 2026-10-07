// orchestrate.rs

use crate::pass::{ClonePass, PassContext, PassSource};
use crate::statistics;
use crate::walker::{WalkConfig, walk_excluding};
use cpd_core::detect::{
    PathFilters, PathLabel, PreparedSource, detect_prepared, merge_gapped_clones,
};
use cpd_core::models::{CpdClone, KindFilter, SourceFile, Statistics};
use cpd_similarity::functions::{
    RawFunction, extract_embedded_units, extract_units, supports_functions,
};
use cpd_similarity::{
    CandidatePolicy, CodeSize, FunctionSig, FunctionSource, SignaturePolicy, SimilarityCandidates,
    SimilarityDecorators, SimilarityIdentifiers, SimilarityLiterals, discount_token_lines,
    find_similar_units,
};
use cpd_tokenizer::tokenizer::{
    Mode, TokenizeOptions, code_ignore_ranges, tokenize_to_detection, tokenize_to_detection_maps,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Full run configuration.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub paths: Vec<PathBuf>,
    pub min_tokens: usize,
    pub min_lines: usize,
    pub max_lines: Option<usize>,
    /// Merge clones of one file pair separated by at most this many unmatched
    /// lines into a `similar` clone (issue #999). 0 = off.
    pub max_gap_lines: usize,
    /// Report function pairs (JavaScript, TypeScript, Python) whose AST
    /// similarity reaches this value as `similar` clones (issue #999, stage
    /// 2). `1.0` (the default) means exact matches only: the pass does not
    /// run. See [`RunConfig::similarity_threshold`].
    pub similarity: f32,
    /// Which identifier names the function summaries of `similarity` keep
    /// (`--similarity-identifiers`, issue #1136): none by default.
    pub similarity_identifiers: SimilarityIdentifiers,
    /// How literals take part in the function summaries of `similarity`
    /// (`--similarity-literals`, issue #1139): by category by default.
    pub similarity_literals: SimilarityLiterals,
    /// How decorators take part in the summaries of `similarity`
    /// (`--similarity-decorators`, issue #1132): left out by default.
    pub similarity_decorators: SimilarityDecorators,
    /// Which units `similarity` compares (`--similarity-candidates`, issue
    /// #1134): every unit by default.
    pub similarity_candidates: SimilarityCandidates,
    /// Leave test code out of the units `similarity` compares
    /// (`--similarity-skip-tests`, issue #1134).
    pub similarity_skip_tests: bool,
    /// Keep the pairs of `similarity` inside the pairs of classes that
    /// matched, in [`RunResult::inner_pairs`], for a baseline: they are part
    /// of those pairs and are not reported. Without it they are not looked
    /// for.
    pub keep_inner_pairs: bool,
    pub mode: Mode,
    pub formats: Vec<String>,
    pub ignore: Vec<String>,
    pub code_ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub no_gitignore: bool,
    pub follow_symlinks: bool,
    pub skip_local: bool,
    /// Isolation groups (`--skip-isolated`): clones spanning two different
    /// folders of the same group are dropped.
    pub skip_isolated: Vec<Vec<PathBuf>>,
    pub blame: bool,
    pub workers: Option<usize>,
    pub ignore_case: bool,
    /// Type-2 normalization (issue #998): see `TokenizeOptions`.
    pub ignore_identifiers: bool,
    pub ignore_literals: bool,
    pub ignore_annotations: bool,
    pub formats_exts: std::collections::HashMap<String, Vec<String>>,
    pub formats_names: std::collections::HashMap<String, Vec<String>>,
    pub pattern: Option<String>,
    /// Format equivalence groups: formats in the same group share one clone
    /// detection pool (`--cross-formats`). Empty = every format is isolated.
    pub cross_formats: Vec<Vec<String>>,
    /// Keep only clones of these kinds (`--kind`). Empty = every kind. Applied
    /// before statistics, so percentages describe the clones reported.
    pub kinds: Vec<KindFilter>,
    /// Passes that look at whole files after the token passes (see
    /// [`crate::pass`]); `--semantic` adds one. Empty: none runs, and no
    /// file is read for them.
    pub passes: Vec<Arc<dyn ClonePass>>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            paths: vec![],
            min_tokens: 50,
            min_lines: 5,
            max_lines: None,
            max_gap_lines: 0,
            similarity: 1.0,
            similarity_identifiers: SimilarityIdentifiers::Ignore,
            similarity_literals: SimilarityLiterals::Categories,
            similarity_decorators: SimilarityDecorators::Omit,
            similarity_candidates: SimilarityCandidates::All,
            similarity_skip_tests: false,
            keep_inner_pairs: false,
            mode: Mode::Mild,
            formats: vec![],
            ignore: vec![],
            code_ignore_patterns: vec![],
            max_size: None,
            no_gitignore: false,
            follow_symlinks: false,
            skip_local: false,
            skip_isolated: vec![],
            blame: false,
            workers: None,
            ignore_case: false,
            ignore_identifiers: false,
            ignore_literals: false,
            ignore_annotations: false,
            formats_exts: std::collections::HashMap::new(),
            formats_names: std::collections::HashMap::new(),
            pattern: None,
            cross_formats: vec![],
            kinds: vec![],
            passes: vec![],
        }
    }
}

impl RunConfig {
    /// The function-similarity threshold when the pass is enabled: values
    /// strictly between 0 and 1. `1.0` (exact only) and out-of-range values
    /// yield `None`, so a run without `--similarity` never touches the
    /// function extractor or the index.
    pub fn similarity_threshold(&self) -> Option<f32> {
        (self.similarity > 0.0 && self.similarity < 1.0).then_some(self.similarity)
    }

    /// What the function summaries of `similarity` keep besides node types:
    /// `--similarity-identifiers`, `--similarity-literals` and
    /// `--similarity-decorators`.
    pub fn signature_policy(&self) -> SignaturePolicy {
        SignaturePolicy {
            identifiers: self.similarity_identifiers,
            literals: self.similarity_literals,
            decorators: self.similarity_decorators,
        }
    }

    /// The units `similarity` compares: `--similarity-candidates` and
    /// `--similarity-skip-tests`.
    pub fn candidate_policy(&self) -> CandidatePolicy {
        CandidatePolicy {
            scope: self.similarity_candidates,
            skip_tests: self.similarity_skip_tests,
        }
    }
}

/// Why a run failed. Only a clone pass can fail — `--semantic` depends on
/// an embedding model — and the token passes never do.
#[derive(Debug, Clone, PartialEq)]
pub enum RunError {
    Pass {
        /// The option of the pass that failed (`--semantic`).
        name: &'static str,
        message: String,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Pass { name, message } => write!(f, "{name}: {message}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Result of a full run.
pub struct RunResult {
    pub clones: Vec<CpdClone>,
    pub statistics: Statistics,
    pub sources: Vec<SourceFile>,
    /// The pairs of `--similarity` inside the pairs of classes that matched,
    /// which are part of them and not in `clones`; only with
    /// [`RunConfig::keep_inner_pairs`]. A baseline records them, so the pair
    /// of two methods stays known when the pair of their classes breaks.
    pub inner_pairs: Vec<CpdClone>,
}

/// Sources produced by the walk + tokenize phase, before clone detection.
pub struct PreparedScan {
    /// Display sources (used by reporters and statistics).
    pub sources: Vec<SourceFile>,
    /// Detection-ready sources (hashed token streams).
    pub prepared: Vec<PreparedSource>,
}

/// Build the rayon thread pool used for tokenization and detection.
///
/// The pool uses a large stack to survive OXC parsing of deeply-nested
/// JS/TS files (e.g., thousands of chained for-loops with no body). OXC's
/// recursive-descent parser allocates one frame per nesting level; the default
/// 8 MiB thread stack is insufficient for pathological inputs like Bun's
/// `lots-of-for-loop.js`. 64 MiB gives ample headroom while remaining reasonable.
/// A local pool (not build_global) avoids poisoning any caller-owned global pool
/// and can be created unconditionally.
pub fn build_thread_pool(workers: Option<usize>) -> rayon::ThreadPool {
    let mut builder =
        rayon::ThreadPoolBuilder::new().stack_size(64 * 1024 * 1024 /* 64 MiB */);
    if let Some(n) = workers {
        builder = builder.num_threads(n);
    }
    builder
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().expect("rayon pool"))
}

/// Run the full detection pipeline.
///
/// Fails only when a clone pass of `config.passes` fails; without one,
/// `run(&config).unwrap()` never panics.
pub fn run(config: &RunConfig) -> Result<RunResult, RunError> {
    run_excluding(config, &[])
}

/// [`run`], leaving out the folders `exclude_dirs` (see
/// [`crate::walker::walk_excluding`]).
pub fn run_excluding(config: &RunConfig, exclude_dirs: &[PathBuf]) -> Result<RunResult, RunError> {
    let pool = build_thread_pool(config.workers);

    // 1-2. Walk + tokenize. The units of --similarity come apart from the
    // prepared sources, which the pools consume; none without it.
    let (mut source_files, mut prepared_sources, mut function_sources) =
        (Vec::new(), Vec::new(), Vec::new());
    for file in prepare_files_in(&pool, config, exclude_dirs) {
        source_files.extend(file.sources);
        prepared_sources.extend(file.prepared);
        function_sources.extend(file.functions);
    }

    // 3. Group prepared sources into detection pools (deterministic order).
    let format_groups = build_pools(prepared_sources, &config.cross_formats);

    // 4. Detect clones — skip_local uses scan roots to determine same-directory
    //    pairs; skip_isolated uses its group folders the same way.
    //    These directories and the file ids they are compared with must use
    //    the same normalization: ids are anchored at the canonical scan root,
    //    so canonicalize the directories once here (resolves symlinks like
    //    macOS /var → /private/var). Fall back to the original path if
    //    canonicalize fails.
    let scan_roots = canonicalize_all(&config.paths);
    let isolated_groups: Vec<Vec<std::path::PathBuf>> = config
        .skip_isolated
        .iter()
        .map(|group| canonicalize_all(group))
        .collect();
    let path_filters = PathFilters {
        skip_local: config.skip_local,
        scan_roots: &scan_roots,
        isolated_groups: &isolated_groups,
    };
    let clones = pool.install(|| {
        detect_prepared(
            format_groups,
            config.min_tokens,
            config.min_lines,
            &path_filters,
        )
    });

    // 4b. Near-miss merging — a no-op unless --max-gap-lines is set.
    let mut clones = merge_gapped_clones(clones, config.max_gap_lines);

    // 4c. Function-level similarity — only when --similarity is set.
    let mut inner_pairs = Vec::new();
    if let Some(threshold) = config.similarity_threshold() {
        let similar = find_similar_units(
            function_sources,
            threshold,
            config.min_tokens,
            config.min_lines,
            &clones,
            &path_filters,
            config.keep_inner_pairs,
        );
        clones.extend(similar.pairs);
        inner_pairs = similar.inner;
    }

    // 4d. Clone passes over whole files (--semantic).
    if !config.passes.is_empty() {
        let filters = PathFilters {
            skip_local: config.skip_local,
            scan_roots: &scan_roots,
            isolated_groups: &isolated_groups,
        };
        let active = filters.is_active();
        let label = |id: &str| match active {
            true => filters.label(id),
            false => PathLabel::default(),
        };
        for pass in &config.passes {
            let context = PassContext {
                existing: &clones,
                min_tokens: config.min_tokens,
                min_lines: config.min_lines,
                label: &label,
            };
            let found = pool
                .install(|| pass.find(&context))
                .map_err(|message| RunError::Pass {
                    name: pass.name(),
                    message,
                })?;
            clones.extend(found);
        }
    }

    // 4e. --kind: drop the kinds nobody asked for.
    if !config.kinds.is_empty() {
        let asked = |clone: &CpdClone| config.kinds.iter().any(|kind| kind.matches(clone));
        clones.retain(asked);
        inner_pairs.retain(asked);
    }
    // A line that a token clone reports counts once.
    discount_token_lines(&mut clones);

    // 5. Compute statistics.
    let statistics = statistics::compute(&source_files, &clones);

    Ok(RunResult {
        clones,
        statistics,
        sources: source_files,
        inner_pairs,
    })
}

/// Canonicalize every path, falling back to the original on failure (e.g. a
/// directory that does not exist).
pub fn canonicalize_all(paths: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    paths
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect()
}

/// Walk the configured paths and tokenize every matching file, producing both
/// display sources and detection-ready prepared sources. This is steps 1-2 of
/// [`run`]; callers that need to keep prepared sources around (e.g. the MCP
/// server's snippet checks) use it directly and run detection themselves.
pub fn prepare_scan_in(pool: &rayon::ThreadPool, config: &RunConfig) -> PreparedScan {
    let (sources, prepared) = prepare_files_in(pool, config, &[]).into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut ss, mut ps): (Vec<SourceFile>, Vec<PreparedSource>), file| {
            ss.extend(file.sources);
            ps.extend(file.prepared);
            (ss, ps)
        },
    );
    PreparedScan { sources, prepared }
}

/// One walked file, tokenized: what [`prepare_scan_in`] makes of it, kept
/// together so a server can replace the file later (see [`FilePreparer`]).
pub struct PreparedFile {
    /// The source id: the walked path anchored at the scan root.
    pub id: String,
    /// The canonical path when it differs from `id`, else empty.
    pub real_path: String,
    /// The format the walk gave the file (the host format of a file that
    /// embeds other languages).
    pub format: String,
    pub sources: Vec<SourceFile>,
    pub prepared: Vec<PreparedSource>,
    /// The units `--similarity` compares, by prepared source; empty
    /// without it.
    pub functions: Vec<FunctionSource>,
}

/// What [`FilePreparer::prepare`] makes of a text: the sources reports
/// show, the sources detection reads, and the units `--similarity`
/// compares in them.
pub struct PreparedText {
    pub sources: Vec<SourceFile>,
    pub prepared: Vec<PreparedSource>,
    pub functions: Vec<FunctionSource>,
}

/// The walk a run makes over its paths.
pub fn walk_config(config: &RunConfig) -> WalkConfig {
    WalkConfig {
        paths: config.paths.clone(),
        extensions: config.formats.clone(),
        ignore_patterns: config.ignore.clone(),
        max_size: config.max_size,
        follow_symlinks: config.follow_symlinks,
        no_gitignore: config.no_gitignore,
        formats_exts: config.formats_exts.clone(),
        formats_names: config.formats_names.clone(),
        pattern: config.pattern.clone(),
    }
}

/// [`prepare_scan_in`], file by file, leaving out the folders `exclude_dirs`
/// (see [`crate::walker::walk_excluding`]).
pub fn prepare_files_in(
    pool: &rayon::ThreadPool,
    config: &RunConfig,
    exclude_dirs: &[PathBuf],
) -> Vec<PreparedFile> {
    // 1. Walk files
    let discovered = walk_excluding(&walk_config(config), exclude_dirs);

    // 2. Read + tokenize files in parallel.
    use rayon::prelude::*;
    let preparer = FilePreparer::new(config);
    pool.install(|| {
        discovered
            .into_par_iter()
            .filter_map(|file| {
                // Open and memory-map the file inside the worker.  By NOT
                // storing the Mmap in DiscoveredFile we cap concurrent
                // mappings to the rayon thread-pool size, which is always
                // far below vm.max_map_count (default 131 072 on Linux).
                // This also avoids the Vec<u8> allocation that a to_vec()
                // copy would require, matching the allocation profile of the
                // original mmap approach.
                let f = std::fs::File::open(&file.real_path).ok()?;
                let map = unsafe { memmap2::Mmap::map(&f) }.ok()?;
                // Line-count filter — fast O(n) pass before UTF-8 decode.
                if !preparer.fits_lines(&map) {
                    return None;
                }
                let content = str::from_utf8(&map).ok()?;
                // The id is the walked path anchored at the scan root: the
                // name reports show, `--ignore` matched and the path filters
                // compare. Behind a symlink the canonical path differs; it
                // travels separately as the `--skip-isolated` fallback for
                // symlinked group folders (issue #1059).
                let id = file.path.to_string_lossy().into_owned();
                let real_path = if file.real_path == file.path {
                    String::new()
                } else {
                    file.real_path.to_string_lossy().into_owned()
                };
                let text =
                    preparer.prepare(id.clone(), real_path.clone(), &file.format, content)?;
                Some(PreparedFile {
                    id,
                    real_path,
                    format: file.format,
                    sources: text.sources,
                    prepared: text.prepared,
                    functions: text.functions,
                })
            })
            .collect()
    })
}

/// Tokenizes one file the way a scan does, with the options of a
/// [`RunConfig`] compiled once. A scan runs it over every walked file;
/// `--lsp` runs it again over the text of a file open in an editor, so a
/// buffer and the same file on disk give the same tokens.
pub struct FilePreparer<'a> {
    mode: Mode,
    min_tokens: usize,
    min_lines: usize,
    max_lines: Option<usize>,
    ignore_case: bool,
    ignore_identifiers: bool,
    ignore_literals: bool,
    ignore_annotations: bool,
    want_functions: bool,
    policy: SignaturePolicy,
    candidates: CandidatePolicy,
    code_ignore_regexes: Vec<regex::Regex>,
    strip_types_formats: std::collections::HashSet<String>,
    passes: &'a [Arc<dyn ClonePass>],
}

/// Formats whose files embed other languages; each embedded language gets a
/// prepared source of its own.
const MULTI_FORMAT_EXTS: &[&str] = &["md", "markdown", "mkd", "vue", "svelte", "astro"];

impl<'a> FilePreparer<'a> {
    pub fn new(config: &'a RunConfig) -> Self {
        Self {
            mode: config.mode,
            min_tokens: config.min_tokens,
            min_lines: config.min_lines,
            max_lines: config.max_lines,
            ignore_case: config.ignore_case,
            ignore_identifiers: config.ignore_identifiers,
            ignore_literals: config.ignore_literals,
            ignore_annotations: config.ignore_annotations,
            want_functions: config.similarity_threshold().is_some(),
            policy: config.signature_policy(),
            candidates: config.candidate_policy(),
            // Pre-compile code-level ignore regex patterns once for all
            // threads. Invalid patterns are silently skipped.
            code_ignore_regexes: config
                .code_ignore_patterns
                .iter()
                .filter_map(|p| regex::Regex::new(p).ok())
                .collect(),
            strip_types_formats: strip_types_formats(&config.cross_formats),
            passes: &config.passes,
        }
    }

    /// Whether a file of these bytes passes `--min-lines` and `--max-lines`.
    pub fn fits_lines(&self, bytes: &[u8]) -> bool {
        if self.min_lines == 0 && self.max_lines.is_none() {
            return true;
        }
        let newlines = memchr::Memchr::new(b'\n', bytes).count();
        let lines = if !bytes.is_empty() && *bytes.last().unwrap() != b'\n' {
            newlines + 1
        } else {
            newlines
        };
        lines >= self.min_lines && self.max_lines.is_none_or(|max| lines <= max)
    }

    /// The display sources and detection-ready sources of one file: `id` is
    /// its source id, `real_path` its canonical path when that differs (or
    /// empty), `format` the format the walk gave it. `None` when the file is
    /// too short or too long for the run.
    pub fn prepare(
        &self,
        id: String,
        real_path: String,
        format: &str,
        content: &str,
    ) -> Option<PreparedText> {
        if !self.fits_lines(content.as_bytes()) {
            return None;
        }
        let file_bytes = content.len() as u64;
        // Compute code-level ignore ranges from regex matches against source text.
        // This matches v4 semantics: regex patterns are matched against source
        // text, and any token overlapping a match range is skipped during detection.
        let code_ranges = if self.code_ignore_regexes.is_empty() {
            Vec::new()
        } else {
            code_ignore_ranges(content, &self.code_ignore_regexes)
        };
        let opts = TokenizeOptions {
            mode: self.mode,
            ignore_case: self.ignore_case,
            ignore_identifiers: self.ignore_identifiers,
            ignore_literals: self.ignore_literals,
            ignore_annotations: self.ignore_annotations,
            ignore_ranges: code_ranges,
            code_ignore_regexes: self.code_ignore_regexes.clone(),
            strip_types_formats: self.strip_types_formats.clone(),
        };

        if MULTI_FORMAT_EXTS.contains(&format) {
            // Multi-format path: produce one PreparedSource per sub-format.
            let maps = tokenize_to_detection_maps(format, content, &opts);

            // Display path: flat tokenize for the parent SourceFile. The
            // display tokens of a component leave out code its blocks hold,
            // such as Astro's frontmatter, so the file stays when its blocks
            // reach the limit together.
            let tokens = cpd_tokenizer::tokenizer::tokenize(format, content, self.mode);
            let block_tokens: usize = maps.iter().map(|map| map.tokens.len()).sum();
            if tokens.len().max(block_tokens) < self.min_tokens {
                return None;
            }

            let mut source_files = vec![SourceFile {
                id: id.clone(),
                format: format.to_string(),
                tokens,
                bytes: file_bytes,
            }];

            // The functions of the code blocks (Markdown fences, component
            // scripts) by the block's language, for --similarity: each joins
            // the prepared source of its language below.
            let mut block_functions: std::collections::HashMap<String, Vec<RawFunction>> =
                std::collections::HashMap::new();
            if self.want_functions {
                for (block_format, function) in
                    extract_embedded_units(content, format, self.unit_size(), &opts.ignore_ranges)
                {
                    block_functions
                        .entry(block_format)
                        .or_default()
                        .push(function);
                }
            }

            let mut prepared = Vec::new();
            let mut functions = Vec::new();
            for map in maps {
                if map.tokens.len() < self.min_tokens {
                    continue;
                }
                let map_id = format!("{}:{}", id, map.format);
                // For sub-formats, create a synthetic SourceFile with detection
                // tokens converted to display tokens so statistics per-format
                // counts are correct.
                if map.format != format {
                    let synth_tokens: Vec<cpd_core::models::Token> = map
                        .tokens
                        .iter()
                        .map(|dt| cpd_core::models::Token {
                            kind: cpd_core::models::TokenKind::Other,
                            value: String::new(),
                            start: dt.start.clone(),
                            end: dt.end.clone(),
                        })
                        .collect();
                    source_files.push(SourceFile {
                        id: map_id.clone(),
                        format: map.format.clone(),
                        tokens: synth_tokens,
                        bytes: 0,
                    });
                }
                let embedded = map.format != format;
                let mut sub =
                    PreparedSource::from_detection_tokens(map_id, map.format, &map.tokens);
                sub.real_path = real_path.clone();
                // Only a different language is embedded; the host's own
                // map covers the file end to end.
                sub.embedded = embedded;
                if embedded && let Some(units) = block_functions.remove(&sub.format) {
                    functions.extend(self.function_source(&sub, units));
                }
                prepared.push(sub);
            }
            if prepared.is_empty() {
                return None;
            }
            show_passes(self.passes, format, content, &prepared);
            Some(PreparedText {
                sources: source_files,
                prepared,
                functions,
            })
        } else {
            // Single-format path.
            let tokens = cpd_tokenizer::tokenizer::tokenize(format, content, self.mode);
            if tokens.len() < self.min_tokens {
                return None;
            }

            let source_file = SourceFile {
                id: id.clone(),
                format: format.to_string(),
                tokens,
                bytes: file_bytes,
            };

            let det_tokens = tokenize_to_detection(format, content, &opts);
            if det_tokens.len() < self.min_tokens {
                return None;
            }

            let mut prepared =
                PreparedSource::from_detection_tokens(id, format.to_string(), &det_tokens);
            prepared.real_path = real_path;
            let mut functions = Vec::new();
            if self.want_functions && supports_functions(&prepared.format) {
                let units = extract_units(
                    content,
                    &prepared.format,
                    self.unit_size(),
                    &opts.ignore_ranges,
                );
                functions.extend(self.function_source(&prepared, units));
            }
            let prepared = vec![prepared];
            show_passes(self.passes, &prepared[0].format, content, &prepared);

            Some(PreparedText {
                sources: vec![source_file],
                prepared,
                functions,
            })
        }
    }

    /// The smallest unit `--similarity` compares: `--min-tokens` and
    /// `--min-lines` on its code.
    fn unit_size(&self) -> CodeSize {
        CodeSize {
            tokens: self.min_tokens as u32,
            lines: self.min_lines as u32,
        }
    }

    /// The signatures of the `units` of the prepared source `prepared` that
    /// `--similarity-candidates` and `--similarity-skip-tests` keep, over
    /// its token spans, summarized as the signature policy says; `None`
    /// when none has a token.
    fn function_source(
        &self,
        prepared: &PreparedSource,
        mut units: Vec<RawFunction>,
    ) -> Option<FunctionSource> {
        units.retain(|unit| self.candidates.admits(unit.context));
        let signatures: Vec<FunctionSig> =
            cpd_similarity::functions::signatures(units, &prepared.spans, self.policy);
        (!signatures.is_empty()).then(|| FunctionSource::new(prepared, signatures))
    }
}

/// Show a prepared file to the clone passes that read its format.
fn show_passes(
    passes: &[Arc<dyn ClonePass>],
    format: &str,
    content: &str,
    prepared: &[PreparedSource],
) {
    let mut readers = passes.iter().filter(|p| p.reads(format)).peekable();
    if readers.peek().is_none() {
        return;
    }
    let sources: Vec<PassSource<'_>> = prepared
        .iter()
        .map(|p| PassSource {
            id: &p.id,
            format: &p.format,
            spans: &p.spans,
        })
        .collect();
    for pass in readers {
        pass.read(format, content, &sources);
    }
}

/// Group prepared sources into detection pools.
///
/// Formats named in the same `cross_formats` group share one pool; every
/// other format keeps its own isolated pool. With no groups configured this
/// reproduces the historical per-format pools exactly (same membership, same
/// deterministic order).
fn build_pools(
    mut prepared_sources: Vec<PreparedSource>,
    cross_formats: &[Vec<String>],
) -> Vec<Vec<PreparedSource>> {
    let mut group_of: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (idx, group) in cross_formats.iter().enumerate() {
        for format in group {
            group_of.insert(format.as_str(), idx);
        }
    }

    // Deterministic file order inside each pool.
    prepared_sources.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));

    let pool_key = |format: &str| match group_of.get(format) {
        Some(idx) => pool_key_of(*idx),
        None => format!("format:{format}"),
    };

    let mut pool_map: std::collections::HashMap<String, Vec<PreparedSource>> =
        std::collections::HashMap::default();
    for ps in prepared_sources {
        pool_map.entry(pool_key(&ps.format)).or_default().push(ps);
    }
    let mut pools: Vec<(String, Vec<PreparedSource>)> = pool_map.into_iter().collect();
    // Sort pools by key for determinism.
    pools.sort_by(|a, b| a.0.cmp(&b.0));
    pools.into_iter().map(|(_, sources)| sources).collect()
}

/// The detection pool of `format`: the formats of one `--cross-formats`
/// group share a pool, and every other format has its own. Sources of
/// different pools never form a clone.
pub fn pool_key(format: &str, cross_formats: &[Vec<String>]) -> String {
    match cross_formats
        .iter()
        .position(|group| group.iter().any(|f| f == format))
    {
        Some(idx) => pool_key_of(idx),
        None => format!("format:{format}"),
    }
}

fn pool_key_of(group: usize) -> String {
    format!("cross:{group:04}")
}

/// Formats whose TypeScript-only syntax must be stripped before detection:
/// TS-family formats that share a cross-format group with a JS-family format
/// (a TS-only group needs no normalization — pooling alone suffices).
pub fn strip_types_formats(cross_formats: &[Vec<String>]) -> std::collections::HashSet<String> {
    const TS_FAMILY: &[&str] = &["typescript", "tsx"];
    const JS_FAMILY: &[&str] = &["javascript", "jsx"];
    cross_formats
        .iter()
        .filter(|group| group.iter().any(|f| JS_FAMILY.contains(&f.as_str())))
        .flat_map(|group| {
            group
                .iter()
                .filter(|f| TS_FAMILY.contains(&f.as_str()))
                .cloned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn prepared(id: &str, format: &str) -> PreparedSource {
        PreparedSource {
            id: id.to_string(),
            format: format.to_string(),
            hashes: vec![],
            spans: vec![],
            raw_hashes: Vec::new(),
            real_path: String::new(),
            embedded: false,
        }
    }

    fn pool_formats(pools: &[Vec<PreparedSource>]) -> Vec<Vec<&str>> {
        pools
            .iter()
            .map(|pool| pool.iter().map(|ps| ps.format.as_str()).collect())
            .collect()
    }

    #[test]
    fn build_pools_default_isolated() {
        let sources = vec![
            prepared("b.ts", "typescript"),
            prepared("a.js", "javascript"),
            prepared("c.py", "python"),
        ];
        let pools = build_pools(sources, &[]);
        assert_eq!(
            pool_formats(&pools),
            vec![vec!["javascript"], vec!["python"], vec!["typescript"]],
            "no cross-formats: one pool per format, sorted by format"
        );
    }

    #[test]
    fn build_pools_merges_grouped_formats() {
        let sources = vec![
            prepared("a.js", "javascript"),
            prepared("b.ts", "typescript"),
            prepared("c.py", "python"),
        ];
        let groups = vec![vec!["javascript".to_string(), "typescript".to_string()]];
        let pools = build_pools(sources, &groups);
        assert_eq!(
            pool_formats(&pools),
            vec![vec!["javascript", "typescript"], vec!["python"]],
            "grouped formats share one pool; python stays isolated"
        );
    }

    #[test]
    fn strip_types_only_when_group_mixes_ts_and_js() {
        let mixed = vec![vec![
            "javascript".to_string(),
            "typescript".to_string(),
            "tsx".to_string(),
        ]];
        let set = strip_types_formats(&mixed);
        assert!(set.contains("typescript") && set.contains("tsx"));
        assert!(!set.contains("javascript"));

        let ts_only = vec![vec!["typescript".to_string(), "tsx".to_string()]];
        assert!(
            strip_types_formats(&ts_only).is_empty(),
            "TS-only groups need no type stripping"
        );

        assert!(strip_types_formats(&[]).is_empty());
    }

    #[test]
    fn empty_paths_returns_empty_result() {
        let config = RunConfig::default();
        let result = run(&config).unwrap();
        assert!(result.clones.is_empty());
        assert_eq!(result.statistics.total.sources, 0);
    }

    #[test]
    fn nonexistent_path_returns_empty() {
        let config = RunConfig {
            paths: vec![PathBuf::from("/tmp/cpd-nonexistent-xyz")],
            ..Default::default()
        };
        let result = run(&config).unwrap();
        assert!(result.clones.is_empty());
    }

    #[test]
    fn workers_1_produces_same_result_as_default() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walker");
        if !fixtures.exists() {
            return;
        }

        let config_default = RunConfig {
            paths: vec![fixtures.clone()],
            min_tokens: 3,
            ..Default::default()
        };
        let config_single = RunConfig {
            paths: vec![fixtures],
            min_tokens: 3,
            workers: Some(1),
            ..Default::default()
        };

        let r1 = run(&config_default).unwrap();
        let r2 = run(&config_single).unwrap();

        assert_eq!(
            r1.sources.len(),
            r2.sources.len(),
            "--workers 1 must produce same source count as default"
        );
    }
}
