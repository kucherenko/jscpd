// orchestrate.rs

use crate::pass::{ClonePass, PassContext, PassSource};
use crate::statistics;
use crate::walker::{WalkConfig, walk_excluding};
use cpd_core::detect::{
    PathFilters, PathLabel, PreparedSource, detect_prepared, merge_gapped_clones,
};
use cpd_core::models::{CpdClone, DetectionToken, KindFilter, SourceFile, Statistics};
use cpd_similarity::{Form, FormSource, discount_token_lines, find_similar};
use cpd_tokenizer::tokenizer::{
    Mode, TokenizeOptions, code_ignore_ranges, compile_ignore_patterns, tokenize_to_detection,
    tokenize_to_detection_maps,
};
use std::path::{Path, PathBuf};
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
    /// Report the pairs of functions whose structural similarity reaches
    /// this value as `similar` clones (`--similarity`); `None` runs no such
    /// search. See [`RunConfig::similarity_threshold`].
    pub similarity: Option<f64>,
    /// The fewest normalized nodes a unit `similarity` compares has
    /// (`--min-nodes`).
    pub min_nodes: u32,
    /// Keep every pair `similarity` finds in [`RunResult::similar`], for the
    /// `edn` report; without it the search keeps the pairs it reports only.
    pub all_similar: bool,
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
            similarity: None,
            min_nodes: cpd_similarity::DEFAULT_MIN_NODES,
            all_similar: false,
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
    /// The threshold of the search for structurally similar functions,
    /// when it runs: a value in `(0, 1]`.
    pub fn similarity_threshold(&self) -> Option<f64> {
        self.similarity.filter(|&t| t > 0.0 && t <= 1.0)
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
    /// Every pair `--similarity` found, most similar first, the ones that
    /// clones in `clones` cover included, when
    /// [`RunConfig::all_similar`] asks for them: the `edn` report lists them.
    pub similar: Vec<CpdClone>,
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

    // 4c. Structurally similar functions — only when --similarity is set.
    let mut similar = Vec::new();
    if let Some(threshold) = config.similarity_threshold() {
        let found = find_similar(
            function_sources,
            threshold,
            config.min_nodes,
            config.min_lines as u32,
            &clones,
            &path_filters,
            config.all_similar,
        );
        clones.extend(found.reported);
        similar = found.all;
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
        similar.retain(asked);
    }
    // A line that a token clone reports counts once.
    discount_token_lines(&mut clones);

    // 5. Compute statistics.
    let statistics = statistics::compute(&source_files, &clones);

    Ok(RunResult {
        clones,
        statistics,
        sources: source_files,
        similar,
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
    pub functions: Vec<FormSource>,
}

/// What [`FilePreparer::prepare`] makes of a text: the sources reports
/// show, the sources detection reads, and the units `--similarity`
/// compares in them.
pub struct PreparedText {
    pub sources: Vec<SourceFile>,
    pub prepared: Vec<PreparedSource>,
    pub functions: Vec<FormSource>,
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
    code_ignore_regexes: Vec<regex::Regex>,
    strip_types_formats: std::collections::HashSet<String>,
    passes: &'a [Arc<dyn ClonePass>],
    /// The scan roots, canonical as the walker anchors the ids of the files
    /// and as given: `--similarity` reads a file's path below its root, so a
    /// project in a folder named `tests` is no test.
    roots: Vec<String>,
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
            // Pre-compile code-level ignore regex patterns once for all
            // threads. Invalid patterns are silently skipped.
            code_ignore_regexes: compile_ignore_patterns(&config.code_ignore_patterns),
            strip_types_formats: strip_types_formats(&config.cross_formats),
            passes: &config.passes,
            roots: canonicalize_all(&config.paths)
                .iter()
                .chain(&config.paths)
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
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

        // The units --similarity compares; it reads them in a file too small
        // for the token passes as well.
        let reads_forms = self.want_functions
            && cpd_similarity::reads(Path::new(below_root(&id, &self.roots)), format);

        if MULTI_FORMAT_EXTS.contains(&format) {
            // Multi-format path: produce one PreparedSource per sub-format.
            let maps = tokenize_to_detection_maps(format, content, &opts);

            // The units of the code blocks (Markdown fences, component
            // scripts) by the block's language: each joins the source of its
            // language below, measured on that language's tokens.
            let mut block_forms: std::collections::HashMap<String, Vec<Form>> =
                std::collections::HashMap::new();
            if reads_forms {
                for (block_format, form) in
                    cpd_similarity::embedded_forms(content, format, &opts.ignore_ranges)
                {
                    block_forms.entry(block_format).or_default().push(form);
                }
            }
            let mut functions = Vec::new();
            for map in maps.iter().filter(|map| map.format != format) {
                if let Some(forms) = block_forms.remove(&map.format) {
                    let spans: Vec<_> = map
                        .tokens
                        .iter()
                        .map(|t| (t.start.clone(), t.end.clone()))
                        .collect();
                    functions.push(FormSource::with_spans(
                        format!("{}:{}", id, map.format),
                        map.format.clone(),
                        real_path.clone(),
                        forms,
                        &spans,
                    ));
                }
            }

            // A block with units counts in the statistics of its language
            // even when it is too small for the token passes.
            let has_units = |block_format: &str| functions.iter().any(|f| f.format == block_format);

            // Display path: flat tokenize for the parent SourceFile. The
            // display tokens of a component leave out code its blocks hold,
            // such as Astro's frontmatter, so the file stays when its blocks
            // reach the limit together.
            let tokens = cpd_tokenizer::tokenizer::tokenize(format, content, self.mode);
            let block_tokens: usize = maps.iter().map(|map| map.tokens.len()).sum();
            if tokens.len().max(block_tokens) < self.min_tokens {
                let mut sources = vec![SourceFile {
                    id: id.clone(),
                    format: format.to_string(),
                    tokens,
                    bytes: file_bytes,
                }];
                sources.extend(
                    maps.iter()
                        .filter(|map| map.format != format && has_units(&map.format))
                        .map(|map| block_source(&id, &map.format, &map.tokens)),
                );
                return forms_only(sources, functions);
            }

            let mut source_files = vec![SourceFile {
                id: id.clone(),
                format: format.to_string(),
                tokens,
                bytes: file_bytes,
            }];

            let mut prepared = Vec::new();
            for map in maps {
                let embedded = map.format != format;
                let detected = map.tokens.len() >= self.min_tokens;
                if !detected && !(embedded && has_units(&map.format)) {
                    continue;
                }
                let map_id = format!("{}:{}", id, map.format);
                // For sub-formats, create a synthetic SourceFile with detection
                // tokens converted to display tokens so statistics per-format
                // counts are correct.
                if embedded {
                    source_files.push(block_source(&id, &map.format, &map.tokens));
                }
                if !detected {
                    continue;
                }
                let mut sub =
                    PreparedSource::from_detection_tokens(map_id, map.format, &map.tokens);
                sub.real_path = real_path.clone();
                // Only a different language is embedded; the host's own
                // map covers the file end to end.
                sub.embedded = embedded;
                prepared.push(sub);
            }
            if prepared.is_empty() {
                return forms_only(source_files, functions);
            }
            show_passes(self.passes, format, content, &prepared);
            Some(PreparedText {
                sources: source_files,
                prepared,
                functions,
            })
        } else {
            // Single-format path.
            let forms = match reads_forms {
                true => cpd_similarity::forms(content, format, &opts.ignore_ranges),
                false => Vec::new(),
            };
            let tokens = cpd_tokenizer::tokenizer::tokenize(format, content, self.mode);
            if tokens.len() < self.min_tokens && forms.is_empty() {
                return None;
            }
            let det_tokens = tokenize_to_detection(format, content, &opts);
            let mut functions = Vec::new();
            if !forms.is_empty() {
                let spans: Vec<_> = det_tokens
                    .iter()
                    .map(|t| (t.start.clone(), t.end.clone()))
                    .collect();
                functions.push(FormSource::with_spans(
                    id.clone(),
                    format.to_string(),
                    real_path.clone(),
                    forms,
                    &spans,
                ));
            }
            if tokens.len() < self.min_tokens || det_tokens.len() < self.min_tokens {
                let source = SourceFile {
                    id: id.clone(),
                    format: format.to_string(),
                    tokens,
                    bytes: file_bytes,
                };
                return forms_only(vec![source], functions);
            }

            let source_file = SourceFile {
                id: id.clone(),
                format: format.to_string(),
                tokens,
                bytes: file_bytes,
            };
            let mut prepared =
                PreparedSource::from_detection_tokens(id, format.to_string(), &det_tokens);
            prepared.real_path = real_path;
            let prepared = vec![prepared];
            show_passes(self.passes, &prepared[0].format, content, &prepared);

            Some(PreparedText {
                sources: vec![source_file],
                prepared,
                functions,
            })
        }
    }
}

/// The part of the path `id` below the longest of `roots` it lies in; all
/// of it when it lies in none.
fn below_root<'i>(id: &'i str, roots: &[String]) -> &'i str {
    roots
        .iter()
        .filter_map(|root| {
            let root = root.trim_end_matches(['/', '\\']);
            let rest = id.strip_prefix(root)?;
            match rest.strip_prefix(['/', '\\']) {
                Some(rest) => Some(rest),
                None => rest.is_empty().then_some(rest),
            }
        })
        .min_by_key(|rest| rest.len())
        .filter(|rest| !rest.is_empty())
        .unwrap_or(id)
}

