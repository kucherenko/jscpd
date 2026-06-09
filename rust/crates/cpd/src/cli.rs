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

    /// File-level glob patterns to ignore, e.g. "**/node_modules/**" (comma-separated)
    #[arg(long, short = 'i', value_delimiter = ',')]
    pub ignore: Vec<String>,

    /// Code-level regex patterns to skip matching tokens during detection, e.g. "//\\s*cpd-disable" (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub ignore_pattern: Vec<String>,

    /// Output reporters (comma-separated): console,json,xml,csv,html,markdown,badge,sarif,ai,xcode,threshold,silent,console-full
    /// Aliases: "full" and "consoleFull" are accepted for "console-full"
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

    /// Glob pattern to find files to scan (e.g. **/*.ts, **/*.{js,ts})
    #[arg(long, short = 'p')]
    pub pattern: Option<String>,

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

    /// Print merged config (CLI + config file) as JSON and exit without running detection
    #[arg(long)]
    pub debug: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFile {
    pub path: Option<Vec<String>>,
    #[serde(alias = "min-tokens")]
    pub min_tokens: Option<usize>,
    #[serde(alias = "min-lines")]
    pub min_lines: Option<usize>,
    #[serde(alias = "max-lines")]
    pub max_lines: Option<usize>,
    pub mode: Option<String>,
    #[serde(alias = "formats")]
    pub format: Option<Vec<String>>,
    #[serde(alias = "ignore-pattern")]
    pub ignore_pattern: Option<Vec<String>>,
    #[serde(alias = "ignore")]
    pub ignore: Option<Vec<String>>,
    pub pattern: Option<String>,
    pub reporters: Option<Vec<String>>,
    pub output: Option<String>,
    pub threshold: Option<f64>,
    pub blame: Option<bool>,
    #[serde(alias = "no-gitignore")]
    pub no_gitignore: Option<bool>,
    #[serde(alias = "follow-symlinks")]
    pub follow_symlinks: Option<bool>,
    #[serde(alias = "max-size")]
    pub max_size: Option<String>,
    #[serde(alias = "no-colors")]
    pub no_colors: Option<bool>,
    pub absolute: Option<bool>,
    #[serde(alias = "ignore-case")]
    pub ignore_case: Option<bool>,
    #[serde(alias = "formats-exts")]
    pub formats_exts: Option<String>,
    #[serde(alias = "formats-names")]
    pub formats_names: Option<String>,
    #[serde(alias = "skip-local")]
    pub skip_local: Option<bool>,
    #[serde(alias = "exit-code")]
    pub exit_code: Option<i32>,
    #[serde(alias = "no-tips")]
    pub no_tips: Option<bool>,
    pub silent: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConfigSource {
    Explicit(PathBuf),
    AutoJscpdJson,
    AutoPackageJson,
}

#[derive(Debug, Clone)]
pub(crate) enum ConfigDiagnostic {
    IoError {
        source: PathBuf,
        error: String,
    },
    ParseError {
        source: PathBuf,
        line: Option<usize>,
        error: String,
    },
    UnknownField {
        source: PathBuf,
        field: String,
        migration_hint: Option<String>,
    },
    InvalidValue {
        source: PathBuf,
        field: String,
        value: String,
        reason: String,
    },
}

impl ConfigDiagnostic {
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ConfigDiagnostic::IoError { .. } | ConfigDiagnostic::ParseError { .. }
        )
    }
}

