// options.rs — normalized runtime configuration for cpd, merged from CLI and config file

use std::collections::HashMap;
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
    pub absolute: bool,
    pub ignore_case: bool,
    pub formats_exts: HashMap<String, Vec<String>>,
    pub formats_names: HashMap<String, Vec<String>>,
    pub skip_local: bool,
    pub no_tips: bool,
    pub silent: bool,
    #[allow(dead_code)]
    pub list: bool,
}

impl Options {
    /// Merge CLI args over config file, with CLI flags taking highest priority.
    pub fn from_cli_and_config(cli: &super::cli::Cli, config: &super::cli::ConfigFile) -> Self {
        let mode_str = if cli.skip_comments {
            "weak".to_string()
        } else {
            cli.mode.clone().or(config.mode.clone()).unwrap_or_else(|| "mild".to_string())
        };
        let mode = mode_str.parse::<Mode>().unwrap_or_default();

        let max_size = cli.max_size.as_deref()
            .or(config.max_size.as_deref())
            .and_then(super::cli::parse_size);

        let formats_exts = cli.formats_exts.as_deref()
            .or(config.formats_exts.as_deref())
            .map(super::cli::parse_format_mappings)
            .unwrap_or_default();

        let formats_names = cli.formats_names.as_deref()
            .or(config.formats_names.as_deref())
            .map(super::cli::parse_format_mappings)
            .unwrap_or_default();

        Self {
            paths: cli.paths.clone(),
            min_tokens: cli.min_tokens.or(config.min_tokens).unwrap_or(50),
            min_lines: cli.min_lines.or(config.min_lines).unwrap_or(5),
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
            reporters: if cli.reporters.is_empty() {
                config.reporters.clone().unwrap_or_else(|| vec!["console".to_string()])
            } else {
                cli.reporters.clone()
            },
            output_dir: cli.output.clone().unwrap_or_else(|| {
                PathBuf::from(config.output.clone().unwrap_or_else(|| "report".to_string()))
            }),
            exit_code: cli.exit_code || config.exit_code.unwrap_or(false),
            threshold: cli.threshold.or(config.threshold),
            blame: cli.blame || config.blame.unwrap_or(false),
            no_gitignore: cli.no_gitignore || config.no_gitignore.unwrap_or(false),
            follow_symlinks: cli.follow_symlinks || config.follow_symlinks.unwrap_or(false),
            max_size,
            workers: cli.workers,
            no_colors: cli.no_colors || config.no_colors.unwrap_or(false),
            absolute: cli.absolute || config.absolute.unwrap_or(false),
            ignore_case: cli.ignore_case || config.ignore_case.unwrap_or(false),
            formats_exts,
            formats_names,
            skip_local: cli.skip_local || config.skip_local.unwrap_or(false),
            no_tips: cli.no_tips || config.no_tips.unwrap_or(false) || std::env::var("CI").is_ok(),
            silent: cli.silent || config.silent.unwrap_or(false),
            list: cli.list,
        }
    }
}
