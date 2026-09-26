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

/// Named shortcuts for common cross-format groups.
pub const CROSS_FORMAT_PRESETS: &[(&str, &[&str])] =
    &[("js-ts", &["javascript", "jsx", "typescript", "tsx"])];

/// Parse a `--cross-formats` value: semicolon-separated groups of
/// comma-separated format names, e.g. "javascript,typescript;css,scss".
/// Preset names (see [`CROSS_FORMAT_PRESETS`]) expand in place. Groups that
/// end up with fewer than two formats are dropped; groups sharing a format
/// are merged (union).
pub fn parse_cross_formats(input: &str) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    for group in input.split(';') {
        let mut formats: Vec<String> = Vec::new();
        for entry in group.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let expanded = CROSS_FORMAT_PRESETS
                .iter()
                .find(|(name, _)| *name == entry)
                .map(|(_, members)| members.iter().map(|m| m.to_string()).collect())
                .unwrap_or_else(|| vec![entry.to_string()]);
            for format in expanded {
                if !formats.contains(&format) {
                    formats.push(format);
                }
            }
        }
        if formats.len() < 2 {
            if !formats.is_empty() {
                eprintln!(
                    "Warning: --cross-formats group '{}' has fewer than two formats, ignored",
                    group.trim()
                );
            }
            continue;
        }
        groups.push(formats);
    }

    // Merge groups that share a format (union semantics): a format can only
    // belong to one detection pool.
    let mut merged: Vec<Vec<String>> = Vec::new();
    for group in groups {
        let mut group = group;
        loop {
            let overlap = merged
                .iter()
                .position(|m| m.iter().any(|f| group.contains(f)));
            match overlap {
                Some(idx) => {
                    let existing = merged.remove(idx);
                    eprintln!(
                        "Warning: --cross-formats groups '{}' and '{}' overlap, merged",
                        existing.join(","),
                        group.join(",")
                    );
                    for format in existing.into_iter().rev() {
                        if !group.contains(&format) {
                            group.insert(0, format);
                        }
                    }
                }
                None => break,
            }
        }
        merged.push(group);
    }
    merged
}

/// Parse a `--skip-isolated` value: comma-separated isolation groups of
/// pipe-separated folders, e.g. "packages/a|packages/b,libs/a|libs/b".
/// Matches the CLI syntax of jscpd PR #628. Groups that end up with fewer
/// than two folders can never isolate anything and are dropped with a warning.
pub fn parse_skip_isolated(input: &str) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    for group in input.split(',') {
        let folders: Vec<String> = group
            .split('|')
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(str::to_string)
            .collect();
        if folders.len() < 2 {
            if !folders.is_empty() {
                eprintln!(
                    "Warning: --skip-isolated group '{}' has fewer than two folders, ignored",
                    group.trim()
                );
            }
            continue;
        }
        groups.push(folders);
    }
    groups
}

