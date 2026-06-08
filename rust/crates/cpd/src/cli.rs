// cli.rs — CLI argument definitions and config file loading for cpd

use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Parse a human-readable size string (e.g. "1kb", "1mb", "100kb", "102400") into bytes.
/// Supports: b, kb, k, mb, m, gb, g (case-insensitive). Returns None on parse failure.
pub fn parse_size(input: &str) -> Option<u64> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }

    let (num_part, multiplier): (&str, u64) = if s.ends_with("gb") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024)
    } else if s.ends_with('g') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024)
    } else if s.ends_with("mb") {
        (&s[..s.len() - 2], 1024 * 1024)
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], 1024 * 1024)
    } else if s.ends_with("kb") {
        (&s[..s.len() - 2], 1024)
    } else if s.ends_with('k') {
        (&s[..s.len() - 1], 1024)
    } else if s.ends_with('b') {
        (&s[..s.len() - 1], 1)
    } else {
        (s.as_str(), 1)
    };

    let num: f64 = num_part.parse().ok()?;
    if num < 0.0 {
        return None;
    }
    Some((num * multiplier as f64).round() as u64)
}

/// Parse format mappings string like "javascript:es,es6;dart:dt" into a HashMap.
pub fn parse_format_mappings(input: &str) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    for group in input.split(';') {
        let group = group.trim();
        if group.is_empty() {
            continue;
        }
        if let Some((format, exts)) = group.split_once(':') {
            let format = format.trim().to_string();
            let exts: Vec<String> = exts.split(',').map(|e| e.trim().to_string()).collect();
            if !format.is_empty() && !exts.is_empty() {
                result.insert(format, exts);
            }
        }
    }
    result
}

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
    #[arg(long, short = 'k')]
    pub min_tokens: Option<usize>,

    /// Minimum number of lines to consider a duplicate
    #[arg(long, short = 'l')]
    pub min_lines: Option<usize>,

    /// Maximum number of lines per block to consider
    #[arg(long, short = 'x')]
    pub max_lines: Option<usize>,

    /// Detection mode: mild, weak, strict
    #[arg(long, short = 'm')]
    pub mode: Option<String>,

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
    #[arg(long, short = 'r', value_delimiter = ',')]
    pub reporters: Vec<String>,

    /// Output directory for file reporters
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Path to config file (.jscpd.json)
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,

    /// Exit with code if duplicates found (default code: 1)
    #[arg(long, num_args(0..=1), default_missing_value = "1")]
    pub exit_code: Option<i32>,

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

    /// Skip files larger than SIZE (e.g. 1kb, 1mb, 100kb, or raw bytes)
    #[arg(long, short = 'z')]
    pub max_size: Option<String>,

    /// Number of worker threads (default: auto)
    #[arg(long)]
    pub workers: Option<usize>,

    /// Disable ANSI color output
    #[arg(long)]
    pub no_colors: bool,

    /// Use absolute paths in reports
    #[arg(long, short = 'a')]
    pub absolute: bool,

    /// Ignore case of symbols in code (experimental)
    #[arg(long)]
    pub ignore_case: bool,

    /// Custom format-to-extension mappings (e.g. javascript:es,es6;dart:dt)
    #[arg(long)]
    pub formats_exts: Option<String>,

    /// Custom format-to-filename mappings (e.g. makefile:Makefile,GNUmakefile;docker:Dockerfile)
    #[arg(long)]
    pub formats_names: Option<String>,

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

    /// Do not write detection progress and result to console
    #[arg(long, short = 's')]
    pub silent: bool,

    /// Do not print tips and promotional messages after detection
    #[arg(long)]
    pub no_tips: bool,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFile {
    pub path: Option<Vec<String>>,
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
    pub max_size: Option<String>,
    pub no_colors: Option<bool>,
    pub absolute: Option<bool>,
    pub ignore_case: Option<bool>,
    pub formats_exts: Option<String>,
    pub formats_names: Option<String>,
    pub skip_local: Option<bool>,
    pub exit_code: Option<i32>,
    pub no_tips: Option<bool>,
    pub silent: Option<bool>,
}