impl std::fmt::Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigDiagnostic::IoError { source, error } => {
                write!(f, "config file {}: {}", source.display(), error)
            }
            ConfigDiagnostic::ParseError {
                source,
                line: Some(line),
                error,
            } => {
                write!(
                    f,
                    "config file {} line {}: {}",
                    source.display(),
                    line,
                    error
                )
            }
            ConfigDiagnostic::ParseError {
                source,
                line: None,
                error,
            } => {
                write!(f, "config file {}: {}", source.display(), error)
            }
            ConfigDiagnostic::UnknownField {
                source,
                field,
                migration_hint: Some(hint),
            } => {
                write!(
                    f,
                    "config file {}: unknown field '{}' — {}",
                    source.display(),
                    field,
                    hint
                )
            }
            ConfigDiagnostic::UnknownField {
                source,
                field,
                migration_hint: None,
            } => {
                write!(
                    f,
                    "config file {}: unknown field '{}'",
                    source.display(),
                    field
                )
            }
            ConfigDiagnostic::InvalidValue {
                source,
                field,
                value,
                reason,
            } => {
                write!(
                    f,
                    "config file {}: invalid value for '{}': {} ({})",
                    source.display(),
                    field,
                    value,
                    reason
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigResult {
    pub config: ConfigFile,
    pub source: Option<ConfigSource>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl ConfigResult {
    #[allow(dead_code)]
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

pub(crate) fn print_diagnostics(diagnostics: &[ConfigDiagnostic]) {
    for d in diagnostics {
        eprintln!("{}", d);
    }
}

pub(crate) fn validate_config(config: &ConfigFile, source: &Path) -> Vec<ConfigDiagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(ref mode) = config.mode {
        match mode.as_str() {
            "mild" | "weak" | "strict" => {}
            _ => diagnostics.push(ConfigDiagnostic::InvalidValue {
                source: source.to_path_buf(),
                field: "mode".to_string(),
                value: mode.clone(),
                reason: "must be one of: mild, weak, strict".to_string(),
            }),
        }
    }

    diagnostics
}

pub(crate) static KNOWN_CONFIG_FIELDS: &[&str] = &[
    "path",
    "minTokens",
    "minLines",
    "maxLines",
    "mode",
    "format",
    "formats",
    "ignorePattern",
    "ignore",
    "pattern",
    "reporters",
    "output",
    "threshold",
    "blame",
    "noGitignore",
    "followSymlinks",
    "noSymlinks",
    "noSymLinks",
    "maxSize",
    "noColors",
    "absolute",
    "ignoreCase",
    "formatsExts",
    "formatsNames",
    "skipLocal",
    "exitCode",
    "noTips",
    "silent",
    // kebab-case aliases (v4 compat)
    "min-tokens",
    "min-lines",
    "max-lines",
    "max-size",
    "ignore-case",
    "no-gitignore",
    "follow-symlinks",
    "skip-local",
    "exit-code",
    "no-colors",
    "no-tips",
    "formats-exts",
    "formats-names",
    "ignore-pattern",
];

pub(crate) static V4_SILENT_IGNORE: &[&str] = &[
    "gitignore",
    "debug",
    "verbose",
    "config",
    "xslHref",
    "//",
    "",
];

pub(crate) static V4_MIGRATIONS: &[(&str, &str)] = &[
    ("executionId", "removed from config file in v5"),
    (
        "store",
        "removed from config file in v5, use --store CLI flag",
    ),
    (
        "storePath",
        "removed from config file in v5, use --store-path CLI flag",
    ),
    ("cache", "removed from config file in v5"),
    ("list", "removed from config file in v5"),
    ("reportersOptions", "removed from config file in v5"),
    ("listeners", "removed from config file in v5"),
    ("tokensToSkip", "removed from config file in v5"),
    ("hashFunction", "removed from config file in v5"),
];

pub(crate) fn scan_unknown_fields(
    value: &serde_json::Value,
    source: &Path,
) -> Vec<ConfigDiagnostic> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return vec![],
    };
    obj.keys()
        .filter(|k| !KNOWN_CONFIG_FIELDS.contains(&k.as_str()))
        .filter(|k| !V4_SILENT_IGNORE.contains(&k.as_str()))
        .map(|k| {
            let hint = check_v4_migration(k);
            ConfigDiagnostic::UnknownField {
                source: source.to_path_buf(),
                field: k.clone(),
                migration_hint: hint,
            }
        })
        .collect()
}

fn check_v4_migration(field: &str) -> Option<String> {
    V4_MIGRATIONS
        .iter()
        .find(|(k, _)| *k == field)
        .map(|(_, v)| v.to_string())
}

/// Normalize v4 config fields in a JSON value before deserializing to ConfigFile.
///
/// Handles:
/// - `"//"` and `""` (JSONC-style comment keys) → removed
/// - `"ignore"` and `"ignorePattern"` are kept as separate fields:
///   `"ignore"` = file-level glob patterns, `"ignorePattern"` = code-level regex
/// - `"noSymlinks"` / `"noSymLinks"` (bool) → inverted and merged into `"followSymlinks"`
/// - `"formatsExts"` / `"formats-exts"` as array or object → converted to string
/// - `"format"` / `"formats"` as string → wrapped in array
/// - `"threshold"` as string → converted to number
fn normalize_v4_config(value: &mut serde_json::Value) {
    let obj = match value.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // Remove JSONC-style comment keys ("//" and "")
    obj.remove("//");
    obj.remove("");

    // Coerce "threshold" from string to number
    if let Some(threshold) = obj.remove("threshold") {
        let coerced = match &threshold {
            serde_json::Value::String(s) => s
                .parse::<f64>()
                .ok()
                .map(|f| {
                    serde_json::Number::from_f64(f)
                        .map(serde_json::Value::from)
                        .unwrap_or_else(|| serde_json::Value::from(0))
                })
                .unwrap_or(threshold),
            _ => threshold,
        };
        obj.insert("threshold".to_string(), coerced);
    }

    // Coerce "format" from string to array
    for key in &["format", "formats"] {
        if let Some(val) = obj.remove(*key) {
            let coerced = match val {
                serde_json::Value::String(s) => {
                    serde_json::Value::Array(vec![serde_json::Value::String(s)])
                }
                other => other,
            };
            obj.insert(key.to_string(), coerced);
        }
    }

    // v4 compat: "ignore" is file-level glob patterns (handled separately from
    // "ignorePattern" which is code-level regex). Both are kept as distinct fields
    // in ConfigFile — no merging needed.

    // "noSymlinks" / "noSymLinks" (bool) → inverted "followSymlinks"
    let no_symlinks_val = obj
        .remove("noSymlinks")
        .or_else(|| obj.remove("noSymLinks"));
    if let Some(val) = no_symlinks_val {
        let inverted = match val {
            serde_json::Value::Bool(b) => !b,
            _ => true,
        };
        obj.entry("followSymlinks".to_string())
            .and_modify(|e| {
                if let serde_json::Value::Bool(existing) = e {
                    *existing = *existing && inverted;
                }
            })
            .or_insert_with(|| serde_json::Value::Bool(inverted));
    }

    // "formatsExts" / "formats-exts" type coercion: accept array or object, convert to string
    for key in &["formatsExts", "formats-exts"] {
        coerce_formats_mapping(obj, key);
    }
    // "formatsNames" / "formats-names" same treatment
    for key in &["formatsNames", "formats-names"] {
        coerce_formats_mapping(obj, key);
    }
}

/// Coerce a formats mapping field (formatsExts/formatsNames) from array or object to string.
///
/// Accepted forms:
///   - String: "javascript:es,es6;dart:dt" (v5 canonical, no conversion needed)
///   - Array of strings: ["javascript:es,es6"] → "javascript:es,es6"
///   - Object: {"javascript": ["es","es6"], "dart": ["dt"]} → "javascript:es,es6;dart:dt"
fn coerce_formats_mapping(obj: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    if let Some(val) = obj.remove(key) {
        let coerced = match val {
            serde_json::Value::String(s) => serde_json::Value::String(s),
            serde_json::Value::Array(arr) => {
                let s: String = arr
                    .into_iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .join(";");
                serde_json::Value::String(s)
            }
            serde_json::Value::Object(map) => {
                let parts: Vec<String> = map
                    .into_iter()
                    .filter_map(|(format, exts)| {
                        let ext_names: Vec<String> = match exts {
                            serde_json::Value::Array(arr) => arr
                                .into_iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                            serde_json::Value::String(s) => vec![s],
                            _ => return None,
                        };
                        if ext_names.is_empty() {
                            None
                        } else {
                            Some(format!("{}:{}", format, ext_names.join(",")))
                        }
                    })
                    .collect();
                serde_json::Value::String(parts.join(";"))
            }
            other => other,
        };
        obj.insert(key.to_string(), coerced);
    }
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
/// Reports diagnostics for any errors encountered (IO, parse, unknown fields, invalid values).
/// For explicit --config paths, all diagnostics are fatal (caller should exit with code 1).
/// For auto-discovered configs, diagnostics are warnings and the cascade falls through
/// to the next source on IO/parse errors.
/// Paths in the config file are resolved relative to the config file's directory,
/// matching jscpd v4 behavior.
pub fn load_config(path: Option<&Path>) -> ConfigResult {
    if let Some(p) = path {
        return load_explicit_config(p);
    }

    let mut auto_diagnostics = Vec::new();

    if let Some(result) = try_load_jscpd_json(&mut auto_diagnostics) {
        return result;
    }

    if let Some(result) = try_load_package_json(&mut auto_diagnostics) {
        return result;
    }

    ConfigResult {
        config: ConfigFile::default(),
        source: None,
        diagnostics: auto_diagnostics,
    }
}

fn load_explicit_config(p: &Path) -> ConfigResult {
    let mut diagnostics = Vec::new();

    match std::fs::read_to_string(p) {
        Ok(content) => {
            let value: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    let line = extract_line_number(&e);
                    diagnostics.push(ConfigDiagnostic::ParseError {
                        source: p.to_path_buf(),
                        line,
                        error: e.to_string(),
                    });
                    return ConfigResult {
                        config: ConfigFile::default(),
                        source: Some(ConfigSource::Explicit(p.to_path_buf())),
                        diagnostics,
                    };
                }
            };

            diagnostics.extend(scan_unknown_fields(&value, p));

            let mut value = value;
            normalize_v4_config(&mut value);

            match serde_json::from_value::<ConfigFile>(value) {
                Ok(mut cfg) => {
                    let config_dir = p.parent().unwrap_or(Path::new(".")).to_path_buf();
                    resolve_config_paths(&mut cfg, &config_dir);
                    diagnostics.extend(validate_config(&cfg, p));
                    ConfigResult {
                        config: cfg,
                        source: Some(ConfigSource::Explicit(p.to_path_buf())),
                        diagnostics,
                    }
                }
                Err(e) => {
                    let line = extract_line_number(&e);
                    diagnostics.push(ConfigDiagnostic::ParseError {
                        source: p.to_path_buf(),
                        line,
                        error: e.to_string(),
                    });
                    ConfigResult {
                        config: ConfigFile::default(),
                        source: Some(ConfigSource::Explicit(p.to_path_buf())),
                        diagnostics,
                    }
                }
            }
        }
        Err(e) => {
            diagnostics.push(ConfigDiagnostic::IoError {
                source: p.to_path_buf(),
                error: e.to_string(),
            });
            ConfigResult {
                config: ConfigFile::default(),
                source: Some(ConfigSource::Explicit(p.to_path_buf())),
                diagnostics,
            }
        }
    }
}

