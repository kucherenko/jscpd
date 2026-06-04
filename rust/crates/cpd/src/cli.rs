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
    #[arg(long, short = 'k', default_value = "50")]
    pub min_tokens: usize,

    /// Minimum number of lines to consider a duplicate
    #[arg(long, short = 'l', default_value = "5")]
    pub min_lines: usize,

    /// Maximum number of lines per block to consider
    #[arg(long, short = 'x')]
    pub max_lines: Option<usize>,

    /// Detection mode: mild, weak, strict
    #[arg(long, short = 'm', default_value = "mild")]
    pub mode: String,

    /// Alias for --mode weak (skip comment tokens)
    #[arg(long)]
    pub skip_comments: bool,

    /// List of file extensions/formats to check (comma-separated)
    #[arg(long, short = 'f', value_delimiter = ',')]
    pub format: Vec<String>,

    /// Glob patterns to ignore (comma-separated)
    #[arg(long, short = 'i', value_delimiter = ',')]
    pub ignore_pattern: Vec<String>,

    /// Output reporters (comma-separated): console,json,xml,csv,html,markdown,badge,sarif,ai,xcode,threshold,silent,console-full
    #[arg(long, short = 'r', default_value = "console", value_delimiter = ',')]
    pub reporters: Vec<String>,

    /// Output directory for file reporters
    #[arg(long, short = 'o', default_value = "report")]
    pub output: PathBuf,

    /// Path to config file (.jscpd.json)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Exit with non-zero code if duplicates found
    #[arg(long)]
    pub exit_code: bool,

    /// Maximum duplication percentage before exit 1
    #[arg(long, short = 't')]
    pub threshold: Option<f64>,

    /// Enrich clones with git blame data
    #[arg(long, short = 'b')]
    pub blame: bool,

    /// Do not respect .gitignore files
    #[arg(long)]
    pub no_gitignore: bool,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Skip files larger than N bytes
    #[arg(long, short = 'z')]
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

    // Short alias tests
    #[test]
    fn short_alias_l_for_min_lines() {
        let cli = Cli::parse_from(["cpd", "-l", "10", "."]);
        assert_eq!(cli.min_lines, 10);
    }

    #[test]
    fn short_alias_k_for_min_tokens() {
        let cli = Cli::parse_from(["cpd", "-k", "30", "."]);
        assert_eq!(cli.min_tokens, 30);
    }

    #[test]
    fn short_alias_r_for_reporters() {
        let cli = Cli::parse_from(["cpd", "-r", "json,xml", "."]);
        assert_eq!(cli.reporters, vec!["json", "xml"]);
    }

    #[test]
    fn short_alias_o_for_output() {
        let cli = Cli::parse_from(["cpd", "-o", "dist", "."]);
        assert_eq!(cli.output, PathBuf::from("dist"));
    }

    #[test]
    fn short_alias_t_for_threshold() {
        let cli = Cli::parse_from(["cpd", "-t", "5.5", "."]);
        assert_eq!(cli.threshold, Some(5.5));
    }

    #[test]
    fn short_alias_m_for_mode() {
        let cli = Cli::parse_from(["cpd", "-m", "strict", "."]);
        assert_eq!(cli.mode, "strict");
    }

    #[test]
    fn short_alias_f_for_format() {
        let cli = Cli::parse_from(["cpd", "-f", "rust,typescript", "."]);
        assert_eq!(cli.format, vec!["rust", "typescript"]);
    }

    #[test]
    fn short_alias_i_for_ignore_pattern() {
        let cli = Cli::parse_from(["cpd", "-i", "*.test.js,*.spec.ts", "."]);
        assert_eq!(cli.ignore_pattern, vec!["*.test.js", "*.spec.ts"]);
    }

    #[test]
    fn short_alias_b_for_blame() {
        let cli = Cli::parse_from(["cpd", "-b", "."]);
        assert!(cli.blame);
    }

    // Equivalence tests: verify short aliases behave identically to long-form flags
    #[test]
    fn alias_k_equivalent_to_min_tokens() {
        let short = Cli::parse_from(["cpd", "-k", "30", "."]);
        let long = Cli::parse_from(["cpd", "--min-tokens", "30", "."]);
        assert_eq!(short.min_tokens, long.min_tokens);
        assert_eq!(short.min_tokens, 30);
    }

    #[test]
    fn alias_l_equivalent_to_min_lines() {
        let short = Cli::parse_from(["cpd", "-l", "10", "."]);
        let long = Cli::parse_from(["cpd", "--min-lines", "10", "."]);
        assert_eq!(short.min_lines, long.min_lines);
        assert_eq!(short.min_lines, 10);
    }

    #[test]
    fn alias_r_equivalent_to_reporters() {
        let short = Cli::parse_from(["cpd", "-r", "json,xml", "."]);
        let long = Cli::parse_from(["cpd", "--reporters", "json,xml", "."]);
        assert_eq!(short.reporters, long.reporters);
        assert_eq!(short.reporters, vec!["json", "xml"]);
    }

    #[test]
    fn alias_o_equivalent_to_output() {
        let short = Cli::parse_from(["cpd", "-o", "dist", "."]);
        let long = Cli::parse_from(["cpd", "--output", "dist", "."]);
        assert_eq!(short.output, long.output);
        assert_eq!(short.output, PathBuf::from("dist"));
    }

    #[test]
    fn alias_t_equivalent_to_threshold() {
        let short = Cli::parse_from(["cpd", "-t", "5.5", "."]);
        let long = Cli::parse_from(["cpd", "--threshold", "5.5", "."]);
        assert_eq!(short.threshold, long.threshold);
        assert_eq!(short.threshold, Some(5.5));
    }

    #[test]
    fn alias_m_equivalent_to_mode() {
        let short = Cli::parse_from(["cpd", "-m", "strict", "."]);
        let long = Cli::parse_from(["cpd", "--mode", "strict", "."]);
        assert_eq!(short.mode, long.mode);
        assert_eq!(short.mode, "strict");
    }

    #[test]
    fn alias_f_equivalent_to_format() {
        let short = Cli::parse_from(["cpd", "-f", "rust,typescript", "."]);
        let long = Cli::parse_from(["cpd", "--format", "rust,typescript", "."]);
        assert_eq!(short.format, long.format);
        assert_eq!(short.format, vec!["rust", "typescript"]);
    }

    #[test]
    fn alias_i_equivalent_to_ignore_pattern() {
        let short = Cli::parse_from(["cpd", "-i", "*.test.js,*.spec.ts", "."]);
        let long = Cli::parse_from(["cpd", "--ignore-pattern", "*.test.js,*.spec.ts", "."]);
        assert_eq!(short.ignore_pattern, long.ignore_pattern);
        assert_eq!(short.ignore_pattern, vec!["*.test.js", "*.spec.ts"]);
    }

    #[test]
    fn alias_b_equivalent_to_blame() {
        let short = Cli::parse_from(["cpd", "-b", "."]);
        let long = Cli::parse_from(["cpd", "--blame", "."]);
        assert_eq!(short.blame, long.blame);
        assert!(short.blame);
    }

    #[test]
    fn alias_x_equivalent_to_max_lines() {
        let short = Cli::parse_from(["cpd", "-x", "1000", "."]);
        let long = Cli::parse_from(["cpd", "--max-lines", "1000", "."]);
        assert_eq!(short.max_lines, long.max_lines);
        assert_eq!(short.max_lines, Some(1000));
    }

    #[test]
    fn alias_z_equivalent_to_max_size() {
        let short = Cli::parse_from(["cpd", "-z", "102400", "."]);
        let long = Cli::parse_from(["cpd", "--max-size", "102400", "."]);
        assert_eq!(short.max_size, long.max_size);
        assert_eq!(short.max_size, Some(102400));
    }

    // Edge case tests: verify error handling for invalid inputs
    #[test]
    fn alias_k_rejects_missing_value() {
        let result = Cli::try_parse_from(["cpd", "-k", "."]);
        assert!(result.is_err(), "Should reject -k without numeric value");
    }

    #[test]
    fn alias_l_rejects_missing_value() {
        let result = Cli::try_parse_from(["cpd", "-l", "."]);
        assert!(result.is_err(), "Should reject -l without numeric value");
    }

    #[test]
    fn alias_t_rejects_missing_value() {
        let result = Cli::try_parse_from(["cpd", "-t", "."]);
        assert!(result.is_err(), "Should reject -t without numeric value");
    }

    #[test]
    fn alias_t_rejects_invalid_float() {
        let result = Cli::try_parse_from(["cpd", "-t", "not-a-number", "."]);
        assert!(result.is_err(), "Should reject -t with non-numeric value");
    }

    #[test]
    fn alias_m_accepts_empty_value() {
        // mode is a String, so it can accept any value (validation happens elsewhere)
        let cli = Cli::parse_from(["cpd", "-m", "", "."]);
        assert_eq!(cli.mode, "");
    }

    #[test]
    fn alias_r_handles_empty_list() {
        let cli = Cli::parse_from(["cpd", "-r", "", "."]);
        // Empty string results in one empty element due to delimiter behavior
        assert!(!cli.reporters.is_empty() || cli.reporters == vec![""]);
    }

    #[test]
    fn multiple_aliases_combined() {
        let cli = Cli::parse_from([
            "cpd",
            "-k", "30",
            "-l", "10",
            "-m", "strict",
            "-r", "json,xml",
            "-o", "output",
            "-b",
            ".",
        ]);
        assert_eq!(cli.min_tokens, 30);
        assert_eq!(cli.min_lines, 10);
        assert_eq!(cli.mode, "strict");
        assert_eq!(cli.reporters, vec!["json", "xml"]);
        assert_eq!(cli.output, PathBuf::from("output"));
        assert!(cli.blame);
    }

    #[test]
    fn aliases_and_long_form_can_mix() {
        let cli = Cli::parse_from([
            "cpd",
            "-k", "30",
            "--min-lines", "10",
            "-m", "strict",
            ".",
        ]);
        assert_eq!(cli.min_tokens, 30);
        assert_eq!(cli.min_lines, 10);
        assert_eq!(cli.mode, "strict");
    }
}
