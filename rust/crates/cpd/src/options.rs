// options.rs — normalized runtime configuration for cpd, merged from CLI and config file

use std::path::PathBuf;

use cpd_tokenizer::tokenizer::Mode;

#[derive(Debug, Clone)]
pub struct Options {
    pub paths: Vec<PathBuf>,
    pub min_tokens: usize,
    pub min_lines: usize,
    pub max_lines: Option<usize>,
    pub mode: Mode,
    pub formats: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub reporters: Vec<String>,
    pub output_dir: PathBuf,
    pub exit_code: bool,
    pub threshold: Option<f64>,
    pub blame: bool,
    pub no_gitignore: bool,
    pub follow_symlinks: bool,
    pub max_size: Option<u64>,
    pub workers: Option<usize>,
    pub no_colors: bool,
    pub skip_local: bool,
    #[allow(dead_code)]
    pub list: bool,
}

impl Options {
    /// Merge CLI args over config file, with CLI flags taking highest priority.
    pub fn from_cli_and_config(cli: &super::cli::Cli, config: &super::cli::ConfigFile) -> Self {
        let mode_str = if cli.skip_comments { "weak" } else { &cli.mode };
        let mode = mode_str.parse::<Mode>().unwrap_or_default();

        Self {
            paths: cli.paths.clone(),
            min_tokens: cli.min_tokens,
            min_lines: cli.min_lines,
            max_lines: cli.max_lines.or(config.max_lines),
            mode,
            formats: if cli.format.is_empty() {
                config.format.clone().unwrap_or_default()
            } else {
                cli.format.clone()
            },
            ignore_patterns: if cli.ignore_pattern.is_empty() {
                config.ignore_pattern.clone().unwrap_or_default()
            } else {
                cli.ignore_pattern.clone()
            },
            reporters: cli.reporters.clone(),
            output_dir: cli.output.clone(),
            exit_code: cli.exit_code || config.exit_code.unwrap_or(false),
            threshold: cli.threshold.or(config.threshold),
            blame: cli.blame || config.blame.unwrap_or(false),
            no_gitignore: cli.no_gitignore || config.no_gitignore.unwrap_or(false),
            follow_symlinks: cli.follow_symlinks || config.follow_symlinks.unwrap_or(false),
            max_size: cli.max_size.or(config.max_size),
            workers: cli.workers,
            no_colors: cli.no_colors || config.no_colors.unwrap_or(false),
            skip_local: cli.skip_local || config.skip_local.unwrap_or(false),
            list: cli.list,
        }
    }
}