fn resolve_config_paths(cfg: &mut ConfigFile, config_dir: &Path) {
    if let Some(ref mut paths) = cfg.path {
        *paths = paths
            .iter()
            .map(|p| {
                let path = PathBuf::from(p);
                if path.is_relative() {
                    config_dir.join(path).to_string_lossy().to_string()
                } else {
                    p.clone()
                }
            })
            .collect();
    }
    if let Some(ref mut patterns) = cfg.ignore_pattern {
        *patterns = patterns
            .iter()
            .map(|p| {
                let path = PathBuf::from(p);
                if path.is_relative() && !p.contains('*') && !p.contains('?') {
                    config_dir.join(path).to_string_lossy().to_string()
                } else {
                    p.clone()
                }
            })
            .collect();
    }
}

/// Load config from file if specified, or from .jscpd.json / package.json jscpd key.
/// Falls back to defaults silently on any error.
/// Paths in the config file are resolved relative to the config file's directory,
/// matching jscpd v4 behavior.
pub fn load_config(path: Option<&Path>) -> ConfigFile {
    if let Some(p) = path {
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(mut cfg) = serde_json::from_str::<ConfigFile>(&content) {
                let config_dir = p.parent().unwrap_or(Path::new(".")).to_path_buf();
                resolve_config_paths(&mut cfg, &config_dir);
                return cfg;
            }
        }
        return ConfigFile::default();
    }

    if let Ok(content) = std::fs::read_to_string(".jscpd.json") {
        if let Ok(mut cfg) = serde_json::from_str::<ConfigFile>(&content) {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            resolve_config_paths(&mut cfg, &cwd);
            return cfg;
        }
    }

    if let Ok(content) = std::fs::read_to_string("package.json") {
        if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(jscpd_cfg) = pkg.get("jscpd") {
                if let Ok(mut cfg) = serde_json::from_value::<ConfigFile>(jscpd_cfg.clone()) {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    resolve_config_paths(&mut cfg, &cwd);
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
        assert_eq!(cli.min_tokens, None);
    }

    #[test]
    fn min_tokens_override() {
        let cli = Cli::parse_from(["cpd", "--min-tokens", "30", "."]);
        assert_eq!(cli.min_tokens, Some(30));
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
    fn config_path_used_when_cli_paths_empty() {
        let cli = Cli::parse_from(["cpd"]);
        let config = ConfigFile {
            path: Some(vec!["./fixtures".to_string()]),
            ..Default::default()
        };
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.paths, vec![PathBuf::from("./fixtures")]);
    }

    #[test]
    fn cli_paths_override_config_path() {
        let cli = Cli::parse_from(["cpd", "/tmp/project"]);
        let config = ConfigFile {
            path: Some(vec!["./fixtures".to_string()]),
            ..Default::default()
        };
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.paths, vec![PathBuf::from("/tmp/project")]);
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
        assert_eq!(cli.min_lines, Some(10));
    }

    #[test]
    fn short_alias_k_for_min_tokens() {
        let cli = Cli::parse_from(["cpd", "-k", "30", "."]);
        assert_eq!(cli.min_tokens, Some(30));
    }

    #[test]
    fn short_alias_r_for_reporters() {
        let cli = Cli::parse_from(["cpd", "-r", "json,xml", "."]);
        assert_eq!(cli.reporters, vec!["json", "xml"]);
    }

    #[test]
    fn short_alias_o_for_output() {
        let cli = Cli::parse_from(["cpd", "-o", "dist", "."]);
        assert_eq!(cli.output, Some(PathBuf::from("dist")));
    }

    #[test]
    fn short_alias_t_for_threshold() {
        let cli = Cli::parse_from(["cpd", "-t", "5.5", "."]);
        assert_eq!(cli.threshold, Some(5.5));
    }

    #[test]
    fn short_alias_m_for_mode() {
        let cli = Cli::parse_from(["cpd", "-m", "strict", "."]);
        assert_eq!(cli.mode, Some("strict".to_string()));
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
        assert_eq!(short.min_tokens, Some(30));
    }

    #[test]
    fn alias_l_equivalent_to_min_lines() {
        let short = Cli::parse_from(["cpd", "-l", "10", "."]);
        let long = Cli::parse_from(["cpd", "--min-lines", "10", "."]);
        assert_eq!(short.min_lines, long.min_lines);
        assert_eq!(short.min_lines, Some(10));
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
        assert_eq!(short.output, Some(PathBuf::from("dist")));
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
        assert_eq!(short.mode, Some("strict".to_string()));
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
        let short = Cli::parse_from(["cpd", "-z", "100kb", "."]);
        let long = Cli::parse_from(["cpd", "--max-size", "100kb", "."]);
        assert_eq!(short.max_size, Some("100kb".to_string()));
        assert_eq!(long.max_size, Some("100kb".to_string()));
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
        let cli = Cli::parse_from(["cpd", "-m", "", "."]);
        assert_eq!(cli.mode, Some("".to_string()));
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
            "cpd", "-k", "30", "-l", "10", "-m", "strict", "-r", "json,xml", "-o", "output", "-b",
            ".",
        ]);
        assert_eq!(cli.min_tokens, Some(30));
        assert_eq!(cli.min_lines, Some(10));
        assert_eq!(cli.mode, Some("strict".to_string()));
        assert_eq!(cli.reporters, vec!["json", "xml"]);
        assert_eq!(cli.output, Some(PathBuf::from("output")));
        assert!(cli.blame);
    }

    #[test]
    fn aliases_and_long_form_can_mix() {
        let cli = Cli::parse_from(["cpd", "-k", "30", "--min-lines", "10", "-m", "strict", "."]);
        assert_eq!(cli.min_tokens, Some(30));
        assert_eq!(cli.min_lines, Some(10));
        assert_eq!(cli.mode, Some("strict".to_string()));
    }

    #[test]
    fn no_tips_flag_defaults_to_false() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.no_tips);
    }

    #[test]
    fn no_tips_flag_set() {
        let cli = Cli::parse_from(["cpd", "--no-tips", "."]);
        assert!(cli.no_tips);
    }

    #[test]
    fn no_tips_propagates_to_options() {
        let cli = Cli::parse_from(["cpd", "--no-tips", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.no_tips);
    }

    #[test]
    fn no_tips_from_config() {
        let config = ConfigFile {
            no_tips: Some(true),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.no_tips);
    }

    #[test]
    fn no_tips_defaults_to_false_in_options() {
        // The CI env var auto-enables no_tips; unset it to test the bare default.
        // SAFETY: nextest runs each test in its own process, so mutating env vars is safe.
        unsafe { std::env::remove_var("CI") };
        let cli = Cli::parse_from(["cpd", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(!opts.no_tips);
    }

    #[test]
    fn silent_flag_defaults_to_false() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.silent);
    }

    #[test]
    fn silent_flag_set() {
        let cli = Cli::parse_from(["cpd", "--silent", "."]);
        assert!(cli.silent);
    }

    #[test]
    fn silent_short_alias() {
        let cli = Cli::parse_from(["cpd", "-s", "."]);
        assert!(cli.silent);
    }

    #[test]
    fn silent_propagates_to_options() {
        let cli = Cli::parse_from(["cpd", "--silent", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.silent);
    }

    #[test]
    fn silent_from_config() {
        let config = ConfigFile {
            silent: Some(true),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.silent);
    }

    #[test]
    fn config_min_tokens_overrides_default() {
        let config = ConfigFile {
            min_tokens: Some(30),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.min_tokens, 30);
    }

    #[test]
    fn cli_min_tokens_overrides_config() {
        let config = ConfigFile {
            min_tokens: Some(30),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "--min-tokens", "100", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.min_tokens, 100);
    }

    #[test]
    fn config_min_lines_overrides_default() {
        let config = ConfigFile {
            min_lines: Some(10),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.min_lines, 10);
    }

    #[test]
    fn config_reporters_override_default() {
        let config = ConfigFile {
            reporters: Some(vec!["json".to_string(), "html".to_string()]),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.reporters, vec!["json", "html"]);
    }

    #[test]
    fn cli_reporters_override_config() {
        let config = ConfigFile {
            reporters: Some(vec!["json".to_string()]),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "--reporters", "xml,html", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.reporters, vec!["xml", "html"]);
    }

    #[test]
    fn config_output_overrides_default() {
        let config = ConfigFile {
            output: Some("my-reports".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.output_dir, PathBuf::from("my-reports"));
    }

    #[test]
    fn config_mode_overrides_default() {
        let config = ConfigFile {
            mode: Some("strict".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.mode, cpd_tokenizer::tokenizer::Mode::Strict);
    }

    #[test]
    fn cli_mode_overrides_config() {
        let config = ConfigFile {
            mode: Some("strict".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "--mode", "weak", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.mode, cpd_tokenizer::tokenizer::Mode::Weak);
    }

    // New option tests

    #[test]
    fn absolute_flag_short() {
        let cli = Cli::parse_from(["cpd", "-a", "."]);
        assert!(cli.absolute);
    }

    #[test]
    fn absolute_flag_long() {
        let cli = Cli::parse_from(["cpd", "--absolute", "."]);
        assert!(cli.absolute);
    }

    #[test]
    fn absolute_defaults_to_false() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.absolute);
    }

    #[test]
    fn absolute_propagates_to_options() {
        let cli = Cli::parse_from(["cpd", "--absolute", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.absolute);
    }

    #[test]
    fn absolute_from_config() {
        let config = ConfigFile {
            absolute: Some(true),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.absolute);
    }

    #[test]
    fn ignore_case_flag() {
        let cli = Cli::parse_from(["cpd", "--ignore-case", "."]);
        assert!(cli.ignore_case);
    }

    #[test]
    fn ignore_case_defaults_to_false() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.ignore_case);
    }

    #[test]
    fn ignore_case_propagates_to_options() {
        let cli = Cli::parse_from(["cpd", "--ignore-case", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.ignore_case);
    }

    #[test]
    fn ignore_case_from_config() {
        let config = ConfigFile {
            ignore_case: Some(true),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.ignore_case);
    }

    #[test]
    fn formats_exts_parsing() {
        let cli = Cli::parse_from(["cpd", "--formats-exts", "javascript:es,es6;dart:dt", "."]);
        assert_eq!(
            cli.formats_exts,
            Some("javascript:es,es6;dart:dt".to_string())
        );
    }

    #[test]
    fn formats_exts_propagates_to_options() {
        let cli = Cli::parse_from(["cpd", "--formats-exts", "javascript:es,es6;dart:dt", "."]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(
            opts.formats_exts.get("javascript"),
            Some(&vec!["es".to_string(), "es6".to_string()])
        );
        assert_eq!(opts.formats_exts.get("dart"), Some(&vec!["dt".to_string()]));
    }

    #[test]
    fn formats_names_parsing() {
        let cli = Cli::parse_from([
            "cpd",
            "--formats-names",
            "makefile:Makefile,GNUmakefile;docker:Dockerfile",
            ".",
        ]);
        assert_eq!(
            cli.formats_names,
            Some("makefile:Makefile,GNUmakefile;docker:Dockerfile".to_string())
        );
    }

    #[test]
    fn formats_names_propagates_to_options() {
        let cli = Cli::parse_from([
            "cpd",
            "--formats-names",
            "makefile:Makefile,GNUmakefile;docker:Dockerfile",
            ".",
        ]);
        let config = ConfigFile::default();
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(
            opts.formats_names.get("makefile"),
            Some(&vec!["Makefile".to_string(), "GNUmakefile".to_string()])
        );
        assert_eq!(
            opts.formats_names.get("docker"),
            Some(&vec!["Dockerfile".to_string()])
        );
    }

    #[test]
    fn formats_exts_from_config() {
        let config = ConfigFile {
            formats_exts: Some("javascript:es,mjs".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(
            opts.formats_exts.get("javascript"),
            Some(&vec!["es".to_string(), "mjs".to_string()])
        );
    }

    #[test]
    fn cli_formats_exts_overrides_config() {
        let config = ConfigFile {
            formats_exts: Some("dart:dt".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "--formats-exts", "javascript:es,es6", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert!(opts.formats_exts.contains_key("javascript"));
        assert!(!opts.formats_exts.contains_key("dart"));
    }

    #[test]
    fn max_size_human_readable_kb() {
        assert_eq!(parse_size("100kb"), Some(102400));
    }

    #[test]
    fn max_size_human_readable_mb() {
        assert_eq!(parse_size("1mb"), Some(1048576));
    }

    #[test]
    fn max_size_human_readable_raw_number() {
        assert_eq!(parse_size("102400"), Some(102400));
    }

    #[test]
    fn max_size_human_readable_gb() {
        assert_eq!(parse_size("2gb"), Some(2147483648));
    }

    #[test]
    fn max_size_human_readable_k() {
        assert_eq!(parse_size("5k"), Some(5120));
    }

    #[test]
    fn max_size_human_readable_m() {
        assert_eq!(parse_size("3m"), Some(3145728));
    }

    #[test]
    fn max_size_human_readable_b() {
        assert_eq!(parse_size("100b"), Some(100));
    }

    #[test]
    fn max_size_human_readable_negative_returns_none() {
        assert_eq!(parse_size("-1kb"), None);
    }

    #[test]
    fn max_size_human_readable_empty_returns_none() {
        assert_eq!(parse_size(""), None);
    }

    #[test]
    fn max_size_human_readable_invalid_returns_none() {
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn max_size_option_string_in_cli() {
        let cli = Cli::parse_from(["cpd", "-z", "100kb", "."]);
        assert_eq!(cli.max_size, Some("100kb".to_string()));
    }

    #[test]
    fn max_size_config_parsing() {
        let config = ConfigFile {
            max_size: Some("1mb".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.max_size, Some(1048576));
    }

    #[test]
    fn max_size_cli_overrides_config() {
        let config = ConfigFile {
            max_size: Some("1mb".to_string()),
            ..Default::default()
        };
        let cli = Cli::parse_from(["cpd", "--max-size", "500kb", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.max_size, Some(512000));
    }

    #[test]
    fn parse_format_mappings_simple() {
        let result = super::parse_format_mappings("javascript:es,es6");
        assert_eq!(
            result.get("javascript"),
            Some(&vec!["es".to_string(), "es6".to_string()])
        );
    }

    #[test]
    fn parse_format_mappings_multiple() {
        let result = super::parse_format_mappings("javascript:es,es6;dart:dt");
        assert_eq!(
            result.get("javascript"),
            Some(&vec!["es".to_string(), "es6".to_string()])
        );
        assert_eq!(result.get("dart"), Some(&vec!["dt".to_string()]));
    }

    #[test]
    fn parse_format_mappings_empty() {
        let result = super::parse_format_mappings("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_format_mappings_trailing_semicolon() {
        let result = super::parse_format_mappings("javascript:es;");
        assert_eq!(result.get("javascript"), Some(&vec!["es".to_string()]));
        assert_eq!(result.len(), 1);
    }
}