/// What a file too small for the token passes gives: its units, if it has
/// any, and its sources, so the statistics count the lines their clones
/// cover.
fn forms_only(sources: Vec<SourceFile>, functions: Vec<FormSource>) -> Option<PreparedText> {
    (!functions.is_empty()).then(|| PreparedText {
        sources,
        prepared: Vec::new(),
        functions,
    })
}

/// The file a fragment with `source_id` lies in: an embedded block
/// (`<path>:<format>`, such as the script of a component) belongs to its
/// host file. The two fragments of a semantic pair can be in different
/// languages, so the suffix is any format's name, not the clone's, or the
/// name of a block that is no format of its own (`html` for the markup of a
/// component, `text` for a code fence without a language).
pub fn host_file(source_id: &str) -> &str {
    static FORMATS: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    let formats =
        FORMATS.get_or_init(|| cpd_tokenizer::formats::list_formats().into_iter().collect());
    match source_id.rsplit_once(':') {
        Some((host, suffix)) if formats.contains(suffix) || is_block_name(host, suffix) => host,
        _ => source_id,
    }
}

/// Whether `suffix` names a block of the file `host`: a bare name after a
/// file with an extension, unlike the rest of `C:\\p\\a.js` after its drive or
/// a colon inside a file name such as `a:b.js`.
fn is_block_name(host: &str, suffix: &str) -> bool {
    let name = host.rsplit(['/', '\\']).next().unwrap_or(host);
    name.contains('.')
        && !suffix.is_empty()
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '#'))
}