/// Name the program was invoked as (`cpd` or `jscpd`).
///
/// Both bin targets are built from the same `main.rs`, so the name cannot be
/// a literal: it is taken from the file stem of argv[0] (`jscpd.exe` → `jscpd`),
/// falling back to the bin target name baked in at compile time when argv[0]
/// is unavailable or empty. clap wants a `&'static str` for the command name,
/// so the (single, process-lifetime) string is leaked.
pub fn invoked_name() -> &'static str {
    std::env::args_os()
        .next()
        .and_then(|arg0| {
            Path::new(&arg0)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .map_or(env!("CARGO_BIN_NAME"), |name| {
            Box::leak(name.into_boxed_str())
        })
}

impl Cli {
    /// Parse `std::env::args_os()` with the command named after the invoked
    /// binary, so `--version` and the `Usage:` line say `jscpd` when run as
    /// `jscpd` and `cpd` when run as `cpd`. Exits like `Cli::parse()` on
    /// error, `--help` and `--version`.
    pub fn parse_invoked() -> Self {
        use clap::{CommandFactory, FromArgMatches};

        let matches = Cli::command().name(invoked_name()).get_matches();
        Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
    }
}

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_BIN_NAME"),
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

    /// Merge clones of the same file pair that are separated by at most N unmatched lines in both files into one near-miss clone (Type-3, reported as "similar"); 0 disables
    #[arg(long, value_name = "N")]
    pub max_gap_lines: Option<usize>,

    /// Report JavaScript/TypeScript function pairs whose AST similarity reaches RATIO as near-miss clones (Type-3, "similar"). A number in (0, 1]; the default 1 means exact matches only, e.g. 0.85 enables it
    #[arg(long, value_name = "RATIO")]
    pub similarity: Option<f32>,

    /// Find semantic clones (Type-4, experimental): functions that do the same
    /// thing written differently, in one language or across languages, e.g. a
    /// Rust backend and a Svelte frontend. Compares embeddings of the
    /// functions' code, computed on this machine by a model that
    /// --semantic-download fetches once, or by an embeddings API
    /// (--semantic-url). Functions of JavaScript, TypeScript, JSX, TSX, Vue,
    /// Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby,
    /// Scala and Swift
    #[arg(long)]
    pub semantic: bool,

    /// Which semantic clones to report: all (default), same (within one
    /// language: several implementations of one feature) or cross (across
    /// languages: a rule written once per side)
    #[arg(long, value_name = "SCOPE", value_parser = ["all", "same", "cross"])]
    pub semantic_scope: Option<String>,

    /// Where embeddings come from: local (default: a model run in-process,
    /// fetched once by --semantic-download) or http (an OpenAI-compatible
    /// embeddings API at --semantic-url; giving a URL selects it)
    #[arg(long, value_name = "PROVIDER", value_parser = ["local", "http"])]
    pub semantic_provider: Option<String>,

    /// Download the local embedding model (jina-embeddings-v2-base-code,
    /// 322 MB from huggingface.co) into the jscpd cache directory, checking
    /// its checksum; alone it exits after the download, with --semantic it
    /// goes on to scan
    #[arg(long)]
    pub semantic_download: bool,

    /// Lowest cosine similarity of a semantic clone, in (0, 1] (default: 0.6,
    /// calibrated for the default model; with another model, check the scores
    /// of a few known pairs first)
    #[arg(long, value_name = "RATIO")]
    pub semantic_threshold: Option<f32>,

    /// Embedding model for --semantic (default: jinaai/jina-embeddings-v2-base-code
    /// for the local provider, unclemusclez/jina-embeddings-v2-base-code — its
    /// Ollama name — for http)
    #[arg(long, value_name = "NAME")]
    pub semantic_model: Option<String>,

    /// OpenAI-compatible embeddings API for --semantic, e.g.
    /// http://localhost:11434/v1 for Ollama; selects the http provider. A key
    /// the API needs is read from the JSCPD_SEMANTIC_API_KEY environment
    /// variable, and is sent only to a URL given here or to a server on this
    /// machine
    #[arg(long, value_name = "URL")]
    pub semantic_url: Option<String>,

    /// Report only clones of these kinds: exact, renamed, similar, gap, ast,
    /// semantic (comma-separated). renamed needs --ignore-identifiers,
    /// --ignore-literals or --ignore-annotations; gap needs --max-gap-lines;
    /// ast needs --similarity; semantic needs --semantic
    #[arg(long, value_name = "LIST", value_delimiter = ',')]
    pub kind: Vec<String>,

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

    /// Output reporters (comma-separated): console,json,xml,csv,html,markdown,badge,sarif,codeclimate,openmetrics,ai,xcode,threshold,silent,console-full
    /// Aliases: "full" and "consoleFull" are accepted for "console-full"; "gitlab" for "codeclimate"
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

    /// Report SARIF results as "error" for clones with at least this many tokens (default: all "warning")
    #[arg(long, value_name = "TOKENS")]
    pub sarif_error_tokens: Option<u32>,

    /// Path to a clone baseline file (e.g. .jscpd-baseline.json): clones whose
    /// fingerprint is absent from it are reported as new
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<PathBuf>,

    /// Rewrite the baseline file from the current run, creating it if missing,
    /// and print added/removed fingerprint counts (requires --baseline)
    #[arg(long)]
    pub update_baseline: bool,

    /// Exit 1 when more than N new clones are found (default N: 0; requires
    /// --baseline or --baseline-from-ref)
    #[arg(long, value_name = "N", num_args(0..=1), default_missing_value = "0")]
    pub fail_on_new_clones: Option<u64>,

    /// Exit 1 when the scan analyzes no files: the paths exist but nothing
    /// matched the --format, --ignore and --pattern filters, or every file
    /// was below --min-tokens
    #[arg(long)]
    pub fail_on_empty: bool,

    /// Compare against an ephemeral baseline built from a git ref's tree
    /// (e.g. origin/main): the base ref is scanned with the same
    /// configuration and clones absent from it are reported as new
    #[arg(long, value_name = "REF", conflicts_with_all = ["baseline", "update_baseline"])]
    pub baseline_from_ref: Option<String>,

    /// Enrich clones with git blame data
    #[arg(long, short = 'b')]
    pub blame: bool,

    /// Do not respect .gitignore files
    #[arg(long)]
    pub no_gitignore: bool,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Skip files larger than SIZE (e.g. 1kb, 1mb, 100kb, or raw bytes). Default: 1mb
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

    /// Treat all identifiers as equal, so clones that differ only in variable, function or type names are found (Type-2 clones)
    #[arg(long)]
    pub ignore_identifiers: bool,

    /// Treat all string and numeric literals as equal, so clones that differ only in literal values are found
    #[arg(long)]
    pub ignore_literals: bool,

    /// Skip annotations and decorators (@Name, @Name(...)) in Java, Kotlin, Scala, Groovy, Python, Dart, Swift, JavaScript and TypeScript
    #[arg(long)]
    pub ignore_annotations: bool,

    /// Custom format-to-extension mappings (e.g. javascript:es,es6;dart:dt)
    #[arg(long)]
    pub formats_exts: Option<String>,

    /// Custom format-to-filename mappings (e.g. makefile:Makefile,GNUmakefile;docker:Dockerfile)
    #[arg(long)]
    pub formats_names: Option<String>,

    /// Detect clones across formats: semicolon-separated groups of
    /// comma-separated formats (e.g. "javascript,typescript;css,scss").
    /// Preset: js-ts = javascript,jsx,typescript,tsx. TypeScript files in a
    /// group that also contains JavaScript are compared with type
    /// annotations stripped.
    #[arg(long)]
    pub cross_formats: Option<String>,

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
    #[arg(long, visible_alias = "skipLocal")]
    pub skip_local: bool,

    /// Skip clones between different folders of the same isolation group:
    /// comma-separated groups of pipe-separated folders
    /// (e.g. "packages/a|packages/b,libs/a|libs/b")
    #[arg(long, visible_alias = "skipIsolated", value_name = "GROUPS")]
    pub skip_isolated: Option<String>,

    /// Accepted for compatibility; never had an effect and will be removed.
    /// Documented until 5.3.2 as a minimum duplication percentage, but no
    /// code ever read it.
    #[arg(long, hide = true, value_name = "PERCENT")]
    pub min_duplicated_lines: Option<f64>,

    /// Serve the Model Context Protocol over stdio: scan PATHs once, then expose
    /// check_duplication / get_statistics / check_current_directory tools to MCP clients
    #[arg(long)]
    pub mcp: bool,

    /// Print a codebase summary: top files and folders by tokens, lines, size, complexity
    #[arg(long)]
    pub summary: bool,

    /// Number of entries in each summary top list (default: 10)
    #[arg(long, value_name = "N")]
    pub summary_top: Option<usize>,

    /// Summary sort metric: tokens, lines, size, complexity (default: tokens)
    #[arg(long, value_name = "METRIC")]
    pub summary_by: Option<String>,

    /// Duplication trend over git history: scan every commit in RANGE (e.g.
    /// v5.0.0..HEAD) with this configuration and print a chart and a table
    #[arg(long, value_name = "RANGE")]
    pub history: Option<String>,

    /// Like --history, selecting commits since DATE (e.g. 2026-01-01);
    /// combines with --history to bound the range
    #[arg(long, value_name = "DATE")]
    pub history_since: Option<String>,

    /// Keep every Nth commit of the history series, counted from the newest (default: 1)
    #[arg(long, value_name = "N")]
    pub history_every: Option<usize>,

    /// Maximum number of commits in the history series, sampled evenly (default: 30)
    #[arg(long, value_name = "N")]
    pub history_limit: Option<usize>,

    /// Find dead code instead of duplicates: unused files, exports, symbols
    /// and imports across JavaScript, TypeScript and Python
    #[arg(long, alias = "basta", conflicts_with_all = ["complexity", "dashboard", "mcp"])]
    pub dead_code: bool,

    /// Report complexity only: the --summary tables ranked by complexity,
    /// without clone detection (reporters: console, ai, json)
    #[arg(long, conflicts_with_all = ["dashboard", "mcp"])]
    pub complexity: bool,

    /// Print one screen with the whole picture: the health score, project
    /// size, duplication, complexity and dead code (JavaScript, TypeScript,
    /// Python). Reporters: console, json, badge, markdown, html
    #[arg(long, conflicts_with = "mcp")]
    pub dashboard: bool,

    /// Print only the project health badge: one 0-100 score with a grade,
    /// from duplication, dead code and complexity, plus the metrics of
    /// --health-input. Reporters: console, ai, json, badge, markdown, html
    #[arg(long, conflicts_with_all = ["mcp", "dashboard", "complexity", "dead_code"])]
    pub health: bool,

    /// JSON file with metrics from other tools (coverage, tests, security)
    /// to include in the health score: {"metrics": [{"id", "score"} or
    /// {"id", "value", "halfLife", "direction"}]}
    #[arg(long, value_name = "FILE")]
    pub health_input: Option<PathBuf>,

    /// Dead-code findings to report: unused-file, unused-export,
    /// unused-symbol, unused-import, unused-member, or `all` (with --dead-code)
    #[arg(long, value_name = "LIST", value_delimiter = ',')]
    pub dead_code_categories: Vec<String>,

    /// Drop dead-code findings below this confidence, 0-100 (with --dead-code)
    #[arg(long, value_name = "N")]
    pub min_confidence: Option<u8>,

    /// Rust dead code from the compiler, for --dead-code, --dashboard and
    /// --health: a file of `cargo check --message-format=json` output, or
    /// `-` to read it from stdin
    #[arg(long, value_name = "FILE")]
    pub rust_diagnostics: Option<PathBuf>,

    /// Treat files matching this glob as dead-code entry points (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub entry: Vec<String>,

    /// Report dead code inside test, fixture and example files
    #[arg(long)]
    pub include_tests: bool,

    /// Report exports of entry-point files, which are usually a public API
    #[arg(long)]
    pub include_entry_exports: bool,

    /// Do not write detection progress and result to console
    #[arg(long, short = 's')]
    pub silent: bool,

    /// Do not print tips and promotional messages after detection (also skipped when stdout is not a terminal or CI or JSCPD_NO_TIPS is set)
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
    #[serde(alias = "max-gap-lines")]
    pub max_gap_lines: Option<usize>,
    pub similarity: Option<f32>,
    /// `true`, or the semantic-clone settings; see [`SemanticSection`].
    #[serde(deserialize_with = "semantic_section")]
    pub semantic: Option<SemanticSection>,
    pub kind: Option<Vec<String>>,
    /// Tuning and external metrics of the health score (`--health`,
    /// `--dashboard`); it does not switch either mode on.
    pub health: Option<cpd_core::health::HealthConfig>,
    #[serde(alias = "health-input")]
    pub health_input: Option<String>,
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
    #[serde(alias = "sarif-error-tokens")]
    pub sarif_error_tokens: Option<u32>,
    pub baseline: Option<String>,
    #[serde(alias = "fail-on-new-clones")]
    pub fail_on_new_clones: Option<u64>,
    #[serde(alias = "fail-on-empty")]
    pub fail_on_empty: Option<bool>,
    #[serde(alias = "baseline-from-ref")]
    pub baseline_from_ref: Option<String>,
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
    #[serde(alias = "ignore-identifiers")]
    pub ignore_identifiers: Option<bool>,
    #[serde(alias = "ignore-literals")]
    pub ignore_literals: Option<bool>,
    #[serde(alias = "ignore-annotations")]
    pub ignore_annotations: Option<bool>,
    #[serde(alias = "formats-exts")]
    pub formats_exts: Option<String>,
    #[serde(alias = "formats-names")]
    pub formats_names: Option<String>,
    #[serde(alias = "cross-formats")]
    pub cross_formats: Option<String>,
    #[serde(alias = "skip-local")]
    pub skip_local: Option<bool>,
    /// Isolation groups as nested arrays (matching the jscpd `skipIsolated`
    /// config shape): `[["packages/a", "packages/b"], ["libs/a", "libs/b"]]`.
    #[serde(alias = "skip-isolated")]
    pub skip_isolated: Option<Vec<Vec<String>>>,
    #[serde(alias = "exit-code")]
    pub exit_code: Option<i32>,
    #[serde(alias = "no-tips")]
    pub no_tips: Option<bool>,
    pub silent: Option<bool>,
    pub summary: Option<bool>,
    #[serde(alias = "summary-top")]
    pub summary_top: Option<usize>,
    #[serde(alias = "summary-by")]
    pub summary_by: Option<String>,
    pub history: Option<String>,
    #[serde(alias = "history-since")]
    pub history_since: Option<String>,
    #[serde(alias = "history-every")]
    pub history_every: Option<usize>,
    #[serde(alias = "history-limit")]
    pub history_limit: Option<usize>,
    #[serde(alias = "dead-code", alias = "basta")]
    pub dead_code: Option<DeadCodeSetting>,
    #[serde(alias = "dead-code-categories", alias = "deadCodeCategories")]
    pub dead_code_categories: Option<Vec<String>>,
    #[serde(alias = "min-confidence")]
    pub min_confidence: Option<u8>,
    pub entry: Option<Vec<String>>,
    #[serde(alias = "include-tests")]
    pub include_tests: Option<bool>,
    #[serde(alias = "include-entry-exports")]
    pub include_entry_exports: Option<bool>,
}

