// orchestrate.rs

use crate::statistics;
use crate::walker::{WalkConfig, walk};
use cpd_core::detect::{PathFilters, PreparedSource, detect_prepared, merge_gapped_clones};
use cpd_core::models::{CpdClone, KindFilter, SourceFile, Statistics};
use cpd_core::similarity::{FunctionSig, collect_function_sources, find_similar_functions};
use cpd_tokenizer::functions::{extract_functions, supports_functions};
use cpd_tokenizer::tokenizer::{
    Mode, TokenizeOptions, code_ignore_ranges, tokenize_to_detection, tokenize_to_detection_maps,
};
use std::path::PathBuf;

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
    /// Report JS/TS function pairs whose AST similarity reaches this value
    /// as `similar` clones (issue #999, stage 2). `1.0` (the default) means
    /// exact matches only: the pass does not run. See
    /// [`RunConfig::similarity_threshold`].
    pub similarity: f32,
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
}

/// Result of a full run.
pub struct RunResult {
    pub clones: Vec<CpdClone>,
    pub statistics: Statistics,
    pub sources: Vec<SourceFile>,
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
/// It cannot fail; the `Result` keeps the embedding API (`run(&config).unwrap()`) stable.
pub fn run(config: &RunConfig) -> Result<RunResult, std::convert::Infallible> {
    let pool = build_thread_pool(config.workers);

    // 1-2. Walk + tokenize.
    let PreparedScan {
        sources: source_files,
        prepared: prepared_sources,
    } = prepare_scan_in(&pool, config);

    // Function signatures must be taken before the pools consume the
    // prepared sources; empty unless --similarity is set.
    let function_sources = collect_function_sources(&prepared_sources);

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
    let clones = pool.install(|| {
        detect_prepared(
            format_groups,
            config.min_tokens,
            config.min_lines,
            &PathFilters {
                skip_local: config.skip_local,
                scan_roots: &scan_roots,
                isolated_groups: &isolated_groups,
            },
        )
    });

    // 4b. Near-miss merging — a no-op unless --max-gap-lines is set.
    let mut clones = merge_gapped_clones(clones, config.max_gap_lines);

    // 4c. Function-level similarity — only when --similarity is set.
    if let Some(threshold) = config.similarity_threshold() {
        let similar = find_similar_functions(
            function_sources,
            threshold,
            config.min_tokens,
            config.min_lines,
            &clones,
        );
        clones.extend(similar);
    }

    // 4d. --kind: drop the kinds nobody asked for.
    if !config.kinds.is_empty() {
        clones.retain(|clone| config.kinds.iter().any(|kind| kind.matches(clone)));
    }

    // 5. Compute statistics.
    let statistics = statistics::compute(&source_files, &clones);

    Ok(RunResult {
        clones,
        statistics,
        sources: source_files,
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
    // 1. Walk files
    let walk_config = WalkConfig {
        paths: config.paths.clone(),
        extensions: config.formats.clone(),
        ignore_patterns: config.ignore.clone(),
        max_size: config.max_size,
        follow_symlinks: config.follow_symlinks,
        no_gitignore: config.no_gitignore,
        formats_exts: config.formats_exts.clone(),
        formats_names: config.formats_names.clone(),
        pattern: config.pattern.clone(),
    };
    let discovered = walk(&walk_config);

    // 2. Read + tokenize files in parallel.
    //    - Display path: produce Vec<Token> for SourceFile (used by reporters).
    //    - Detection path: produce Vec<DetectionToken> via tokenize_to_detection
    //      (filtered + hashed at tokenize time, never stored in SourceFile).
    //    - Multi-format files (markdown) produce multiple TokenMaps, one per
    //      embedded sub-language, so embedded code blocks join the correct pool.
    use rayon::prelude::*;
    let mode = config.mode;
    let min_tokens = config.min_tokens;
    let min_lines = config.min_lines;
    let max_lines = config.max_lines;
    let ignore_case = config.ignore_case;
    let ignore_identifiers = config.ignore_identifiers;
    let ignore_literals = config.ignore_literals;
    let ignore_annotations = config.ignore_annotations;
    let want_functions = config.similarity_threshold().is_some();

    // Pre-compile code-level ignore regex patterns once for all threads.
    // Invalid patterns are silently skipped.
    let code_ignore_regexes: Vec<regex::Regex> = config
        .code_ignore_patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    let strip_types_formats = strip_types_formats(&config.cross_formats);

    const MULTI_FORMAT_EXTS: &[&str] = &["md", "markdown", "mkd", "vue", "svelte", "astro"];

    fn is_multi_format(format: &str) -> bool {
        MULTI_FORMAT_EXTS.contains(&format)
    }

    let results: Vec<(Vec<SourceFile>, Vec<PreparedSource>)> = pool.install(|| {
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
                if min_lines > 0 || max_lines.is_some() {
                    let newlines = memchr::Memchr::new(b'\n', &map).count();
                    let lc = if !map.is_empty() && *map.last().unwrap() != b'\n' {
                        newlines + 1
                    } else {
                        newlines
                    };
                    if lc < min_lines {
                        return None;
                    }
                    if max_lines.is_some_and(|m| lc > m) {
                        return None;
                    }
                }

                let file_bytes = map.len() as u64;
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

                // Compute code-level ignore ranges from regex matches against source text.
                // This matches v4 semantics: regex patterns are matched against source
                // text, and any token overlapping a match range is skipped during detection.
                let code_ranges = if code_ignore_regexes.is_empty() {
                    Vec::new()
                } else {
                    code_ignore_ranges(content, &code_ignore_regexes)
                };

                if is_multi_format(&file.format) {
                    // Multi-format path: produce one PreparedSource per sub-format.
                    let opts = TokenizeOptions {
                        mode,
                        ignore_case,
                        ignore_identifiers,
                        ignore_literals,
                        ignore_annotations,
                        ignore_ranges: code_ranges,
                        code_ignore_regexes: code_ignore_regexes.clone(),
                        strip_types_formats: strip_types_formats.clone(),
                    };
                    let maps = tokenize_to_detection_maps(&file.format, content, &opts);

                    // Display path: flat tokenize for the parent SourceFile.
                    let tokens = cpd_tokenizer::tokenizer::tokenize(&file.format, content, mode);
                    if tokens.len() < min_tokens {
                        return None;
                    }

                    let mut source_files = vec![SourceFile {
                        id: id.clone(),
                        format: file.format.clone(),
                        tokens,
                        bytes: file_bytes,
                    }];

                    let mut prepared = Vec::new();
                    for map in maps {
                        if map.tokens.len() < min_tokens {
                            continue;
                        }
                        let map_id = format!("{}:{}", id, map.format);
                        // For sub-formats, create a synthetic SourceFile with detection
                        // tokens converted to display tokens so statistics per-format
                        // counts are correct.
                        if map.format != file.format {
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
                        let embedded = map.format != file.format;
                        let mut sub =
                            PreparedSource::from_detection_tokens(map_id, map.format, &map.tokens);
                        sub.real_path = real_path.clone();
                        // Only a different language is embedded; the host's own
                        // map covers the file end to end.
                        sub.embedded = embedded;
                        prepared.push(sub);
                    }
                    if prepared.is_empty() {
                        return None;
                    }
                    Some((source_files, prepared))
                } else {
                    // Single-format path.
                    let tokens = cpd_tokenizer::tokenizer::tokenize(&file.format, content, mode);
                    if tokens.len() < min_tokens {
                        return None;
                    }

                    let source_file = SourceFile {
                        id: id.clone(),
                        format: file.format.clone(),
                        tokens,
                        bytes: file_bytes,
                    };

                    let opts = TokenizeOptions {
                        mode,
                        ignore_case,
                        ignore_identifiers,
                        ignore_literals,
                        ignore_annotations,
                        ignore_ranges: code_ranges,
                        code_ignore_regexes: code_ignore_regexes.clone(),
                        strip_types_formats: strip_types_formats.clone(),
                    };
                    let det_tokens = tokenize_to_detection(&file.format, content, &opts);
                    if det_tokens.len() < min_tokens {
                        return None;
                    }

                    let mut prepared =
                        PreparedSource::from_detection_tokens(id, file.format, &det_tokens);
                    prepared.real_path = real_path;
                    if want_functions && supports_functions(&prepared.format) {
                        prepared.functions = extract_functions(content, &prepared.format)
                            .into_iter()
                            .filter_map(|f| {
                                FunctionSig::build(
                                    f.grammar,
                                    f.name,
                                    f.start,
                                    f.end,
                                    &f.kinds,
                                    &prepared.spans,
                                )
                            })
                            .collect();
                    }

                    Some((vec![source_file], vec![prepared]))
                }
            })
            .collect()
    });

    let (sources, prepared): (Vec<SourceFile>, Vec<PreparedSource>) = results.into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut ss, mut ps), (more_s, more_p)| {
            ss.extend(more_s);
            ps.extend(more_p);
            (ss, ps)
        },
    );

    PreparedScan { sources, prepared }
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
        Some(idx) => format!("cross:{idx:04}"),
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
            functions: Vec::new(),
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