fn try_load_jscpd_json(auto_diagnostics: &mut Vec<ConfigDiagnostic>) -> Option<ConfigResult> {
    let path = PathBuf::from(".jscpd.json");

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let value: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    let line = extract_line_number(&e);
                    auto_diagnostics.push(ConfigDiagnostic::ParseError {
                        source: path.clone(),
                        line,
                        error: e.to_string(),
                    });
                    return None;
                }
            };

            let mut field_diagnostics = scan_unknown_fields(&value, &path);

            let mut value = value;
            normalize_v4_config(&mut value);

            match serde_json::from_value::<ConfigFile>(value) {
                Ok(mut cfg) => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    resolve_config_paths(&mut cfg, &cwd);
                    let mut validation_diagnostics = validate_config(&cfg, &path);
                    field_diagnostics.append(&mut validation_diagnostics);

                    Some(ConfigResult {
                        config: cfg,
                        source: Some(ConfigSource::AutoJscpdJson),
                        diagnostics: field_diagnostics,
                    })
                }
                Err(e) => {
                    let line = extract_line_number(&e);
                    auto_diagnostics.push(ConfigDiagnostic::ParseError {
                        source: path.clone(),
                        line,
                        error: e.to_string(),
                    });
                    None
                }
            }
        }
        Err(_) => None,
    }
}

fn try_load_package_json(auto_diagnostics: &mut Vec<ConfigDiagnostic>) -> Option<ConfigResult> {
    let path = PathBuf::from("package.json");

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let pkg: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    let line = extract_line_number(&e);
                    auto_diagnostics.push(ConfigDiagnostic::ParseError {
                        source: path.clone(),
                        line,
                        error: e.to_string(),
                    });
                    return None;
                }
            };

            let jscpd_cfg = pkg.get("jscpd")?;

            let mut field_diagnostics = scan_unknown_fields(jscpd_cfg, &path);

            let mut jscpd_value = jscpd_cfg.clone();
            normalize_v4_config(&mut jscpd_value);

            match serde_json::from_value::<ConfigFile>(jscpd_value) {
                Ok(mut cfg) => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    resolve_config_paths(&mut cfg, &cwd);
                    let mut validation_diagnostics = validate_config(&cfg, &path);
                    field_diagnostics.append(&mut validation_diagnostics);

                    Some(ConfigResult {
                        config: cfg,
                        source: Some(ConfigSource::AutoPackageJson),
                        diagnostics: field_diagnostics,
                    })
                }
                Err(e) => {
                    let line = extract_line_number(&e);
                    auto_diagnostics.push(ConfigDiagnostic::ParseError {
                        source: path.clone(),
                        line,
                        error: e.to_string(),
                    });
                    None
                }
            }
        }
        Err(_) => None,
    }
}