/// The `semantic` section of a config file. `true` and `false` are short for
/// `{"enabled": true}` / `{"enabled": false}`; an object turns the mode on only
/// with `"enabled": true`, like the `deadCode` section, so a project can keep
/// its settings in the file and choose the run on the command line.
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSection {
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "from_name")]
    pub provider: Option<cpd_semantic::Provider>,
    #[serde(default, deserialize_with = "from_name")]
    pub scope: Option<cpd_semantic::SemanticScope>,
    pub threshold: Option<f32>,
    pub model: Option<String>,
    pub url: Option<String>,
    /// Output dimensions to ask for (Matryoshka models); vectors longer than
    /// this are cut to it.
    pub dimensions: Option<u32>,
    /// Extra fields for the embeddings request, e.g. `{"task": "code2code.query"}`.
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
    /// Keep vectors in the user cache directory (default: true).
    pub cache: Option<bool>,
}

/// A string field parsed by its type's `FromStr`, whose error names the
/// accepted values.
fn from_name<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr<Err = String>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|name| name.parse().map_err(serde::de::Error::custom))
        .transpose()
}

fn semantic_section<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SemanticSection>, D::Error> {
    use serde::de::Error;
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Bool(enabled) => Ok(Some(SemanticSection {
            enabled: Some(enabled),
            ..SemanticSection::default()
        })),
        serde_json::Value::Object(map) if map.contains_key("apiKey") => Err(D::Error::custom(
            "apiKey is not read from config files; set the JSCPD_SEMANTIC_API_KEY environment variable",
        )),
        value @ serde_json::Value::Object(_) => serde_json::from_value(value)
            .map(Some)
            .map_err(D::Error::custom),
        _ => Err(D::Error::custom(
            "expected true, false or an object of semantic-clone settings",
        )),
    }
}

/// What a config file puts under `deadCode` (or `dead-code`, or `basta`).
///
/// `true` is the short form: run dead-code detection instead of clone
/// detection. An object is the dead-code section — everything the mode can be
/// told, framework definitions included — and turns the mode on only when it
/// says `"enabled": true`, so that a project can keep its dead-code settings
/// beside its clone settings and still choose the run on the command line.
#[derive(Debug, Clone, PartialEq)]
pub enum DeadCodeSetting {
    Enabled(bool),
    Section(Box<basta::section::Section>),
}

impl DeadCodeSetting {
    /// Whether the config file asks for a dead-code run.
    pub fn enabled(&self) -> bool {
        match self {
            Self::Enabled(enabled) => *enabled,
            Self::Section(section) => section.enabled.unwrap_or(false),
        }
    }

    pub fn section(&self) -> Option<&basta::section::Section> {
        match self {
            Self::Enabled(_) => None,
            Self::Section(section) => Some(section),
        }
    }
}

impl<'de> Deserialize<'de> for DeadCodeSetting {
    /// By hand rather than `#[serde(untagged)]`, which would answer a
    /// misspelled key inside the section with "data did not match any
    /// variant" instead of the name of the key.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Bool(enabled) => Ok(Self::Enabled(enabled)),
            value @ serde_json::Value::Object(_) => basta::section::Section::from_value(&value)
                .map(|section| Self::Section(Box::new(section)))
                .map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom(
                "expected true, false or an object of dead-code settings",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConfigSource {
    Explicit(PathBuf),
    AutoJscpdJson(PathBuf),
    AutoDotConfig(PathBuf),
    AutoPackageJson(PathBuf),
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
    /// A credential in the `semantic` section (`apiKey`, `api_key`, a
    /// `token`, ...). Its value is never kept or printed.
    SecretInConfig {
        source: PathBuf,
        field: String,
    },
}

impl ConfigDiagnostic {
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ConfigDiagnostic::IoError { .. }
                | ConfigDiagnostic::ParseError { .. }
                | ConfigDiagnostic::SecretInConfig { .. }
        )
    }

    /// Whether the run stops even for a config file that was found rather
    /// than named with --config: a key in a file is shared with everyone
    /// who can read it, and dropping it quietly would hide that.
    pub fn stops_any_run(&self) -> bool {
        matches!(self, ConfigDiagnostic::SecretInConfig { .. })
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
            ConfigDiagnostic::SecretInConfig { source, field } => {
                write!(
                    f,
                    "config file {}: '{}' is not read from config files, which everyone who can read the file shares; remove it (and rotate the key if the file was ever shared) and set the {} environment variable instead",
                    source.display(),
                    field,
                    cpd_semantic::API_KEY_ENV
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
    "maxGapLines",
    "similarity",
    "semantic",
    "kind",
    "health",
    "healthInput",
    "health-input",
    "mode",
    "format",
    "formats",
    "ignorePattern",
    "ignore",
    "pattern",
    "reporters",
    "output",
    "threshold",
    "sarifErrorTokens",
    "baseline",
    "failOnNewClones",
    "failOnEmpty",
    "baselineFromRef",
    "blame",
    "noGitignore",
    "followSymlinks",
    "noSymlinks",
    "noSymLinks",
    "maxSize",
    "noColors",
    "absolute",
    "ignoreCase",
    "ignoreIdentifiers",
    "ignoreLiterals",
    "ignoreAnnotations",
    "formatsExts",
    "formatsNames",
    "crossFormats",
    "skipLocal",
    "skipIsolated",
    "exitCode",
    "noTips",
    "silent",
    // kebab-case aliases (v4 compat)
    "min-tokens",
    "min-lines",
    "max-lines",
    "max-gap-lines",
    "max-size",
    "ignore-case",
    "ignore-identifiers",
    "ignore-literals",
    "ignore-annotations",
    "no-gitignore",
    "follow-symlinks",
    "skip-local",
    "skip-isolated",
    "exit-code",
    "no-colors",
    "no-tips",
    "formats-exts",
    "formats-names",
    "cross-formats",
    "ignore-pattern",
    "sarif-error-tokens",
    "fail-on-new-clones",
    "baseline-from-ref",
    "history",
    "historySince",
    "historyEvery",
    "historyLimit",
    "deadCode",
    "basta",
    "deadCodeCategories",
    "minConfidence",
    "entry",
    "includeTests",
    "includeEntryExports",
    "dead-code",
    "dead-code-categories",
    "min-confidence",
    "include-tests",
    "include-entry-exports",
    "history-since",
    "history-every",
    "history-limit",
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

/// Whether a config key names a credential: `apiKey`, `api_key`,
/// `openaiApiKey`, `token`, `authToken`, `secret`, `password`, ... but not
/// `minTokens`.
fn looks_like_secret(key: &str) -> bool {
    let key: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(key.as_str(), "key" | "token" | "authorization" | "bearer")
        || ["apikey", "accesstoken", "authtoken", "secret", "password"]
            .iter()
            .any(|suffix| key.ends_with(suffix))
}

/// Take every credential out of the `semantic` section before anything else
/// reads it, reporting each by name: the value never reaches a diagnostic,
/// and the report stops the run.
fn take_secrets(value: &mut serde_json::Value, source: &Path) -> Vec<ConfigDiagnostic> {
    let Some(section) = value
        .get_mut("semantic")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Vec::new();
    };
    let keys: Vec<String> = section
        .keys()
        .filter(|k| looks_like_secret(k))
        .cloned()
        .collect();
    keys.into_iter()
        .map(|key| {
            section.remove(&key);
            ConfigDiagnostic::SecretInConfig {
                source: source.to_path_buf(),
                field: format!("semantic.{key}"),
            }
        })
        .collect()
}

/// `value` with the value of every credential-looking key, at any depth,
/// replaced, for printing.
fn redact_secrets(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let v = match looks_like_secret(k) {
                        true => serde_json::Value::String("<redacted>".to_string()),
                        false => redact_secrets(v),
                    };
                    (k.clone(), v)
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_secrets).collect())
        }
        other => other.clone(),
    }
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
    // "crossFormats" / "cross-formats" type coercion: accept array forms,
    // convert to the canonical "a,b;c,d" string.
    for key in &["crossFormats", "cross-formats"] {
        coerce_cross_formats(obj, key);
    }
}