/// The statistics source of a code block in the language `format`: its
/// detection tokens as display tokens.
fn block_source(id: &str, format: &str, tokens: &[DetectionToken]) -> SourceFile {
    SourceFile {
        id: format!("{id}:{format}"),
        format: format.to_string(),
        tokens: tokens
            .iter()
            .map(|dt| cpd_core::models::Token {
                kind: cpd_core::models::TokenKind::Other,
                value: String::new(),
                start: dt.start.clone(),
                end: dt.end.clone(),
            })
            .collect(),
        bytes: 0,
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
    #[test]
    fn host_file_folds_an_embedded_block_into_its_file() {
        assert_eq!(host_file("/p/README.md:javascript"), "/p/README.md");
        assert_eq!(host_file("/p/Form.svelte:typescript"), "/p/Form.svelte");
        assert_eq!(host_file("/p/a.js"), "/p/a.js");
        assert_eq!(host_file(r"C:\p\a.js"), r"C:\p\a.js");
        assert_eq!(host_file("/p/Form.vue:html"), "/p/Form.vue");
        assert_eq!(host_file(r"C:\p\Form.vue:html"), r"C:\p\Form.vue");
        assert_eq!(host_file("/p/notes.md:text"), "/p/notes.md");
        assert_eq!(host_file("/p/a:b.js"), "/p/a:b.js");
    }

    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_path_is_read_below_its_scan_root() {
        let roots = ["projects/tests".to_string(), ".".to_string()];
        assert_eq!(below_root("projects/tests/src/a.py", &roots), "src/a.py");
        assert_eq!(below_root("./tests/test_a.py", &roots), "tests/test_a.py");
        assert_eq!(below_root("other/a.py", &roots), "other/a.py");
        assert_eq!(below_root("projects/tests", &roots), "projects/tests");
    }

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