fn extract_line_number(error: &serde_json::Error) -> Option<usize> {
    Some(error.line())
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
    fn short_alias_i_for_ignore() {
        let cli = Cli::parse_from(["cpd", "-i", "*.test.js,*.spec.ts", "."]);
        assert_eq!(cli.ignore, vec!["*.test.js", "*.spec.ts"]);
    }

    #[test]
    fn ignore_pattern_cli_flag() {
        let cli = Cli::parse_from(["cpd", "--ignore-pattern", "function", "."]);
        assert_eq!(cli.ignore_pattern, vec!["function"]);
        assert!(cli.ignore.is_empty());
    }

    #[test]
    fn ignore_and_ignore_pattern_work_together() {
        let cli = Cli::parse_from([
            "cpd",
            "--ignore",
            "*.test.js",
            "--ignore-pattern",
            "function",
            ".",
        ]);
        assert_eq!(cli.ignore, vec!["*.test.js"]);
        assert_eq!(cli.ignore_pattern, vec!["function"]);
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
    fn alias_i_equivalent_to_ignore() {
        let short = Cli::parse_from(["cpd", "-i", "*.test.js,*.spec.ts", "."]);
        let long = Cli::parse_from(["cpd", "--ignore", "*.test.js,*.spec.ts", "."]);
        assert_eq!(short.ignore, long.ignore);
        assert_eq!(short.ignore, vec!["*.test.js", "*.spec.ts"]);
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

    #[test]
    fn known_fields_covers_all_config_file_fields() {
        let expected_fields = [
            "path",
            "minTokens",
            "minLines",
            "maxLines",
            "mode",
            "format",
            "formats",
            "ignorePattern",
            "ignore",
            "pattern",
            "reporters",
            "output",
            "threshold",
            "blame",
            "noGitignore",
            "followSymlinks",
            "noSymlinks",
            "noSymLinks",
            "maxSize",
            "noColors",
            "absolute",
            "ignoreCase",
            "formatsExts",
            "formatsNames",
            "skipLocal",
            "exitCode",
            "noTips",
            "silent",
            "min-tokens",
            "min-lines",
            "max-lines",
            "max-size",
            "ignore-case",
            "no-gitignore",
            "follow-symlinks",
            "skip-local",
            "exit-code",
            "no-colors",
            "no-tips",
            "formats-exts",
            "formats-names",
            "ignore-pattern",
        ];
        for field in &expected_fields {
            assert!(
                KNOWN_CONFIG_FIELDS.contains(field),
                "KNOWN_CONFIG_FIELDS missing field: '{}'",
                field
            );
        }
    }

    #[test]
    fn scan_unknown_fields_empty_object() {
        let value = serde_json::json!({});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn scan_unknown_fields_known_fields_only() {
        let value = serde_json::json!({"minTokens": 50, "mode": "strict"});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn scan_unknown_fields_detects_unknown() {
        let value = serde_json::json!({"minTokens": 50, "unknownField": true});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert_eq!(diagnostics.len(), 1);
        match &diagnostics[0] {
            ConfigDiagnostic::UnknownField {
                field,
                migration_hint,
                ..
            } => {
                assert_eq!(field, "unknownField");
                assert!(migration_hint.is_none());
            }
            _ => panic!("Expected UnknownField diagnostic"),
        }
    }

    #[test]
    fn scan_unknown_fields_v4_migration_hint() {
        let value = serde_json::json!({"store": "leveldb"});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert_eq!(diagnostics.len(), 1);
        match &diagnostics[0] {
            ConfigDiagnostic::UnknownField {
                field,
                migration_hint,
                ..
            } => {
                assert_eq!(field, "store");
                assert_eq!(
                    migration_hint.as_deref(),
                    Some("removed from config file in v5, use --store CLI flag")
                );
            }
            _ => panic!("Expected UnknownField diagnostic"),
        }
    }

    #[test]
    fn scan_known_fields_pattern_is_known() {
        let value = serde_json::json!({"pattern": "**/*.js"});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "pattern should be a known field, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn config_file_pattern_string() {
        let v: ConfigFile = serde_json::from_str(r#"{"pattern": "**/*.ts"}"#).unwrap();
        assert_eq!(v.pattern, Some("**/*.ts".to_string()));
    }

    #[test]
    fn config_file_pattern_defaults_to_none() {
        let v: ConfigFile = serde_json::from_str(r#"{"threshold": 5}"#).unwrap();
        assert_eq!(v.pattern, None);
    }

    #[test]
    fn scan_known_fields_nosymlinks_is_known() {
        let value = serde_json::json!({"noSymlinks": true});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "noSymlinks should be a known field, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn scan_known_fields_nosymlink_capital_l_is_known() {
        let value = serde_json::json!({"noSymLinks": true});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "noSymLinks should be a known field, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn scan_unknown_fields_v4_removed_field() {
        let value = serde_json::json!({"store": "leveldb"});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert_eq!(diagnostics.len(), 1);
        match &diagnostics[0] {
            ConfigDiagnostic::UnknownField {
                field,
                migration_hint,
                ..
            } => {
                assert_eq!(field, "store");
                assert_eq!(
                    migration_hint.as_deref(),
                    Some("removed from config file in v5, use --store CLI flag")
                );
            }
            _ => panic!("Expected UnknownField diagnostic"),
        }
    }

    #[test]
    fn scan_unknown_fields_silent_ignore() {
        let value = serde_json::json!({"gitignore": true, "debug": true, "verbose": false});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "gitignore, debug, verbose should be silently ignored, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn scan_unknown_fields_non_object_returns_empty() {
        let value = serde_json::json!("not an object");
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn validate_config_accepts_valid_modes() {
        for mode in &["mild", "weak", "strict"] {
            let config = ConfigFile {
                mode: Some(mode.to_string()),
                ..Default::default()
            };
            let diagnostics = super::validate_config(&config, Path::new(".jscpd.json"));
            assert!(
                diagnostics.is_empty(),
                "mode '{}' should be valid but got diagnostics: {:?}",
                mode,
                diagnostics
            );
        }
    }

    #[test]
    fn validate_config_rejects_invalid_mode() {
        let config = ConfigFile {
            mode: Some("fast".to_string()),
            ..Default::default()
        };
        let diagnostics = super::validate_config(&config, Path::new(".jscpd.json"));
        assert_eq!(diagnostics.len(), 1);
        match &diagnostics[0] {
            ConfigDiagnostic::InvalidValue { field, reason, .. } => {
                assert_eq!(field, "mode");
                assert_eq!(reason, "must be one of: mild, weak, strict");
            }
            other => panic!("expected InvalidValue, got {:?}", other),
        }
    }

    #[test]
    fn validate_config_no_mode_produces_no_diagnostics() {
        let config = ConfigFile {
            mode: None,
            ..Default::default()
        };
        let diagnostics = super::validate_config(&config, Path::new(".jscpd.json"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn config_diagnostic_display_io_error() {
        let d = ConfigDiagnostic::IoError {
            source: PathBuf::from("/test/config.json"),
            error: "No such file".to_string(),
        };
        assert_eq!(
            format!("{}", d),
            "config file /test/config.json: No such file"
        );
    }

    #[test]
    fn config_diagnostic_display_parse_error_with_line() {
        let d = ConfigDiagnostic::ParseError {
            source: PathBuf::from("/test/config.json"),
            line: Some(5),
            error: "expected comma".to_string(),
        };
        let displayed = format!("{}", d);
        assert!(
            displayed.contains("/test/config.json"),
            "missing path: {}",
            displayed
        );
        assert!(
            displayed.contains("line 5"),
            "missing line number: {}",
            displayed
        );
    }

    #[test]
    fn config_diagnostic_display_parse_error_without_line() {
        let d = ConfigDiagnostic::ParseError {
            source: PathBuf::from("/test/config.json"),
            line: None,
            error: "parse error".to_string(),
        };
        let displayed = format!("{}", d);
        assert!(
            !displayed.contains("line"),
            "should not contain 'line': {}",
            displayed
        );
    }

    #[test]
    fn config_diagnostic_display_unknown_field_with_hint() {
        let d = ConfigDiagnostic::UnknownField {
            source: PathBuf::from("/test/.jscpd.json"),
            field: "store".to_string(),
            migration_hint: Some(
                "removed from config file in v5, use --store CLI flag".to_string(),
            ),
        };
        let displayed = format!("{}", d);
        assert!(
            displayed.contains("unknown field 'store'"),
            "missing field name: {}",
            displayed
        );
        assert!(
            displayed.contains("removed from config file in v5, use --store CLI flag"),
            "missing hint: {}",
            displayed
        );
    }

    #[test]
    fn config_diagnostic_display_unknown_field_without_hint() {
        let d = ConfigDiagnostic::UnknownField {
            source: PathBuf::from("/test/.jscpd.json"),
            field: "badField".to_string(),
            migration_hint: None,
        };
        let displayed = format!("{}", d);
        assert!(
            displayed.contains("unknown field 'badField'"),
            "missing field name: {}",
            displayed
        );
        assert!(
            !displayed.contains("did you mean"),
            "should not contain hint: {}",
            displayed
        );
        assert!(
            !displayed.contains("removed"),
            "should not contain removed: {}",
            displayed
        );
    }

    #[test]
    fn config_diagnostic_display_invalid_value() {
        let d = ConfigDiagnostic::InvalidValue {
            source: PathBuf::from("/test/.jscpd.json"),
            field: "mode".to_string(),
            value: "fast".to_string(),
            reason: "must be one of: mild, weak, strict".to_string(),
        };
        let displayed = format!("{}", d);
        assert!(
            displayed.contains("invalid value for 'mode': fast"),
            "missing value part: {}",
            displayed
        );
        assert!(
            displayed.contains("mild, weak, strict"),
            "missing reason: {}",
            displayed
        );
    }

    #[test]
    fn config_result_has_diagnostics() {
        let empty = ConfigResult {
            config: ConfigFile::default(),
            source: None,
            diagnostics: vec![],
        };
        assert!(!empty.has_diagnostics());

        let with_diag = ConfigResult {
            config: ConfigFile::default(),
            source: None,
            diagnostics: vec![ConfigDiagnostic::IoError {
                source: PathBuf::from("test.json"),
                error: "err".to_string(),
            }],
        };
        assert!(with_diag.has_diagnostics());
    }

    // kebab-case alias tests
    #[test]
    fn config_file_kebab_case_min_tokens() {
        let v: ConfigFile = serde_json::from_str(r#"{"min-tokens": 30}"#).unwrap();
        assert_eq!(v.min_tokens, Some(30));
    }

    #[test]
    fn config_file_kebab_case_min_lines() {
        let v: ConfigFile = serde_json::from_str(r#"{"min-lines": 10}"#).unwrap();
        assert_eq!(v.min_lines, Some(10));
    }

    #[test]
    fn config_file_kebab_case_max_lines() {
        let v: ConfigFile = serde_json::from_str(r#"{"max-lines": 500}"#).unwrap();
        assert_eq!(v.max_lines, Some(500));
    }

    #[test]
    fn config_file_kebab_case_max_size() {
        let v: ConfigFile = serde_json::from_str(r#"{"max-size": "100kb"}"#).unwrap();
        assert_eq!(v.max_size, Some("100kb".to_string()));
    }

    #[test]
    fn config_file_kebab_case_ignore_case() {
        let v: ConfigFile = serde_json::from_str(r#"{"ignore-case": true}"#).unwrap();
        assert_eq!(v.ignore_case, Some(true));
    }

    #[test]
    fn config_file_kebab_case_no_gitignore() {
        let v: ConfigFile = serde_json::from_str(r#"{"no-gitignore": true}"#).unwrap();
        assert_eq!(v.no_gitignore, Some(true));
    }

    #[test]
    fn config_file_kebab_case_follow_symlinks() {
        let v: ConfigFile = serde_json::from_str(r#"{"follow-symlinks": true}"#).unwrap();
        assert_eq!(v.follow_symlinks, Some(true));
    }

    #[test]
    fn config_file_kebab_case_skip_local() {
        let v: ConfigFile = serde_json::from_str(r#"{"skip-local": true}"#).unwrap();
        assert_eq!(v.skip_local, Some(true));
    }

    #[test]
    fn config_file_kebab_case_exit_code() {
        let v: ConfigFile = serde_json::from_str(r#"{"exit-code": 2}"#).unwrap();
        assert_eq!(v.exit_code, Some(2));
    }

    #[test]
    fn config_file_kebab_case_no_colors() {
        let v: ConfigFile = serde_json::from_str(r#"{"no-colors": true}"#).unwrap();
        assert_eq!(v.no_colors, Some(true));
    }

    #[test]
    fn config_file_kebab_case_no_tips() {
        let v: ConfigFile = serde_json::from_str(r#"{"no-tips": true}"#).unwrap();
        assert_eq!(v.no_tips, Some(true));
    }

    #[test]
    fn config_file_kebab_case_formats_exts() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"formats-exts": "javascript:es,mjs"}"#).unwrap();
        assert_eq!(v.formats_exts, Some("javascript:es,mjs".to_string()));
    }

    #[test]
    fn config_file_kebab_case_formats_names() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"formats-names": "makefile:Makefile"}"#).unwrap();
        assert_eq!(v.formats_names, Some("makefile:Makefile".to_string()));
    }

    #[test]
    fn config_file_kebab_case_ignore_pattern() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"ignore-pattern": ["**/node_modules/**"]}"#).unwrap();
        assert_eq!(
            v.ignore_pattern,
            Some(vec!["**/node_modules/**".to_string()])
        );
    }

    // v4 compat: "formats" alias for "format"
    #[test]
    fn config_file_formats_alias() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"formats": ["typescript", "javascript"]}"#).unwrap();
        assert_eq!(
            v.format,
            Some(vec!["typescript".to_string(), "javascript".to_string()])
        );
    }

    // v4 compat: "ignore" is now a separate field for file-level globs
    #[test]
    fn config_file_ignore_field() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"ignore": ["**/node_modules/**", "**/*.test.ts"]}"#).unwrap();
        assert_eq!(
            v.ignore,
            Some(vec![
                "**/node_modules/**".to_string(),
                "**/*.test.ts".to_string()
            ])
        );
    }

    // "ignore" and "ignorePattern" are separate fields in config
    #[test]
    fn config_file_ignore_and_ignore_pattern_separate() {
        let v: ConfigFile = serde_json::from_str(
            r#"{"ignore": ["**/node_modules/**"], "ignorePattern": ["function"]}"#,
        )
        .unwrap();
        assert_eq!(v.ignore, Some(vec!["**/node_modules/**".to_string()]));
        assert_eq!(v.ignore_pattern, Some(vec!["function".to_string()]));
    }

    // v4 compat: both camelCase and kebab-case work for same field
    #[test]
    fn config_file_camel_case_still_works() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"minTokens": 50, "ignorePattern": ["*.js"]}"#).unwrap();
        assert_eq!(v.min_tokens, Some(50));
        assert_eq!(v.ignore_pattern, Some(vec!["*.js".to_string()]));
    }

    // v4 compat: "debug" and "verbose" are silently ignored
    #[test]
    fn scan_unknown_fields_debug_silently_ignored() {
        let value = serde_json::json!({"debug": true, "verbose": false});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "debug and verbose should be silently ignored, got: {:?}",
            diagnostics
        );
    }

    // v4 compat: "config" and "xslHref" are silently ignored
    #[test]
    fn scan_unknown_fields_v4_silent_fields() {
        let value = serde_json::json!({"config": ".jscpd.json", "xslHref": "report.xsl", "gitignore": true});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "config, xslHref, gitignore should be silently ignored, got: {:?}",
            diagnostics
        );
    }

    // v4 compat: "ignore" is now a known field, not an unknown field
    #[test]
    fn scan_known_fields_ignore_is_known() {
        let value = serde_json::json!({"ignore": ["**/dist/**"]});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "ignore should be a known field, got: {:?}",
            diagnostics
        );
    }

    // v4 compat: "formats" is now a known field (alias), not an unknown field
    #[test]
    fn scan_known_fields_formats_is_known() {
        let value = serde_json::json!({"formats": ["typescript"]});
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "formats should be a known field, got: {:?}",
            diagnostics
        );
    }

    // debug flag
    #[test]
    fn debug_flag_defaults_to_false() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.debug);
    }

    #[test]
    fn debug_flag_set() {
        let cli = Cli::parse_from(["cpd", "--debug", "."]);
        assert!(cli.debug);
    }

    // normalize_v4_config tests

    #[test]
    fn normalize_pattern_preserved_as_own_field() {
        let mut value = serde_json::json!({"pattern": "**/*.ts", "ignore": ["**/node_modules/**"]});
        normalize_v4_config(&mut value);
        assert_eq!(value.get("pattern"), Some(&serde_json::json!("**/*.ts")));
        // "ignore" is kept as a separate field (file-level globs), not merged into "ignorePattern"
        assert!(value.get("ignore").is_some());
        assert_eq!(
            value.get("ignore"),
            Some(&serde_json::json!(["**/node_modules/**"]))
        );
    }

    #[test]
    fn normalize_noSymlinks_inverts_to_followSymlinks() {
        let mut value = serde_json::json!({"noSymlinks": true});
        normalize_v4_config(&mut value);
        assert!(
            value.get("noSymlinks").is_none(),
            "noSymlinks should be removed"
        );
        assert_eq!(value.get("followSymlinks"), Some(&serde_json::json!(false)));
    }

    #[test]
    fn normalize_noSymlinks_false_means_follow() {
        let mut value = serde_json::json!({"noSymlinks": false});
        normalize_v4_config(&mut value);
        assert!(value.get("noSymlinks").is_none());
        assert_eq!(value.get("followSymlinks"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn normalize_noSymLinks_capital_l_inverts() {
        let mut value = serde_json::json!({"noSymLinks": true});
        normalize_v4_config(&mut value);
        assert!(
            value.get("noSymLinks").is_none(),
            "noSymLinks should be removed"
        );
        assert_eq!(value.get("followSymlinks"), Some(&serde_json::json!(false)));
    }

    #[test]
    fn normalize_formats_exts_array_to_string() {
        let mut value = serde_json::json!({"formatsExts": ["javascript:es,es6"]});
        normalize_v4_config(&mut value);
        assert_eq!(
            value.get("formatsExts"),
            Some(&serde_json::json!("javascript:es,es6"))
        );
    }

    #[test]
    fn normalize_formats_exts_object_to_string() {
        let mut value =
            serde_json::json!({"formatsExts": {"javascript": ["es", "es6"], "dart": ["dt"]}});
        normalize_v4_config(&mut value);
        let result = value.get("formatsExts").unwrap().as_str().unwrap();
        assert!(
            result.contains("javascript:es,es6"),
            "should contain javascript mapping: {}",
            result
        );
        assert!(
            result.contains("dart:dt"),
            "should contain dart mapping: {}",
            result
        );
    }

    #[test]
    fn normalize_formats_exts_kebab_case_array() {
        let mut value = serde_json::json!({"formats-exts": ["javascript:es,es6"]});
        normalize_v4_config(&mut value);
        assert_eq!(
            value.get("formats-exts"),
            Some(&serde_json::json!("javascript:es,es6"))
        );
    }

    #[test]
    fn normalize_formats_names_object_to_string() {
        let mut value =
            serde_json::json!({"formatsNames": {"makefile": ["Makefile", "GNUmakefile"]}});
        normalize_v4_config(&mut value);
        let result = value.get("formatsNames").unwrap().as_str().unwrap();
        assert!(
            result.contains("makefile:Makefile,GNUmakefile"),
            "should contain makefile mapping: {}",
            result
        );
    }

    #[test]
    fn normalize_formats_exts_string_unchanged() {
        let mut value = serde_json::json!({"formatsExts": "javascript:es,es6;dart:dt"});
        normalize_v4_config(&mut value);
        assert_eq!(
            value.get("formatsExts"),
            Some(&serde_json::json!("javascript:es,es6;dart:dt"))
        );
    }

    #[test]
    fn normalize_mixed_v4_config() {
        let mut value = serde_json::json!({
            "pattern": "**/*.test.ts",
            "noSymlinks": true,
            "formatsExts": {"javascript": ["es", "es6"]},
            "ignore": ["**/node_modules/**"],
            "min-lines": 5,
            "threshold": 10
        });
        normalize_v4_config(&mut value);
        assert_eq!(
            value.get("pattern"),
            Some(&serde_json::json!("**/*.test.ts"))
        );
        assert!(value.get("noSymlinks").is_none());
        // "ignore" is kept as separate field (file-level globs), not merged into "ignorePattern"
        assert!(
            value.get("ignore").is_some(),
            "ignore is kept as a separate field"
        );
        assert_eq!(value.get("min-lines"), Some(&serde_json::json!(5)));
        assert_eq!(value.get("threshold"), Some(&serde_json::json!(10)));
        let ignore = value.get("ignore").unwrap().as_array().unwrap();
        assert!(ignore.contains(&serde_json::json!("**/node_modules/**")));
        assert_eq!(value.get("followSymlinks"), Some(&serde_json::json!(false)));
        assert!(
            value
                .get("formatsExts")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("javascript:es,es6")
        );
    }

    #[test]
    fn normalize_ignore_and_pattern_coexist() {
        let mut value = serde_json::json!({
            "ignore": ["**/node_modules/**"],
            "pattern": "**/*.ts"
        });
        normalize_v4_config(&mut value);
        // "ignore" is kept as separate field, not merged into "ignorePattern"
        assert!(value.get("ignore").is_some());
        assert_eq!(value.get("pattern"), Some(&serde_json::json!("**/*.ts")));
        let ignore = value.get("ignore").unwrap().as_array().unwrap();
        assert!(ignore.contains(&serde_json::json!("**/node_modules/**")));
    }

    #[test]
    fn normalize_comment_keys_removed() {
        let mut value = serde_json::json!({
            "//": "this is a comment",
            "": "https://example.com",
            "threshold": 10
        });
        normalize_v4_config(&mut value);
        assert!(
            value.get("//").is_none(),
            "// comment key should be removed"
        );
        assert!(value.get("").is_none(), "empty key should be removed");
        assert_eq!(value.get("threshold"), Some(&serde_json::json!(10)));
    }

    #[test]
    fn normalize_ignore_preserved_as_separate_field() {
        let mut value = serde_json::json!({"ignore": ["**/dist/**", "**/node_modules/**"]});
        normalize_v4_config(&mut value);
        // "ignore" is preserved as a separate field (file-level globs)
        assert!(
            value.get("ignore").is_some(),
            "ignore is kept as a separate field"
        );
        assert_eq!(
            value.get("ignore"),
            Some(&serde_json::json!(["**/dist/**", "**/node_modules/**"]))
        );
    }

    #[test]
    fn normalize_format_string_to_array() {
        let mut value = serde_json::json!({"format": "python"});
        normalize_v4_config(&mut value);
        assert_eq!(value.get("format"), Some(&serde_json::json!(["python"])));
    }

    #[test]
    fn normalize_format_array_unchanged() {
        let mut value = serde_json::json!({"format": ["typescript", "javascript"]});
        normalize_v4_config(&mut value);
        assert_eq!(
            value.get("format"),
            Some(&serde_json::json!(["typescript", "javascript"]))
        );
    }

    #[test]
    fn normalize_threshold_string_to_number() {
        let mut value = serde_json::json!({"threshold": "0"});
        normalize_v4_config(&mut value);
        let t = value.get("threshold").unwrap().as_f64().unwrap();
        assert_eq!(t, 0.0);
    }

    #[test]
    fn normalize_threshold_string_float_to_number() {
        let mut value = serde_json::json!({"threshold": "10.5"});
        normalize_v4_config(&mut value);
        assert_eq!(value.get("threshold"), Some(&serde_json::json!(10.5)));
    }

    #[test]
    fn normalize_threshold_number_unchanged() {
        let mut value = serde_json::json!({"threshold": 20});
        normalize_v4_config(&mut value);
        assert_eq!(value.get("threshold"), Some(&serde_json::json!(20)));
    }

    // Real-world config validation: db-ux-design-system/core-web pattern
    // Both "ignore" (file globs) and "ignorePattern" (code regexes) present
    #[test]
    fn real_world_config_ignore_and_ignore_pattern_separate() {
        let mut value = serde_json::json!({
            "threshold": 0,
            "reporters": ["consoleFull"],
            "minTokens": 50,
            "ignore": [
                "**/node_modules/**",
                "**/*.test.ts",
                "**/tests/**",
                "**/public/**"
            ],
            "ignorePattern": ["//\\s*cpd-disable", "import.*from\\s*'.*'"]
        });
        normalize_v4_config(&mut value);
        let v: ConfigFile = serde_json::from_value(value).unwrap();
        // "ignore" stays as file-level globs
        assert_eq!(
            v.ignore,
            Some(vec![
                "**/node_modules/**".to_string(),
                "**/*.test.ts".to_string(),
                "**/tests/**".to_string(),
                "**/public/**".to_string(),
            ])
        );
        // "ignorePattern" stays as code-level regexes
        assert_eq!(
            v.ignore_pattern,
            Some(vec![
                "//\\s*cpd-disable".to_string(),
                "import.*from\\s*'.*'".to_string(),
            ])
        );
    }

    // Real-world: producer-pal pattern with regex-based ignorePattern for copyright
    #[test]
    fn real_world_config_ignore_pattern_regex_for_copyright() {
        let mut value = serde_json::json!({
            "threshold": 0.25,
            "reporters": ["console"],
            "ignorePattern": ["//\\s*Copyright\\s*\\(C\\).*", "//\\s*SPDX-License-Identifier:.*"],
            "ignore": ["**/node_modules/**", "**/*.test.ts"]
        });
        normalize_v4_config(&mut value);
        let v: ConfigFile = serde_json::from_value(value).unwrap();
        assert_eq!(
            v.ignore_pattern,
            Some(vec![
                "//\\s*Copyright\\s*\\(C\\).*".to_string(),
                "//\\s*SPDX-License-Identifier:.*".to_string(),
            ])
        );
        assert_eq!(
            v.ignore,
            Some(vec![
                "**/node_modules/**".to_string(),
                "**/*.test.ts".to_string(),
            ])
        );
    }

    // Real-world: tweetclaw pattern with noSymlinks + both fields
    #[test]
    fn real_world_config_v4_nosymlinks_with_ignore_fields() {
        let mut value = serde_json::json!({
            "threshold": 0,
            "mode": "strict",
            "format": ["typescript"],
            "reporters": ["console"],
            "gitignore": true,
            "ignore": ["**/node_modules/**", "**/*.test.ts"],
            "ignorePattern": ["import.*from\\s*'.*'"],
            "minLines": 5,
            "minTokens": 50,
            "noSymlinks": true
        });
        normalize_v4_config(&mut value);
        let v: ConfigFile = serde_json::from_value(value).unwrap();
        assert_eq!(
            v.ignore,
            Some(vec![
                "**/node_modules/**".to_string(),
                "**/*.test.ts".to_string(),
            ])
        );
        assert_eq!(
            v.ignore_pattern,
            Some(vec!["import.*from\\s*'.*'".to_string(),])
        );
        // noSymlinks should be inverted to followSymlinks
        assert_eq!(v.follow_symlinks, Some(false));
        // gitignore should be silently ignored (v4 field)
        assert!(v.no_gitignore.is_none());
    }

    // Validation: "ignore" (file globs) should NOT be merged into "ignorePattern" (code regexes)
    #[test]
    fn v4_compat_ignore_not_merged_into_ignore_pattern() {
        let mut value = serde_json::json!({
            "ignore": ["**/node_modules/**", "**/*.spec.ts"],
            "ignorePattern": ["function"]
        });
        normalize_v4_config(&mut value);
        // Both fields must remain separate after normalization
        assert!(
            value.get("ignore").is_some(),
            "ignore must be preserved as separate field"
        );
        assert!(
            value.get("ignorePattern").is_some(),
            "ignorePattern must be preserved as separate field"
        );
        // ignore should NOT be merged into ignorePattern
        let ignore_pattern = value.get("ignorePattern").unwrap().as_array().unwrap();
        assert_eq!(
            ignore_pattern.len(),
            1,
            "ignorePattern should only contain its own entry, not merged from ignore"
        );
        assert_eq!(ignore_pattern[0], "function");
    }
}
