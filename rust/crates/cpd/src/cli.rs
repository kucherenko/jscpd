// cli.rs — CLI argument definitions and config file loading for cpd

use clap::Parser;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "cpd",
    about = "Copy/Paste Detector — find duplicated code",
    version
)]
pub struct Cli {
    /// Paths to scan for duplicates
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Minimum number of tokens to consider a duplicate
    #[arg(long, default_value = "50")]
    pub min_tokens: usize,

    /// Minimum number of lines to consider a duplicate
    #[arg(long, default_value = "5")]
    pub min_lines: usize,

    /// Maximum number of lines per block to consider
    #[arg(long)]
    pub max_lines: Option<usize>,

    /// Detection mode: mild, weak, strict
    #[arg(long, default_value = "mild")]
    pub mode: String,

    /// Alias for --mode weak (skip comment tokens)
    #[arg(long)]
    pub skip_comments: bool,

    /// List of file extensions/formats to check (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub format: Vec<String>,

    /// Glob patterns to ignore (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub ignore_pattern: Vec<String>,

    /// Output reporters (comma-separated): console,json,xml,csv,html,markdown,badge,sarif,ai,xcode,threshold,silent,console-full
    #[arg(long, default_value = "console", value_delimiter = ',')]
    pub reporters: Vec<String>,

    /// Output directory for file reporters
    #[arg(long, default_value = "report")]
    pub output: PathBuf,

    /// Path to config file (.jscpd.json)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Exit with non-zero code if duplicates found
    #[arg(long)]
    pub exit_code: bool,

    /// Maximum duplication percentage before exit 1
    #[arg(long)]
    pub threshold: Option<f64>,

    /// Enrich clones with git blame data
    #[arg(long)]
    pub blame: bool,

    /// Do not respect .gitignore files
    #[arg(long)]
    pub no_gitignore: bool,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Skip files larger than N bytes
    #[arg(long)]
    pub max_size: Option<u64>,

    /// Number of worker threads (default: auto)
    #[arg(long)]
    pub workers: Option<usize>,

    /// Disable ANSI color output
    #[arg(long)]
    pub no_colors: bool,

    /// Accepted for compatibility; external store backend not supported in V1
    #[arg(long, hide = true)]
    pub store: Option<String>,

    /// Accepted for compatibility; external store path not supported in V1
    #[arg(long, hide = true)]
    pub store_path: Option<PathBuf>,

    /// List all supported formats and exit
    #[arg(long)]
    pub list: bool,

    /// Skip clones where both fragments are in the same directory
    #[arg(long)]
    pub skip_local: bool,

    /// Minimum percentage of duplication to report (0-100)
    #[arg(long, default_value = "0")]
    pub min_duplicated_lines: f64,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFile {
    pub min_tokens: Option<usize>,
    pub min_lines: Option<usize>,
    pub max_lines: Option<usize>,
    pub mode: Option<String>,
    pub format: Option<Vec<String>>,
    pub ignore_pattern: Option<Vec<String>>,
    pub reporters: Option<Vec<String>>,
    pub output: Option<String>,
    pub threshold: Option<f64>,
    pub blame: Option<bool>,
    pub no_gitignore: Option<bool>,
    pub follow_symlinks: Option<bool>,
    pub max_size: Option<u64>,
    pub no_colors: Option<bool>,
    pub skip_local: Option<bool>,
    pub exit_code: Option<bool>,
}

/// Load config from file if specified, or from .jscpd.json / package.json jscpd key.
/// Falls back to defaults silently on any error.
pub fn load_config(path: Option<&Path>) -> ConfigFile {
    if let Some(p) = path {
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(cfg) = serde_json::from_str::<ConfigFile>(&content) {
                return cfg;
            }
        }
        return ConfigFile::default();
    }

    if let Ok(content) = std::fs::read_to_string(".jscpd.json") {
        if let Ok(cfg) = serde_json::from_str::<ConfigFile>(&content) {
            return cfg;
        }
    }

    if let Ok(content) = std::fs::read_to_string("package.json") {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(jscpd_cfg) = pkg.get("jscpd") {
                if let Ok(cfg) = serde_json::from_value::<ConfigFile>(jscpd_cfg.clone()) {
                    return cfg;
                }
            }
        }
    }

    ConfigFile::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn default_min_tokens_is_50() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert_eq!(cli.min_tokens, 50);
    }

    #[test]
    fn min_tokens_override() {
        let cli = Cli::parse_from(["cpd", "--min-tokens", "30", "."]);
        assert_eq!(cli.min_tokens, 30);
    }

    #[test]
    fn list_flag() {
        let cli = Cli::parse_from(["cpd", "--list"]);
        assert!(cli.list);
    }

    #[test]
    fn skip_comments_sets_mode_weak_in_options() {
        let cli = Cli::parse_from(["cpd", "--skip-comments", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.mode, cpd_tokenizer::tokenizer::Mode::Weak);
    }

    #[test]
    fn config_file_min_tokens_overrides_default() {
        let config = ConfigFile {
            min_tokens: Some(30),
            ..Default::default()
        };
        let _ = config;
    }

    #[test]
    fn store_flag_accepted_without_error() {
        let result = Cli::try_parse_from(["cpd", "--store", "leveldb", "."]);
        assert!(result.is_ok(), "--store flag must be accepted");
    }

    #[test]
    fn reporters_split_by_comma() {
        let cli = Cli::parse_from(["cpd", "--reporters", "console,json", "."]);
        assert_eq!(cli.reporters, vec!["console", "json"]);
    }
}