/// Coerce a crossFormats field from array forms to the canonical string.
///
/// Accepted forms:
///   - String: "javascript,typescript;css,scss" (canonical, no conversion)
///   - Array of strings: ["javascript,typescript", "css,scss"] → joined with ";"
///   - Array of arrays: [["javascript","typescript"],["css","scss"]] → inner
///     joined with ",", outer with ";"
fn coerce_cross_formats(obj: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    if let Some(val) = obj.remove(key) {
        let coerced = match val {
            serde_json::Value::Array(arr) => {
                let groups: Vec<String> = arr
                    .into_iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s),
                        serde_json::Value::Array(inner) => Some(
                            inner
                                .into_iter()
                                .filter_map(|f| f.as_str().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                                .join(","),
                        ),
                        _ => None,
                    })
                    .collect();
                serde_json::Value::String(groups.join(";"))
            }
            other => other,
        };
        obj.insert(key.to_string(), coerced);
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
    for file in [&mut cfg.baseline, &mut cfg.health_input]
        .into_iter()
        .flatten()
    {
        let path = PathBuf::from(&*file);
        if path.is_relative() {
            *file = config_dir.join(path).to_string_lossy().to_string();
        }
    }
    // The dead-code section's file, by the same rule as `baseline`.
    if let Some(DeadCodeSetting::Section(section)) = &mut cfg.dead_code
        && let Some(path) = &section.rust_diagnostics
        && path.is_relative()
    {
        section.rust_diagnostics = Some(config_dir.join(path));
    }
    // `ignore_pattern` is deliberately left untouched: its entries are
    // code-level regexes matched against source text, not paths, so they
    // must never be resolved against the config directory.
}

