// orchestrate.rs
// Attribution: detection orchestration pipeline; inspired by jscpd-rs approach; rewritten independently.

use std::path::PathBuf;
use cpd_core::models::{CpdClone, SourceFile, Statistics};
use crate::walker::{walk, WalkConfig};
use crate::statistics;
use crate::skip_local;

/// Full run configuration.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub paths: Vec<PathBuf>,
    pub min_tokens: usize,
    pub min_lines: usize,
    pub max_lines: Option<usize>,
    pub mode: cpd_tokenizer::tokenizer::Mode,
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
            mode: cpd_tokenizer::tokenizer::Mode::Mild,
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

    // 2. Read + tokenize files in parallel
    use rayon::prelude::*;
    let mode = config.mode;
    let min_tokens = config.min_tokens;
    let source_files: Vec<SourceFile> = discovered.par_iter()
        .filter_map(|file| {
            let content = std::fs::read_to_string(&file.path).ok()?;
            let tokens = cpd_tokenizer::tokenizer::tokenize(&file.format, &content, mode);
            if tokens.len() < min_tokens { return None; }
            Some(SourceFile {
                id: file.path.to_string_lossy().into_owned(),
                format: file.format.clone(),
                tokens,
            })
        })
        .collect();

    // 3. Detect clones
    let mut clones = cpd_core::detect::detect(&source_files, config.min_tokens);

    // 4. Apply skip-local filter
    if config.skip_local {
        clones = skip_local::apply_skip_local(clones);
    }

    // 5. Compute statistics
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
