// orchestrate.rs
// Attribution: detection orchestration pipeline; inspired by jscpd-rs approach; rewritten independently.

use std::path::PathBuf;
use cpd_core::detect::{detect_prepared, PreparedSource};
use cpd_core::models::{CpdClone, SourceFile, Statistics};
use cpd_tokenizer::tokenizer::{Mode, TokenizeOptions, tokenize_to_detection, tokenize_to_detection_maps};
use crate::walker::{walk, WalkConfig};
use crate::statistics;

/// Full run configuration.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub paths: Vec<PathBuf>,
    pub min_tokens: usize,
    pub min_lines: usize,
    pub max_lines: Option<usize>,
    pub mode: Mode,
    pub formats: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub no_gitignore: bool,
    pub follow_symlinks: bool,
    pub skip_local: bool,
    pub blame: bool,
    pub workers: Option<usize>,
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
            ignore_patterns: vec![],
            max_size: Some(512 * 1024),
            no_gitignore: false,
            follow_symlinks: false,
            skip_local: false,
            blame: false,
            workers: None,
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
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

/// Run the full detection pipeline.
pub fn run(config: &RunConfig) -> Result<RunResult, FinderError> {
    // Configure rayon thread pool if workers specified; ignore if already initialized.
    if let Some(n) = config.workers {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    // 1. Walk files
    let walk_config = WalkConfig {
        paths: config.paths.clone(),
        extensions: config.formats.clone(),
        ignore_patterns: config.ignore_patterns.clone(),
        max_size: config.max_size,
        min_lines: if config.min_lines > 0 { Some(config.min_lines) } else { None },
        max_lines: config.max_lines,
        follow_symlinks: config.follow_symlinks,
        no_gitignore: config.no_gitignore,
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
    let skip_local = config.skip_local;

    /// Multi-format extensions that need cross-format tokenization.
    const MULTI_FORMAT_EXTS: &[&str] = &["md", "markdown", "mkd", "vue", "svelte", "astro"];

    fn is_multi_format(format: &str) -> bool {
        MULTI_FORMAT_EXTS.iter().any(|&ext| format == ext)
            || format == "markdown"
    }

    let results: Vec<Option<(Vec<SourceFile>, Vec<PreparedSource>)>> = discovered.par_iter()
        .map(|file| {
            let content = std::fs::read_to_string(&file.path).ok()?;
            let id = file.path.to_string_lossy().into_owned();

            if is_multi_format(&file.format) {
                // Multi-format path: produce one PreparedSource per sub-format.
                let opts = TokenizeOptions::new(mode);
                let maps = tokenize_to_detection_maps(&file.format, &content, &opts);

                // Display path: flat tokenize for the parent SourceFile.
                let tokens = cpd_tokenizer::tokenizer::tokenize(&file.format, &content, mode);
                if tokens.len() < min_tokens { return None; }

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
                        let synth_tokens: Vec<cpd_core::models::Token> = map.tokens.iter().map(|dt| {
                            cpd_core::models::Token {
                                kind: cpd_core::models::TokenKind::Other,
                                value: String::new(),
                                start: dt.start.clone(),
                                end: dt.end.clone(),
                            }
                        }).collect();
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
                if prepared.is_empty() { return None; }
                Some((source_files, prepared))
            } else {
                // Single-format path.
                let tokens = cpd_tokenizer::tokenizer::tokenize(&file.format, &content, mode);
                if tokens.len() < min_tokens { return None; }

                let source_file = SourceFile {
                    id: id.clone(),
                    format: file.format.clone(),
                    tokens,
                };

                let opts = TokenizeOptions::new(mode);
                let det_tokens = tokenize_to_detection(&file.format, &content, &opts);
                if det_tokens.len() < min_tokens { return None; }

                let prepared = PreparedSource::from_detection_tokens(
                    id,
                    file.format.clone(),
                    &det_tokens,
                );

                Some((vec![source_file], vec![prepared]))
            }
        })
        .collect();

    let (source_files, mut prepared_sources): (Vec<SourceFile>, Vec<PreparedSource>) = results
        .into_iter()
        .filter_map(|opt| opt)
        .fold((Vec::new(), Vec::new()), |(mut ss, mut ps), (more_s, more_p)| {
            ss.extend(more_s);
            ps.extend(more_p);
            (ss, ps)
        });

    // 3. Group prepared sources by format (deterministic order).
    prepared_sources.sort_unstable_by(|a, b| {
        a.format.cmp(&b.format).then(a.id.cmp(&b.id))
    });

    let mut format_map: std::collections::HashMap<String, Vec<PreparedSource>> = std::collections::HashMap::default();
    for ps in prepared_sources {
        format_map.entry(ps.format.clone()).or_default().push(ps);
    }
    let mut format_groups: Vec<Vec<PreparedSource>> = format_map.into_values().collect();
    // Sort groups by format name for determinism.
    format_groups.sort_by(|a, b| a[0].format.cmp(&b[0].format));

    // 4. Detect clones — skip_local is now handled inside flush_clone.
    let clones = detect_prepared(format_groups, min_tokens, skip_local, config.min_lines);

    // 5. Compute statistics.
    let statistics = statistics::compute(&source_files, &clones);

    Ok(RunResult { clones, statistics, sources: source_files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        if !fixtures.exists() { return; }

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

        assert_eq!(r1.sources.len(), r2.sources.len(),
            "--workers 1 must produce same source count as default");
    }
}