/// Load config from file if specified, or from .jscpd.json / .config/jscpd.json /
/// package.json jscpd key.
///
/// Only the working directory is searched, as jscpd v4 and the released v5 do:
/// a config beside the scanned path (e.g. `fixtures/.jscpd.json` with
/// `"silent": true`) must not change what `jscpd fixtures` prints.
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
    // An empty base keeps the candidates spelled `.jscpd.json`, not `./.jscpd.json`.
    for attempt in [
        try_load_jscpd_json,
        try_load_dot_config,
        try_load_package_json,
    ] {
        if let Some(result) = attempt(Path::new(""), &mut auto_diagnostics) {
            return result;
        }
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
                    diagnostics.push(ConfigDiagnostic::ParseError {
                        source: p.to_path_buf(),
                        line: Some(e.line()),
                        error: e.to_string(),
                    });
                    return ConfigResult {
                        config: ConfigFile::default(),
                        source: Some(ConfigSource::Explicit(p.to_path_buf())),
                        diagnostics,
                    };
                }
            };

            let mut value = value;
            diagnostics.extend(take_secrets(&mut value, p));
            diagnostics.extend(scan_unknown_fields(&value, p));

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
                    diagnostics.push(ConfigDiagnostic::ParseError {
                        source: p.to_path_buf(),
                        line: Some(e.line()),
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

fn try_load_jscpd_json(
    directory: &Path,
    auto_diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<ConfigResult> {
    let path = directory.join(".jscpd.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let value = parse_json_config(&content, &path, auto_diagnostics)?;
    Some(build_config_result(
        value,
        ConfigSource::AutoJscpdJson(path.clone()),
        &path,
    ))
}

/// Optional dot-config convention (https://dot-config.github.io/): projects can
/// keep tool configs in a .config/ subfolder instead of the repository root.
/// Checked only after .jscpd.json, so a root config always wins.
fn try_load_dot_config(
    directory: &Path,
    auto_diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<ConfigResult> {
    for candidate in [".config/jscpd.json", ".config/.jscpd.json"] {
        let path = directory.join(candidate);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let value = parse_json_config(&content, &path, auto_diagnostics)?;
            return Some(build_config_result(
                value,
                ConfigSource::AutoDotConfig(path.clone()),
                &path,
            ));
        }
    }
    None
}

fn try_load_package_json(
    directory: &Path,
    auto_diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<ConfigResult> {
    let path = directory.join("package.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let pkg = parse_json_config(&content, &path, auto_diagnostics)?;
    let jscpd_cfg = pkg.get("jscpd")?;
    Some(build_config_result(
        jscpd_cfg.clone(),
        ConfigSource::AutoPackageJson(path.clone()),
        &path,
    ))
}

fn parse_json_config(
    content: &str,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<serde_json::Value> {
    match serde_json::from_str(content) {
        Ok(v) => Some(v),
        Err(e) => {
            diagnostics.push(ConfigDiagnostic::ParseError {
                source: path.to_path_buf(),
                line: Some(e.line()),
                error: e.to_string(),
            });
            None
        }
    }
}

/// Test each top-level key of `value` on its own against `ConfigFile` (every
/// other field defaulted) and drop the ones that fail alone, so one field
/// with the wrong JSON type — `"entry": "src/index.js"` where an array is
/// expected — does not take every other, valid field down with it. Returns
/// `None` when `value` is not a JSON object, or when nothing fails in
/// isolation: the original error must then come from some combination of
/// fields rather than one field's own type, and the caller falls back to
/// discarding the file as before.
fn strip_invalid_fields(
    value: &serde_json::Value,
    path: &Path,
) -> Option<(serde_json::Value, Vec<ConfigDiagnostic>)> {
    let obj = value.as_object()?;
    let mut kept = serde_json::Map::new();
    let mut diagnostics = Vec::new();
    for (key, field_value) in obj {
        let mut single = serde_json::Map::new();
        single.insert(key.clone(), field_value.clone());
        match serde_json::from_value::<ConfigFile>(serde_json::Value::Object(single)) {
            Ok(_) => {
                kept.insert(key.clone(), field_value.clone());
            }
            Err(e) => {
                let rendered =
                    serde_json::to_string(&redact_secrets(field_value)).unwrap_or_default();
                let value = match rendered.chars().count() > 60 {
                    true => format!("{}…", rendered.chars().take(60).collect::<String>()),
                    false => rendered,
                };
                diagnostics.push(ConfigDiagnostic::InvalidValue {
                    source: path.to_path_buf(),
                    field: key.clone(),
                    value,
                    reason: e.to_string(),
                });
            }
        }
    }
    if diagnostics.is_empty() {
        return None;
    }
    Some((serde_json::Value::Object(kept), diagnostics))
}

fn build_config_result(
    mut value: serde_json::Value,
    source: ConfigSource,
    path: &Path,
) -> ConfigResult {
    let mut field_diagnostics = take_secrets(&mut value, path);
    field_diagnostics.extend(scan_unknown_fields(&value, path));
    normalize_v4_config(&mut value);

    match serde_json::from_value::<ConfigFile>(value.clone()) {
        Ok(mut cfg) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            resolve_config_paths(&mut cfg, &cwd);
            let mut validation_diagnostics = validate_config(&cfg, path);
            field_diagnostics.append(&mut validation_diagnostics);

            ConfigResult {
                config: cfg,
                source: Some(source),
                diagnostics: field_diagnostics,
            }
        }
        // A recognized field with a value of the wrong JSON type (a string
        // where `entry` needs an array, say) makes serde refuse the whole
        // object — the same as a config file it cannot parse at all — even
        // though every other field, `threshold` included, was perfectly
        // fine. Recover it the way an unknown field already is: warn about
        // the one field and keep the rest, instead of quietly falling back
        // to defaults for the whole file.
        Err(e) => match strip_invalid_fields(&value, path) {
            Some((stripped, mut invalid_field_diagnostics)) => {
                match serde_json::from_value::<ConfigFile>(stripped) {
                    Ok(mut cfg) => {
                        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                        resolve_config_paths(&mut cfg, &cwd);
                        let mut validation_diagnostics = validate_config(&cfg, path);
                        field_diagnostics.append(&mut invalid_field_diagnostics);
                        field_diagnostics.append(&mut validation_diagnostics);

                        ConfigResult {
                            config: cfg,
                            source: Some(source),
                            diagnostics: field_diagnostics,
                        }
                    }
                    // Stripping every individually-bad field still leaves
                    // something serde refuses: give up exactly as before.
                    Err(e) => ConfigResult {
                        config: ConfigFile::default(),
                        source: Some(source),
                        diagnostics: vec![ConfigDiagnostic::ParseError {
                            source: path.to_path_buf(),
                            line: Some(e.line()),
                            error: e.to_string(),
                        }],
                    },
                }
            }
            None => ConfigResult {
                config: ConfigFile::default(),
                source: Some(source),
                diagnostics: vec![ConfigDiagnostic::ParseError {
                    source: path.to_path_buf(),
                    line: Some(e.line()),
                    error: e.to_string(),
                }],
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Generate the four canonical boolean-flag tests for a CLI option.
    ///
    /// `$default`, `$set`, `$propagate`, `$from_config` are the test names;
    /// `$field` is both the `Cli` and `ConfigFile` field name;
    /// `$long` is the long CLI flag (without the leading `--`).
    macro_rules! bool_flag_tests {
        ($default:ident, $set:ident, $propagate:ident, $from_config:ident, $field:ident, $long:literal) => {
            #[test]
            fn $default() {
                let cli = Cli::parse_from(["cpd", "."]);
                assert!(!cli.$field);
            }

            #[test]
            fn $set() {
                let cli = Cli::parse_from(["cpd", concat!("--", $long), "."]);
                assert!(cli.$field);
            }

            #[test]
            fn $propagate() {
                let cli = Cli::parse_from(["cpd", concat!("--", $long), "."]);
                let config = ConfigFile::default();
                let opts = crate::options::Options::from_cli_and_config(&cli, &config);
                assert!(opts.$field);
            }

            #[test]
            fn $from_config() {
                let config = ConfigFile {
                    $field: Some(true),
                    ..Default::default()
                };
                let cli = Cli::parse_from(["cpd", "."]);
                let opts = crate::options::Options::from_cli_and_config(&cli, &config);
                assert!(opts.$field);
            }
        };
    }

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
    fn sarif_error_tokens_default_is_none() {
        let cli = Cli::parse_from(["cpd", "."]);
        assert_eq!(cli.sarif_error_tokens, None);
    }

    #[test]
    fn sarif_error_tokens_override() {
        let cli = Cli::parse_from(["cpd", "--sarif-error-tokens", "200", "."]);
        assert_eq!(cli.sarif_error_tokens, Some(200));
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

    fn paths_from(cli_args: &[&str], config_paths: Option<Vec<String>>) -> Vec<PathBuf> {
        let cli = Cli::parse_from(cli_args);
        let config = ConfigFile {
            path: config_paths,
            ..Default::default()
        };
        crate::options::Options::from_cli_and_config(&cli, &config).paths
    }

    #[test]
    fn config_path_used_when_cli_paths_empty() {
        assert_eq!(
            paths_from(&["cpd"], Some(vec!["./fixtures".to_string()])),
            vec![PathBuf::from("./fixtures")]
        );
    }

    #[test]
    fn cli_paths_override_config_path() {
        assert_eq!(
            paths_from(
                &["cpd", "/tmp/project"],
                Some(vec!["./fixtures".to_string()])
            ),
            vec![PathBuf::from("/tmp/project")]
        );
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
    fn max_gap_lines_flag_and_config() {
        let cli = Cli::parse_from(["cpd", "--max-gap-lines", "2", "."]);
        assert_eq!(cli.max_gap_lines, Some(2));
        let opts = crate::options::Options::from_cli_and_config(&cli, &ConfigFile::default());
        assert_eq!(opts.max_gap_lines, 2);

        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &ConfigFile::default());
        assert_eq!(opts.max_gap_lines, 0, "off by default");

        let config = ConfigFile {
            max_gap_lines: Some(3),
            ..Default::default()
        };
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(opts.max_gap_lines, 3);
        let v: ConfigFile = serde_json::from_str(r#"{"max-gap-lines": 1}"#).unwrap();
        assert_eq!(v.max_gap_lines, Some(1));
    }

    #[test]
    fn similarity_flag_and_config() {
        let cli = Cli::parse_from(["cpd", "--similarity", "0.85", "."]);
        assert_eq!(cli.similarity, Some(0.85));
        let opts = crate::options::Options::from_cli_and_config(&cli, &ConfigFile::default());
        assert_eq!(opts.similarity, 0.85);

        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &ConfigFile::default());
        assert_eq!(opts.similarity, 1.0, "1 = exact matches only, the default");

        let v: ConfigFile = serde_json::from_str(r#"{"similarity": 0.9}"#).unwrap();
        let opts = crate::options::Options::from_cli_and_config(&cli, &v);
        assert_eq!(opts.similarity, 0.9);
    }

    #[test]
    fn semantic_flags_and_config_section() {
        let options = |args: &[&str], config: &str| {
            let cli = Cli::parse_from(args);
            let file: ConfigFile = serde_json::from_str(config).unwrap();
            crate::options::Options::from_cli_and_config(&cli, &file).semantic
        };
        assert_eq!(options(&["cpd", "."], "{}"), None, "off by default");
        assert_eq!(
            options(&["cpd", "--semantic-model", "m", "."], "{}"),
            None,
            "tuning flags alone do not switch it on"
        );

        let defaults = options(&["cpd", "--semantic", "."], "{}").unwrap();
        assert_eq!(
            defaults,
            cpd_semantic::SemanticOptions {
                on_command_line: true,
                ..cpd_semantic::SemanticOptions::default()
            }
        );
        assert_eq!(defaults.threshold, 0.6);
        assert_eq!(defaults.url, "http://localhost:11434/v1");

        assert!(options(&["cpd", "."], r#"{"semantic": true}"#).is_some());
        assert!(options(&["cpd", "."], r#"{"semantic": {"model": "m"}}"#).is_none());
        let section = r#"{"semantic": {"enabled": true, "threshold": 0.7, "model": "file", "url": "http://h/v1", "dimensions": 256, "params": {"task": "code2code.query"}, "cache": false}}"#;
        let from_file = options(&["cpd", "."], section).unwrap();
        assert_eq!(
            (
                from_file.threshold,
                from_file.model.as_str(),
                from_file.url.as_str()
            ),
            (0.7, "file", "http://h/v1")
        );
        assert_eq!((from_file.dimensions, from_file.cache), (Some(256), false));
        assert_eq!(from_file.params["task"], "code2code.query");
        assert!(
            from_file.url_from_config && !from_file.on_command_line,
            "the URL and the switch both came from the file"
        );

        let flags = [
            "cpd",
            "--semantic-threshold",
            "0.8",
            "--semantic-model",
            "flag",
            "--semantic-url",
            "http://f/v1",
            ".",
        ];
        let overridden = options(&flags, section).unwrap();
        assert_eq!(
            (
                overridden.threshold,
                overridden.model.as_str(),
                overridden.url.as_str()
            ),
            (0.8, "flag", "http://f/v1"),
            "flags win over the file"
        );
        assert!(!overridden.url_from_config, "the URL was typed");

        for (config, error) in [
            (
                r#"{"semantic": {"enabled": true, "apiKey": "k"}}"#,
                "JSCPD_SEMANTIC_API_KEY",
            ),
            (r#"{"semantic": {"modle": "m"}}"#, "unknown field `modle`"),
            (r#"{"semantic": 1}"#, "expected true, false or an object"),
        ] {
            let err = serde_json::from_str::<ConfigFile>(config)
                .unwrap_err()
                .to_string();
            assert!(err.contains(error), "{config}: {err}");
        }
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

    bool_flag_tests!(
        no_tips_defaults_to_false,
        no_tips_flag_set,
        no_tips_propagates_to_options,
        no_tips_from_config,
        no_tips,
        "no-tips"
    );

    #[test]
    fn no_tips_default_and_env_vars_in_options() {
        // The default and the env-var cases share one test so their env
        // mutations cannot race under `cargo test`'s threaded runner.
        // SAFETY: no other test touches these variables.
        const VARS: [&str; 2] = ["CI", "JSCPD_NO_TIPS"];
        let no_tips = || {
            let cli = Cli::parse_from(["cpd", "."]);
            let config = ConfigFile::default();
            crate::options::Options::from_cli_and_config(&cli, &config).no_tips
        };
        for var in VARS {
            unsafe { std::env::remove_var(var) };
        }
        assert!(!no_tips(), "bare default should be false");
        // NO_COLOR only asks for plain output; it must not hide the tips.
        unsafe { std::env::set_var("NO_COLOR", "1") };
        assert!(!no_tips(), "NO_COLOR must not enable no_tips");
        unsafe { std::env::remove_var("NO_COLOR") };
        for var in VARS {
            unsafe { std::env::set_var(var, "1") };
            assert!(no_tips(), "{var}=1 should enable no_tips");
            unsafe { std::env::remove_var(var) };
        }
    }

    bool_flag_tests!(
        silent_flag_defaults_to_false,
        silent_flag_set,
        silent_propagates_to_options,
        silent_from_config,
        silent,
        "silent"
    );

    #[test]
    fn silent_short_alias() {
        let cli = Cli::parse_from(["cpd", "-s", "."]);
        assert!(cli.silent);
    }

    fn assert_config_overrides_default<T: PartialEq + std::fmt::Debug>(
        field: impl FnOnce(&mut ConfigFile),
        extract: impl Fn(&crate::options::Options) -> &T,
        expected: T,
        msg: &str,
    ) {
        let mut config = ConfigFile::default();
        field(&mut config);
        let cli = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&cli, &config);
        assert_eq!(extract(&opts), &expected, "{}", msg);
    }

    #[test]
    fn config_min_tokens_overrides_default() {
        assert_config_overrides_default(
            |c| c.min_tokens = Some(30),
            |o| &o.min_tokens,
            30usize,
            "config min_tokens should override default",
        );
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
        assert_config_overrides_default(
            |c| c.min_lines = Some(10),
            |o| &o.min_lines,
            10usize,
            "config min_lines should override default",
        );
    }

    #[test]
    fn config_reporters_override_default() {
        assert_config_overrides_default(
            |c| c.reporters = Some(vec!["json".to_string(), "html".to_string()]),
            |o| &o.reporters,
            vec!["json".to_string(), "html".to_string()],
            "config reporters should override default",
        );
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
        assert_config_overrides_default(
            |c| c.output = Some("my-reports".to_string()),
            |o| &o.output_dir,
            PathBuf::from("my-reports"),
            "config output should override default",
        );
    }

    #[test]
    fn config_mode_overrides_default() {
        assert_config_overrides_default(
            |c| c.mode = Some("strict".to_string()),
            |o| &o.mode,
            cpd_tokenizer::tokenizer::Mode::Strict,
            "config mode should override default",
        );
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

    bool_flag_tests!(
        absolute_defaults_to_false,
        absolute_flag_long,
        absolute_propagates_to_options,
        absolute_from_config,
        absolute,
        "absolute"
    );

    bool_flag_tests!(
        ignore_case_defaults_to_false,
        ignore_case_flag,
        ignore_case_propagates_to_options,
        ignore_case_from_config,
        ignore_case,
        "ignore-case"
    );

    bool_flag_tests!(
        ignore_identifiers_defaults_to_false,
        ignore_identifiers_flag,
        ignore_identifiers_propagates_to_options,
        ignore_identifiers_from_config,
        ignore_identifiers,
        "ignore-identifiers"
    );

    bool_flag_tests!(
        ignore_literals_defaults_to_false,
        ignore_literals_flag,
        ignore_literals_propagates_to_options,
        ignore_literals_from_config,
        ignore_literals,
        "ignore-literals"
    );

    bool_flag_tests!(
        ignore_annotations_defaults_to_false,
        ignore_annotations_flag,
        ignore_annotations_propagates_to_options,
        ignore_annotations_from_config,
        ignore_annotations,
        "ignore-annotations"
    );

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
        assert_config_overrides_default(
            |c| c.formats_exts = Some("javascript:es,mjs".to_string()),
            |o| &o.formats_exts,
            {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "javascript".to_string(),
                    vec!["es".to_string(), "mjs".to_string()],
                );
                map
            },
            "config formats_exts should parse into map",
        );
    }

    #[test]
    fn formats_names_from_config() {
        assert_config_overrides_default(
            |c| c.formats_names = Some("make:Makefile,GNUmakefile;docker:Dockerfile".to_string()),
            |o| &o.formats_names,
            {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "make".to_string(),
                    vec!["Makefile".to_string(), "GNUmakefile".to_string()],
                );
                map.insert("docker".to_string(), vec!["Dockerfile".to_string()]);
                map
            },
            "config formats_names should parse into map",
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
    fn cross_formats_flag_parsing() {
        let cli = Cli::parse_from(["cpd", "--cross-formats", "javascript,typescript", "."]);
        assert_eq!(cli.cross_formats, Some("javascript,typescript".to_string()));
    }

    #[test]
    fn parse_cross_formats_groups() {
        assert_eq!(
            parse_cross_formats("javascript, typescript ; css,scss"),
            vec![
                vec!["javascript".to_string(), "typescript".to_string()],
                vec!["css".to_string(), "scss".to_string()],
            ]
        );
        assert_eq!(parse_cross_formats(""), Vec::<Vec<String>>::new());
        assert_eq!(parse_cross_formats(";;"), Vec::<Vec<String>>::new());
    }

    #[test]
    fn parse_cross_formats_preset() {
        let expected_js_ts = vec![
            "javascript".to_string(),
            "jsx".to_string(),
            "typescript".to_string(),
            "tsx".to_string(),
        ];
        assert_eq!(parse_cross_formats("js-ts"), vec![expected_js_ts.clone()]);
        assert_eq!(
            parse_cross_formats("js-ts;css,scss"),
            vec![
                expected_js_ts.clone(),
                vec!["css".to_string(), "scss".to_string()]
            ]
        );
        // Preset inside a group merges with the extra format, deduped.
        assert_eq!(
            parse_cross_formats("js-ts,vue,typescript"),
            vec![{
                let mut g = expected_js_ts;
                g.push("vue".to_string());
                g
            }]
        );
    }

    #[test]
    fn parse_cross_formats_single_format_group_dropped() {
        assert_eq!(
            parse_cross_formats("javascript;css,scss"),
            vec![vec!["css".to_string(), "scss".to_string()]]
        );
    }

    #[test]
    fn parse_cross_formats_overlapping_groups_merged() {
        let groups = parse_cross_formats("javascript,typescript;typescript,tsx");
        assert_eq!(groups.len(), 1, "overlapping groups must merge");
        let merged = &groups[0];
        for format in ["javascript", "typescript", "tsx"] {
            assert!(merged.contains(&format.to_string()), "missing {format}");
        }
    }

    /// `--cross-formats javascript,typescript` on the CLI, merged with `config`.
    fn cross_formats_with(config: ConfigFile) -> Vec<Vec<String>> {
        let cli = Cli::parse_from(["cpd", "--cross-formats", "javascript,typescript", "."]);
        crate::options::Options::from_cli_and_config(&cli, &config).cross_formats
    }

    #[test]
    fn cross_formats_propagates_to_options() {
        assert_eq!(
            cross_formats_with(ConfigFile::default()),
            vec![vec!["javascript".to_string(), "typescript".to_string()]]
        );
    }

    #[test]
    fn cli_cross_formats_overrides_config() {
        let config = ConfigFile {
            cross_formats: Some("css,scss".to_string()),
            ..Default::default()
        };
        assert_eq!(
            cross_formats_with(config),
            vec![vec!["javascript".to_string(), "typescript".to_string()]]
        );
    }

    #[test]
    fn cross_formats_from_config() {
        assert_config_overrides_default(
            |c| c.cross_formats = Some("javascript,typescript".to_string()),
            |o| &o.cross_formats,
            vec![vec!["javascript".to_string(), "typescript".to_string()]],
            "config cross_formats should parse into groups",
        );
    }

    #[test]
    fn cross_formats_config_coercion_shapes() {
        // string / array-of-strings / array-of-arrays all normalize to the
        // canonical string before ConfigFile deserialization.
        let cases = [
            serde_json::json!({"crossFormats": "javascript,typescript;css,scss"}),
            serde_json::json!({"crossFormats": ["javascript,typescript", "css,scss"]}),
            serde_json::json!({"crossFormats": [["javascript","typescript"],["css","scss"]]}),
            serde_json::json!({"cross-formats": [["javascript","typescript"],["css","scss"]]}),
        ];
        for mut value in cases {
            normalize_v4_config(&mut value);
            let cfg: ConfigFile = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                cfg.cross_formats,
                Some("javascript,typescript;css,scss".to_string()),
                "shape {value} must coerce to canonical string"
            );
        }
    }

    #[test]
    fn cross_formats_is_known_config_field() {
        for key in ["crossFormats", "cross-formats"] {
            let value = serde_json::json!({key: "javascript,typescript"});
            let diags = scan_unknown_fields(&value, Path::new(".jscpd.json"));
            assert!(
                diags.is_empty(),
                "'{key}' must not be reported as unknown: {diags:?}"
            );
        }
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
        let expected_fields = super::KNOWN_CONFIG_FIELDS;
        for field in expected_fields {
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

    fn assert_unknown_field(
        value: serde_json::Value,
        expected_field: &str,
        expected_hint: Option<&str>,
    ) {
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert_eq!(diagnostics.len(), 1);
        match &diagnostics[0] {
            ConfigDiagnostic::UnknownField {
                field,
                migration_hint,
                ..
            } => {
                assert_eq!(field, expected_field);
                assert_eq!(migration_hint.as_deref(), expected_hint);
            }
            _ => panic!("Expected UnknownField diagnostic"),
        }
    }

    #[test]
    fn scan_unknown_fields_detects_unknown() {
        assert_unknown_field(
            serde_json::json!({"minTokens": 50, "unknownField": true}),
            "unknownField",
            None,
        );
    }

    fn assert_store_migration_hint(value: serde_json::Value, expected_hint: Option<&str>) {
        assert_unknown_field(value, "store", expected_hint);
    }

    #[test]
    fn scan_unknown_fields_v4_migration_hint() {
        assert_store_migration_hint(
            serde_json::json!({"store": "leveldb"}),
            Some("removed from config file in v5, use --store CLI flag"),
        );
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
        assert_store_migration_hint(
            serde_json::json!({"store": "leveldb"}),
            Some("removed from config file in v5, use --store CLI flag"),
        );
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
    fn a_wrong_typed_known_field_does_not_take_the_rest_of_the_config_down() {
        // `entry` wants an array; a string is a type error that would
        // otherwise make serde refuse the whole object, `threshold` included.
        let value: serde_json::Value =
            serde_json::from_str(r#"{"entry": "src/index.js", "threshold": 1}"#).unwrap();
        let result = build_config_result(
            value,
            ConfigSource::AutoJscpdJson(PathBuf::from(".jscpd.json")),
            Path::new(".jscpd.json"),
        );
        assert_eq!(result.config.threshold, Some(1.0), "{:?}", result.config);
        assert!(
            result.diagnostics.iter().any(
                |d| matches!(d, ConfigDiagnostic::InvalidValue { field, .. } if field == "entry")
            ),
            "{:?}",
            result.diagnostics
        );
        assert!(
            !result.diagnostics.iter().any(|d| d.is_fatal()),
            "a single bad field must not be fatal: {:?}",
            result.diagnostics
        );
    }

    fn config_from(json: &str) -> ConfigResult {
        build_config_result(
            serde_json::from_str(json).unwrap(),
            ConfigSource::AutoJscpdJson(PathBuf::from(".jscpd.json")),
            Path::new(".jscpd.json"),
        )
    }

    #[test]
    fn the_dead_code_key_is_a_switch_or_a_section_under_any_of_its_names() {
        let switch = config_from(r#"{"deadCode": true}"#).config;
        assert_eq!(switch.dead_code, Some(DeadCodeSetting::Enabled(true)));
        assert!(switch.dead_code.as_ref().unwrap().enabled());
        assert!(switch.dead_code.as_ref().unwrap().section().is_none());

        for key in ["deadCode", "dead-code", "basta"] {
            let result = config_from(&format!(
                r#"{{"{key}": {{"minConfidence": 80, "framework": ["next"]}}, "threshold": 5}}"#
            ));
            assert!(
                result.diagnostics.is_empty(),
                "{key}: {:?}",
                result.diagnostics
            );
            let setting = result.config.dead_code.expect(key);
            // Settings alone do not change which mode a plain `jscpd` runs.
            assert!(!setting.enabled(), "{key}");
            let section = setting.section().expect(key);
            assert_eq!(section.min_confidence, Some(80), "{key}");
            assert_eq!(
                section.framework.as_deref(),
                Some(&["next".to_string()][..])
            );
        }

        let on = config_from(r#"{"deadCode": {"enabled": true}}"#).config;
        assert!(on.dead_code.unwrap().enabled());
    }

    #[test]
    fn a_misspelled_key_in_the_section_is_named_and_the_rest_of_the_config_survives() {
        let result = config_from(r#"{"deadCode": {"minConfidense": 80}, "threshold": 5}"#);
        assert_eq!(result.config.threshold, Some(5.0));
        assert_eq!(result.config.dead_code, None);
        let named = result.diagnostics.iter().any(|d| {
            matches!(d, ConfigDiagnostic::InvalidValue { field, reason, .. }
                if field == "deadCode" && reason.contains("minConfidense"))
        });
        assert!(named, "{:?}", result.diagnostics);

        let wrong = config_from(r#"{"deadCode": "yes"}"#);
        assert!(
            wrong.diagnostics.iter().any(|d| {
                matches!(d, ConfigDiagnostic::InvalidValue { reason, .. }
                    if reason.contains("true, false or an object"))
            }),
            "{:?}",
            wrong.diagnostics
        );
    }

    #[test]
    fn a_flag_beats_the_section_and_the_section_beats_the_flat_keys() {
        let config = config_from(
            r#"{
                "minConfidence": 50,
                "entry": ["flat/**"],
                "includeTests": true,
                "deadCodeCategories": ["unused-import"],
                "deadCode": {
                    "minConfidence": 80,
                    "entry": ["section/**"],
                    "includeTests": false,
                    "categories": ["unused-file"],
                    "minLines": 3
                }
            }"#,
        )
        .config;

        let plain = Cli::parse_from(["cpd", "."]);
        let opts = crate::options::Options::from_cli_and_config(&plain, &config);
        assert!(!opts.dead_code, "settings do not switch the mode on");
        assert_eq!(opts.min_confidence, Some(80));
        assert_eq!(opts.entry, ["section/**"]);
        assert!(!opts.include_tests);
        assert_eq!(opts.dead_code_categories, ["unused-file"]);
        assert_eq!(opts.dead_code_section.min_lines, Some(3));

        let flagged = Cli::parse_from([
            "cpd",
            "--dead-code",
            "--min-confidence",
            "95",
            "--entry",
            "flag/**",
            ".",
        ]);
        let opts = crate::options::Options::from_cli_and_config(&flagged, &config);
        assert!(opts.dead_code);
        assert_eq!(opts.min_confidence, Some(95));
        assert_eq!(opts.entry, ["flag/**"]);

        // A config with no section still reads its flat keys, as before.
        let flat = config_from(r#"{"minConfidence": 50, "entry": ["flat/**"]}"#).config;
        let opts = crate::options::Options::from_cli_and_config(&plain, &flat);
        assert_eq!(opts.min_confidence, Some(50));
        assert_eq!(opts.entry, ["flat/**"]);
    }

    #[test]
    fn strip_invalid_fields_keeps_the_good_ones_and_reports_the_bad_one() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"entry": "src/index.js", "threshold": 1}"#).unwrap();
        let (stripped, diagnostics) = strip_invalid_fields(&value, Path::new(".jscpd.json"))
            .expect("entry is individually invalid");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(stripped, serde_json::json!({"threshold": 1}));
    }

    #[test]
    fn strip_invalid_fields_gives_up_when_nothing_fails_alone() {
        // Not an object at all: there is no per-field failure to isolate.
        let value = serde_json::json!(["not", "an", "object"]);
        assert!(strip_invalid_fields(&value, Path::new(".jscpd.json")).is_none());
    }

    #[test]
    fn a_key_in_the_semantic_section_stops_the_run_and_is_never_printed() {
        let path = Path::new(".jscpd.json");
        for config in [
            r#"{"semantic": {"enabled": true, "apiKey": "sk-live-SECRET123"}}"#,
            r#"{"semantic": {"enabled": true, "api_key": "sk-live-SECRET123"}}"#,
            r#"{"semantic": {"token": "sk-live-SECRET123", "model": 5}}"#,
        ] {
            let value: serde_json::Value = serde_json::from_str(config).unwrap();
            for result in [
                build_config_result(
                    value.clone(),
                    ConfigSource::AutoJscpdJson(path.to_path_buf()),
                    path,
                ),
                {
                    let dir = std::env::temp_dir()
                        .join(format!("cpd-secret-config-{}", std::process::id()));
                    std::fs::create_dir_all(&dir).unwrap();
                    let file = dir.join("jscpd.json");
                    std::fs::write(&file, config).unwrap();
                    let result = load_explicit_config(&file);
                    std::fs::remove_dir_all(&dir).unwrap();
                    result
                },
            ] {
                assert!(
                    result.diagnostics.iter().any(|d| d.stops_any_run()),
                    "{config}: {:?}",
                    result.diagnostics
                );
                for d in &result.diagnostics {
                    assert!(!d.to_string().contains("SECRET123"), "{config}: {d}");
                }
            }
        }
    }

    #[test]
    fn credentials_are_told_apart_from_settings() {
        for key in [
            "apiKey",
            "api_key",
            "OPENAI_API_KEY",
            "token",
            "authToken",
            "secret",
        ] {
            assert!(looks_like_secret(key), "{key}");
        }
        for key in [
            "minTokens",
            "maxTokens",
            "model",
            "url",
            "keyboard",
            "enabled",
        ] {
            assert!(!looks_like_secret(key), "{key}");
        }
        assert_eq!(
            redact_secrets(
                &serde_json::json!({"a": {"apiKey": "k", "n": 1}, "b": [{"token": "t"}]})
            ),
            serde_json::json!({"a": {"apiKey": "<redacted>", "n": 1}, "b": [{"token": "<redacted>"}]})
        );
    }

    #[test]
    fn an_unrecoverable_config_still_falls_back_to_a_parse_error() {
        // `threshold` alone would parse fine (`"invalid type: string, expected
        // a sequence"` is not how a bad threshold fails), so this simulates
        // stripping finding nothing wrong per field: the object as a whole
        // still fails, which must still be reported, not silently dropped.
        let value = serde_json::json!(["not", "an", "object"]);
        let result = build_config_result(
            value,
            ConfigSource::AutoJscpdJson(PathBuf::from(".jscpd.json")),
            Path::new(".jscpd.json"),
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| matches!(d, ConfigDiagnostic::ParseError { .. })),
            "{:?}",
            result.diagnostics
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
    fn config_file_sarif_error_tokens_both_spellings() {
        let v: ConfigFile = serde_json::from_str(r#"{"sarifErrorTokens": 150}"#).unwrap();
        assert_eq!(v.sarif_error_tokens, Some(150));
        let v: ConfigFile = serde_json::from_str(r#"{"sarif-error-tokens": 150}"#).unwrap();
        assert_eq!(v.sarif_error_tokens, Some(150));
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
    fn config_file_skip_isolated_nested_arrays() {
        let v: ConfigFile =
            serde_json::from_str(r#"{"skipIsolated": [["packages/a", "packages/b"]]}"#).unwrap();
        assert_eq!(
            v.skip_isolated,
            Some(vec![vec![
                "packages/a".to_string(),
                "packages/b".to_string()
            ]])
        );
        let v: ConfigFile =
            serde_json::from_str(r#"{"skip-isolated": [["libs/a", "libs/b"]]}"#).unwrap();
        assert_eq!(
            v.skip_isolated,
            Some(vec![vec!["libs/a".to_string(), "libs/b".to_string()]])
        );
    }

    #[test]
    fn parse_skip_isolated_groups() {
        assert_eq!(
            parse_skip_isolated("packages/a|packages/b, libs/a | libs/b"),
            vec![
                vec!["packages/a".to_string(), "packages/b".to_string()],
                vec!["libs/a".to_string(), "libs/b".to_string()],
            ]
        );
        assert_eq!(parse_skip_isolated(""), Vec::<Vec<String>>::new());
        assert_eq!(parse_skip_isolated(",,"), Vec::<Vec<String>>::new());
        // Single-folder groups can never isolate anything — dropped.
        assert_eq!(
            parse_skip_isolated("packages/a,libs/a|libs/b"),
            vec![vec!["libs/a".to_string(), "libs/b".to_string()]]
        );
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

    /// Assert that `scan_unknown_fields` produces no diagnostics for `value`.
    fn assert_no_unknown_diagnostics(value: serde_json::Value, message: &str) {
        let diagnostics = scan_unknown_fields(&value, Path::new("test.json"));
        assert!(
            diagnostics.is_empty(),
            "{} should produce no diagnostics, got: {:?}",
            message,
            diagnostics
        );
    }
    #[test]
    fn scan_unknown_fields_debug_silently_ignored() {
        assert_no_unknown_diagnostics(
            serde_json::json!({"debug": true, "verbose": false}),
            "debug and verbose",
        );
    }

    // v4 compat: "config" and "xslHref" are silently ignored
    #[test]
    fn scan_unknown_fields_v4_silent_fields() {
        assert_no_unknown_diagnostics(
            serde_json::json!({"config": ".jscpd.json", "xslHref": "report.xsl", "gitignore": true}),
            "config, xslHref, gitignore",
        );
    }

    // v4 compat: "ignore" is now a known field, not an unknown field
    #[test]
    fn scan_known_fields_ignore_is_known() {
        assert_no_unknown_diagnostics(serde_json::json!({"ignore": ["**/dist/**"]}), "ignore");
    }

    // v4 compat: "formats" is now a known field (alias), not an unknown field
    #[test]
    fn scan_known_fields_formats_is_known() {
        assert_no_unknown_diagnostics(serde_json::json!({"formats": ["typescript"]}), "formats");
    }

    #[test]
    fn scan_known_fields_fail_on_empty_is_known() {
        assert_no_unknown_diagnostics(serde_json::json!({"failOnEmpty": true}), "failOnEmpty");
    }

    #[test]
    fn fail_on_empty_cli_flag() {
        let cli = Cli::parse_from(["cpd", "--fail-on-empty", "."]);
        assert!(cli.fail_on_empty);
        let cli = Cli::parse_from(["cpd", "."]);
        assert!(!cli.fail_on_empty);
    }

    #[test]
    fn history_cli_flags() {
        let cli = Cli::parse_from([
            "cpd",
            "--history",
            "v5.0.0..HEAD",
            "--history-every",
            "3",
            "--history-limit",
            "10",
            ".",
        ]);
        assert_eq!(cli.history.as_deref(), Some("v5.0.0..HEAD"));
        assert_eq!(cli.history_every, Some(3));
        assert_eq!(cli.history_limit, Some(10));
        let cli = Cli::parse_from(["cpd", "--history-since", "2026-01-01", "."]);
        assert_eq!(cli.history_since.as_deref(), Some("2026-01-01"));
        assert!(cli.history.is_none());
    }

    #[test]
    fn scan_known_fields_history_keys_are_known() {
        assert_no_unknown_diagnostics(
            serde_json::json!({"history": "v5..HEAD", "historySince": "2026-01-01", "historyEvery": 2, "historyLimit": 5}),
            "history",
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
    #[allow(non_snake_case)]
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
    #[allow(non_snake_case)]
    fn normalize_noSymlinks_false_means_follow() {
        let mut value = serde_json::json!({"noSymlinks": false});
        normalize_v4_config(&mut value);
        assert!(value.get("noSymlinks").is_none());
        assert_eq!(value.get("followSymlinks"), Some(&serde_json::json!(true)));
    }

    #[test]
    #[allow(non_snake_case)]
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
