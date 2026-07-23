// orchestrate.rs

use crate::statistics;
use crate::walker::{WalkConfig, walk};
use cpd_core::detect::{PreparedSource, detect_prepared};
use cpd_core::models::{CpdClone, SourceFile, Statistics};
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
    pub mode: Mode,
    pub formats: Vec<String>,
    pub ignore: Vec<String>,
    pub code_ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub no_gitignore: bool,
    pub follow_symlinks: bool,
    pub skip_local: bool,
    pub blame: bool,
    pub workers: Option<usize>,
    pub ignore_case: bool,
    pub formats_exts: std::collections::HashMap<String, Vec<String>>,
    pub formats_names: std::collections::HashMap<String, Vec<String>>,
    pub pattern: Option<String>,
    /// Format equivalence groups: formats in the same group share one clone
    /// detection pool (`--cross-formats`). Empty = every format is isolated.
    pub cross_formats: Vec<Vec<String>>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            paths: vec![],
            min_tokens: 50,
            min_lines: 5,
            max_lines: None,
            mode: Mode::Mild,
            formats: vec![],
            ignore: vec![],
            code_ignore_patterns: vec![],
            max_size: None,
            no_gitignore: false,
            follow_symlinks: false,
            skip_local: false,
            blame: false,
            workers: None,
            ignore_case: false,
            formats_exts: std::collections::HashMap::new(),
            formats_names: std::collections::HashMap::new(),
            pattern: None,
            cross_formats: vec![],
        }
    }
}

/// Result of a full run.
pub struct RunResult {
    pub clones: Vec<CpdClone>,
    pub statistics: Statistics,
    pub sources: Vec<SourceFile>,
}

#[derive(Debug)]
pub enum FinderError {
    Io(std::io::Error),
    Other(String),
}

impl std::fmt::Display for FinderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Other(s) => write!(f, "Error: {s}"),
        }
    }
}

impl std::error::Error for FinderError {}

impl From<std::io::Error> for FinderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Run the full detection pipeline.
pub fn run(config: &RunConfig) -> Result<RunResult, FinderError> {
    // Build a thread pool with a large stack to survive OXC parsing of deeply-nested
    // JS/TS files (e.g., thousands of chained for-loops with no body). OXC's
    // recursive-descent parser allocates one frame per nesting level; the default
    // 8 MiB thread stack is insufficient for pathological inputs like Bun's
    // `lots-of-for-loop.js`. 64 MiB gives ample headroom while remaining reasonable.
    // A local pool (not build_global) avoids poisoning any caller-owned global pool
    // and can be created unconditionally on every run() call.
    let pool = {
        let mut builder =
            rayon::ThreadPoolBuilder::new().stack_size(64 * 1024 * 1024 /* 64 MiB */);
        if let Some(n) = config.workers {
            builder = builder.num_threads(n);
        }
        builder
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().expect("rayon pool"))
    };

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
    let skip_local = config.skip_local;
    let ignore_case = config.ignore_case;

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
                let f = std::fs::File::open(&file.path).ok()?;
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

                let content = str::from_utf8(&map).ok()?;
                let id = file
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| file.path.clone())
                    .to_string_lossy()
                    .into_owned();

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
                    }];

                    let mut prepared = Vec::new();
                    for map in maps {
                        if map.tokens.len() < min_tokens {
                            continue;
                        }
                        let map_id = format!("{}:{}", &id, &map.format);
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
                            });
                        }
                        prepared.push(PreparedSource::from_detection_tokens(
                            map_id,
                            map.format,
                            &map.tokens,
                        ));
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
                    };

                    let opts = TokenizeOptions {
                        mode,
                        ignore_case,
                        ignore_ranges: code_ranges,
                        code_ignore_regexes: code_ignore_regexes.clone(),
                        strip_types_formats: strip_types_formats.clone(),
                    };
                    let det_tokens = tokenize_to_detection(&file.format, content, &opts);
                    if det_tokens.len() < min_tokens {
                        return None;
                    }

                    let prepared =
                        PreparedSource::from_detection_tokens(id, file.format, &det_tokens);

                    Some((vec![source_file], vec![prepared]))
                }
            })
            .collect()
    });

    let (source_files, prepared_sources): (Vec<SourceFile>, Vec<PreparedSource>) =
        results.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut ss, mut ps), (more_s, more_p)| {
                ss.extend(more_s);
                ps.extend(more_p);
                (ss, ps)
            },
        );

    // 3. Group prepared sources into detection pools (deterministic order).
    let format_groups = build_pools(prepared_sources, &config.cross_formats);

    // 4. Detect clones — skip_local uses scan roots to determine same-directory pairs.
    //    Both scan roots and file IDs must use the same path normalization so
    //    that prefix comparisons work. Canonicalize scan roots once here (resolves
    //    symlinks like macOS /var → /private/var), and canonicalize file paths in
    //    the parallel processing loop above. Fall back to the original path if
    //    canonicalize fails.
    let scan_roots: Vec<std::path::PathBuf> = config
        .paths
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect();
    let clones = pool.install(|| {
        detect_prepared(
            format_groups,
            min_tokens,
            skip_local,
            config.min_lines,
            &scan_roots,
        )
    });

    // 5. Compute statistics.
    let statistics = statistics::compute(&source_files, &clones);

    Ok(RunResult {
        clones,
        statistics,
        sources: source_files,
    })
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
fn strip_types_formats(cross_formats: &[Vec<String>]) -> std::collections::HashSet<String> {
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
