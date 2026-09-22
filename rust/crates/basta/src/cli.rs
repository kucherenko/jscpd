//! The `basta` command line.
//!
//! Flag names follow jscpd's wherever the two tools mean the same thing
//! (`--ignore`, `--format`, `-r`, `-o`, `--threshold`, `--silent`, `-c`), so
//! muscle memory carries over. What a project always wants lives in the
//! dead-code section of its jscpd config ([`crate::section`]), which both
//! tools read; a flag here overrides it for one run.

use crate::config::{BastaConfig, RustDiagnostics};
use crate::framework::{Registry, Sources};
use crate::run::OutputOptions;
use crate::section::Section;
use clap::Parser;
use cpd_core::deadcode::Category;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "basta",
    version,
    about = "Find dead code: unused files, exports, declarations and imports",
    long_about = "basta finds code nothing runs.\n\nIt builds the import graph from the project's entry points, walks it, and \
reports what it never reaches. Every finding carries a confidence score and \
the reasons it might be wrong, because static analysis of JavaScript and \
Python cannot be certain and a tool that pretends otherwise gets ignored."
)]
pub struct Cli {
    /// Paths to scan (default: the current directory)
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Findings to report: unused-file, unused-export, unused-symbol,
    /// unused-import, unused-member, or `all`
    #[arg(long, value_name = "LIST", value_delimiter = ',')]
    pub categories: Vec<String>,

    /// Drop findings below this confidence, 0-100 (default: 60)
    #[arg(long, value_name = "N")]
    pub min_confidence: Option<u8>,

    /// Only report declarations spanning at least this many lines
    #[arg(long, value_name = "N")]
    pub min_lines: Option<u32>,

    /// Treat files matching this glob as entry points (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub entry: Vec<String>,

    /// jscpd config file to read the dead-code section from (default:
    /// .jscpd.json, .config/jscpd.json or package.json in the working
    /// directory)
    #[arg(short = 'c', long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Framework definitions to add to the built-in ones, as YAML or JSON
    /// (default: basta.frameworks.{yaml,yml,json} in the working directory)
    #[arg(long, value_name = "FILE")]
    pub frameworks_config: Option<PathBuf>,

    /// Treat this framework as present at the scan roots (repeatable)
    #[arg(long, value_name = "NAME")]
    pub framework: Vec<String>,

    /// Do not detect frameworks; only manifests, conventions and --entry
    /// decide where the program starts
    #[arg(long)]
    pub no_frameworks: bool,

    /// Rust dead code from the compiler: a file of `cargo check
    /// --message-format=json` output, or `-` to read it from stdin
    #[arg(long, value_name = "FILE")]
    pub rust_diagnostics: Option<PathBuf>,

    /// Skip files matching this glob (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub ignore: Vec<String>,

    /// Report dead code inside test, fixture and example files
    #[arg(long)]
    pub include_tests: bool,

    /// Report exports of entry-point files, which are usually a public API
    #[arg(long)]
    pub include_entry_exports: bool,

    /// Restrict the scan to these formats (repeatable)
    #[arg(long = "format", value_name = "FORMAT")]
    pub formats: Vec<String>,

    /// Extra extension mappings, e.g. "typescript:mts,cts;python:pyw"
    #[arg(long, value_name = "MAP")]
    pub formats_exts: Option<String>,

    /// Reporters to run, comma-separated (default: console)
    #[arg(short = 'r', long, value_name = "LIST", value_delimiter = ',')]
    pub reporters: Vec<String>,

    /// Directory for reporters that write files
    #[arg(short = 'o', long, value_name = "DIR", default_value = "report")]
    pub output: PathBuf,

    /// Fail when dead code exceeds this percentage of the codebase
    #[arg(long, value_name = "PERCENT")]
    pub threshold: Option<f64>,

    /// Exit with this code when anything is found
    #[arg(long, value_name = "CODE")]
    pub exit_code: Option<i32>,

    /// Do not read .gitignore while walking
    #[arg(long)]
    pub no_gitignore: bool,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Skip files larger than this, e.g. 500kb
    #[arg(long, value_name = "SIZE")]
    pub max_size: Option<String>,

    /// Number of worker threads
    #[arg(long, value_name = "N")]
    pub workers: Option<usize>,

    /// Disable colored output
    #[arg(long)]
    pub no_colors: bool,

    /// Do not write to the console; file reporters still run
    #[arg(short = 's', long)]
    pub silent: bool,

    /// Print the formats basta can analyze and exit
    #[arg(long)]
    pub list: bool,

    /// Print the frameworks basta can recognise, and what gives each away,
    /// and exit
    #[arg(long)]
    pub list_frameworks: bool,

    /// Print the resolved configuration as JSON and exit
    #[arg(long)]
    pub debug: bool,
}

/// A problem with the invocation. Warnings let the run continue; errors do not.
#[derive(Debug, PartialEq)]
pub enum Diagnostic {
    Warning(String),
    Error(String),
}

impl Diagnostic {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    pub fn print(&self) {
        match self {
            Self::Warning(message) => eprintln!("Warning: {message}"),
            Self::Error(message) => eprintln!("Error: {message}"),
        }
    }
}

/// Turn parsed arguments into a configuration, collecting everything wrong
/// with them rather than stopping at the first problem.
pub fn resolve(cli: &Cli) -> (BastaConfig, OutputOptions, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let defaults = BastaConfig::default();

    // Every setting below is the flag when there is one and the config
    // file's section otherwise: a project writes down what it always wants,
    // and a command line overrides it for one run.
    let section = resolve_section(cli, &mut diagnostics);

    let categories = resolve_categories(
        &flag_or(&cli.categories, &section.categories),
        &mut diagnostics,
    );

    let min_confidence = match cli.min_confidence.or(section.min_confidence) {
        // clap already rejects anything outside a u8, which leaves 101-255 as
        // the only reachable mistake: a threshold nothing can satisfy.
        Some(value) if value > 100 => {
            diagnostics.push(Diagnostic::Warning(format!(
                "--min-confidence: {value} is above 100, which would hide every finding; using 100"
            )));
            100
        }
        Some(value) => value,
        None => defaults.min_confidence,
    };

    let threshold = cli.threshold.or(section.threshold);
    if let Some(threshold) = threshold
        && !(0.0..=100.0).contains(&threshold)
    {
        diagnostics.push(Diagnostic::Warning(format!(
            "--threshold: {threshold} is outside 0-100"
        )));
    }

    let max_size = match &cli.max_size {
        Some(raw) => match parse_size(raw) {
            Some(bytes) => Some(bytes),
            None => {
                diagnostics.push(Diagnostic::Warning(format!(
                    "--max-size: '{raw}' is not a size like 500kb or 2mb; ignoring it"
                )));
                None
            }
        },
        None => None,
    };

    let formats_exts = cli
        .formats_exts
        .as_deref()
        .map(parse_format_mappings)
        .unwrap_or_default();

    // A misspelled format would otherwise match no file and report a clean
    // scan, which is the most misleading thing a dead-code tool can do.
    let known = crate::lang::supported_formats();
    for format in &cli.formats {
        if !known.contains(&format.as_str()) && !formats_exts.contains_key(format) {
            diagnostics.push(Diagnostic::Error(format!(
                "--format: '{format}' is not a format basta analyzes (run --list to see them)"
            )));
        }
    }

    let reporters = if cli.reporters.is_empty() {
        vec!["console".to_string()]
    } else {
        cli.reporters.clone()
    };
    for name in &reporters {
        let normalized = crate::run::normalize_reporter_name(name);
        if !cpd_reporter::deadcode::dead_code_reporter_names().contains(&normalized) {
            diagnostics.push(Diagnostic::Warning(format!(
                "unknown reporter '{name}'; it will be skipped"
            )));
        }
    }

    let frameworks = resolve_frameworks(cli, &section, &mut diagnostics);

    // Read now rather than in the run: a file that cannot be read is an
    // error the user can act on, and stdin is only readable once.
    let rust_diagnostics = match cli
        .rust_diagnostics
        .clone()
        .or_else(|| section.rust_diagnostics.clone())
    {
        None => None,
        Some(path) if path.as_os_str() == "-" => {
            let mut text = String::new();
            match std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
                Ok(_) => Some(RustDiagnostics { text, base: None }),
                Err(error) => {
                    diagnostics.push(Diagnostic::Error(format!(
                        "--rust-diagnostics: could not read stdin: {error}"
                    )));
                    None
                }
            }
        }
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(text) => Some(RustDiagnostics {
                text,
                base: Some(
                    path.parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
                ),
            }),
            Err(error) => {
                diagnostics.push(Diagnostic::Error(format!(
                    "--rust-diagnostics: {}: {error}",
                    path.display()
                )));
                None
            }
        },
    };

    for path in &cli.paths {
        if !path.exists() {
            diagnostics.push(Diagnostic::Error(format!(
                "path does not exist: {}",
                path.display()
            )));
        }
    }

    let config = BastaConfig {
        paths: cli.paths.clone(),
        categories,
        min_confidence,
        entry: flag_or(&cli.entry, &section.entry),
        frameworks,
        ignore: flag_or(&cli.ignore, &section.ignore),
        include_tests: cli.include_tests || section.include_tests.unwrap_or(false),
        include_entry_exports: cli.include_entry_exports
            || section.include_entry_exports.unwrap_or(false),
        min_lines: cli
            .min_lines
            .or(section.min_lines)
            .unwrap_or(defaults.min_lines),
        no_gitignore: cli.no_gitignore,
        follow_symlinks: cli.follow_symlinks,
        max_size,
        workers: cli.workers,
        formats: cli.formats.clone(),
        formats_exts,
        rust_diagnostics,
    };
    let output = OutputOptions {
        reporters,
        output_dir: cli.output.clone(),
        no_colors: cli.no_colors || std::env::var_os("NO_COLOR").is_some(),
        silent: cli.silent,
        threshold,
        exit_code: cli.exit_code,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    (config, output, diagnostics)
}

/// The frameworks this run can recognise: the built-in table, then the
/// project's own definitions on top of it.
///
/// A definitions file that cannot be read is an error, not a warning. Going
/// on without it would report as dead exactly the files it was written to
/// keep alive.
fn resolve_frameworks(cli: &Cli, section: &Section, diagnostics: &mut Vec<Diagnostic>) -> Registry {
    let forced = flag_or(&cli.framework, &section.framework);
    let disabled = cli.no_frameworks || section.no_frameworks.unwrap_or(false);
    if disabled && !forced.is_empty() {
        diagnostics.push(Diagnostic::Warning(
            "--no-frameworks turns --framework off as well".to_string(),
        ));
    }
    let (registry, problems) = Registry::assemble(Sources {
        file: cli
            .frameworks_config
            .clone()
            .or_else(|| section.frameworks_config.clone()),
        inline: section.frameworks.as_deref().unwrap_or_default(),
        forced: &forced,
        disabled,
    });
    diagnostics.extend(
        problems
            .into_iter()
            .map(|problem| Diagnostic::Error(format!("frameworks: {problem}"))),
    );
    registry
}

/// A repeatable flag, or what the config file says when the flag was not
/// given: the command line replaces a list, it does not add to it.
fn flag_or(flag: &[String], section: &Option<Vec<String>>) -> Vec<String> {
    match flag.is_empty() {
        true => section.clone().unwrap_or_default(),
        false => flag.to_vec(),
    }
}

/// The dead-code section of the project's jscpd config: the file `--config`
/// names, or the one jscpd itself would find in the working directory.
///
/// A named file that cannot be read stops the run, the way it does in jscpd;
/// one that was only found is warned about and left out.
fn resolve_section(cli: &Cli, diagnostics: &mut Vec<Diagnostic>) -> Section {
    let found = match &cli.config {
        Some(path) => Section::load(path).map_err(Diagnostic::Error),
        None => std::env::current_dir()
            .map_err(|error| error.to_string())
            .and_then(|directory| Section::discover(&directory))
            .map(|found| found.map(|(_, section)| section))
            .map_err(|error| Diagnostic::Warning(format!("{error}; ignoring it"))),
    };
    match found {
        Ok(section) => section.unwrap_or_default(),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            Section::default()
        }
    }
}

/// One line per known framework — its name, then every signal that detects
/// it — for `--list-frameworks`.
pub fn describe_frameworks(registry: &Registry) -> String {
    let width = registry
        .definitions()
        .map(|framework| framework.name.len())
        .max()
        .unwrap_or(0);
    registry
        .definitions()
        .map(|framework| {
            let detect = &framework.detect;
            let signals: Vec<String> = detect
                .config_files
                .iter()
                .cloned()
                .chain(
                    detect
                        .dependencies
                        .iter()
                        .map(|d| format!("dependency {d}")),
                )
                .chain(
                    detect
                        .package_json_keys
                        .iter()
                        .map(|key| format!("package.json \"{key}\"")),
                )
                .collect();
            format!("{:width$}  {}", framework.name, signals.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve_categories(raw: &[String], diagnostics: &mut Vec<Diagnostic>) -> Vec<Category> {
    if raw.is_empty() {
        return Category::DEFAULT.to_vec();
    }
    let mut categories = Vec::new();
    for entry in raw {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry.eq_ignore_ascii_case("all") {
            categories.extend_from_slice(Category::ALL);
            continue;
        }
        match entry.parse::<Category>() {
            Ok(category) => categories.push(category),
            Err(message) => diagnostics.push(Diagnostic::Error(format!("--categories: {message}"))),
        }
    }
    categories.sort();
    categories.dedup();
    if categories.is_empty() {
        return Category::DEFAULT.to_vec();
    }
    categories
}

/// `500kb`, `2mb`, `1024` → bytes. Mirrors jscpd's `--max-size`.
pub fn parse_size(input: &str) -> Option<u64> {
    let text = input.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }
    let (digits, multiplier) = if let Some(rest) = text.strip_suffix("gb") {
        (rest, 1024 * 1024 * 1024)
    } else if let Some(rest) = text.strip_suffix("mb") {
        (rest, 1024 * 1024)
    } else if let Some(rest) = text.strip_suffix("kb") {
        (rest, 1024)
    } else if let Some(rest) = text.strip_suffix('g') {
        (rest, 1024 * 1024 * 1024)
    } else if let Some(rest) = text.strip_suffix('m') {
        (rest, 1024 * 1024)
    } else if let Some(rest) = text.strip_suffix('k') {
        (rest, 1024)
    } else if let Some(rest) = text.strip_suffix('b') {
        (rest, 1)
    } else {
        (text.as_str(), 1)
    };
    let value: f64 = digits.trim().parse().ok()?;
    (value >= 0.0).then(|| (value * multiplier as f64).round() as u64)
}

/// `"typescript:mts,cts;python:pyw"` → a format-to-extensions map.
pub fn parse_format_mappings(input: &str) -> HashMap<String, Vec<String>> {
    let mut mappings = HashMap::new();
    for group in input.split(';') {
        let Some((format, extensions)) = group.trim().split_once(':') else {
            continue;
        };
        let format = format.trim().to_string();
        let extensions: Vec<String> = extensions
            .split(',')
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty())
            .collect();
        if !format.is_empty() && !extensions.is_empty() {
            mappings.insert(format, extensions);
        }
    }
    mappings
}

/// The resolved configuration as JSON, for `--debug` and for a bug report
/// that needs to say what the tool actually decided to do.
pub fn describe(config: &BastaConfig, output: &OutputOptions) -> String {
    let value = serde_json::json!({
        "paths": config.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "categories": config.active_categories().iter().map(|c| c.as_str()).collect::<Vec<_>>(),
        "minConfidence": config.min_confidence,
        "minLines": config.min_lines,
        "entry": config.entry,
        "frameworks": {
            "detect": config.frameworks.is_enabled(),
            "forced": config.frameworks.forced(),
            "known": config.frameworks.definitions().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        },
        "ignore": config.ignore,
        "includeTests": config.include_tests,
        "includeEntryExports": config.include_entry_exports,
        "formats": if config.formats.is_empty() {
            crate::lang::supported_formats().iter().map(|s| s.to_string()).collect::<Vec<_>>()
        } else {
            config.formats.clone()
        },
        "formatsExts": config.formats_exts,
        "rustDiagnostics": config.rust_diagnostics.is_some(),
        "noGitignore": config.no_gitignore,
        "followSymlinks": config.follow_symlinks,
        "maxSize": config.max_size,
        "workers": config.workers,
        "reporters": output.reporters,
        "outputDir": output.output_dir.display().to_string(),
        "threshold": output.threshold,
        "exitCode": output.exit_code,
        "noColors": output.no_colors,
        "silent": output.silent,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_from(std::iter::once("basta").chain(args.iter().copied()))
    }

    fn resolved(args: &[&str]) -> (BastaConfig, OutputOptions, Vec<Diagnostic>) {
        resolve(&parse(args))
    }

    #[test]
    fn defaults_match_the_library_defaults() {
        let (config, output, diagnostics) = resolved(&[]);
        assert!(diagnostics.is_empty());
        assert_eq!(config.categories, Category::DEFAULT.to_vec());
        assert_eq!(config.min_confidence, 60);
        assert!(!config.include_tests);
        assert_eq!(output.reporters, vec!["console"]);
        assert_eq!(output.output_dir, PathBuf::from("report"));
    }

    #[test]
    fn categories_accept_a_list_and_the_word_all() {
        let (config, _, diagnostics) = resolved(&["--categories", "unused-export,imports"]);
        assert!(diagnostics.is_empty());
        assert_eq!(
            config.categories,
            vec![Category::UnusedExport, Category::UnusedImport]
        );

        let (config, _, _) = resolved(&["--categories", "all"]);
        assert_eq!(config.categories, Category::ALL.to_vec());
    }

    #[test]
    fn an_unknown_category_is_an_error_not_a_silent_skip() {
        let (_, _, diagnostics) = resolved(&["--categories", "unused-everything"]);
        assert!(
            diagnostics.iter().any(Diagnostic::is_error),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn duplicate_categories_collapse() {
        let (config, _, _) = resolved(&["--categories", "exports,unused-exports,export"]);
        assert_eq!(config.categories, vec![Category::UnusedExport]);
    }

    #[test]
    fn an_unknown_format_is_an_error_because_it_would_scan_nothing() {
        let (_, _, diagnostics) = resolved(&["--format", "ruby"]);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.is_error() && format!("{d:?}").contains("ruby")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn a_format_introduced_by_a_mapping_is_accepted() {
        let (_, _, diagnostics) = resolved(&[
            "--format",
            "typescript",
            "--formats-exts",
            "typescript:mts,cts",
        ]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn an_unknown_reporter_warns_but_does_not_stop_the_run() {
        let (_, _, diagnostics) = resolved(&["-r", "nonsense"]);
        assert_eq!(diagnostics.len(), 1);
        assert!(!diagnostics[0].is_error());
    }

    #[test]
    fn reporter_aliases_are_not_reported_as_unknown() {
        for alias in ["full", "consoleFull", "gitlab"] {
            let (_, _, diagnostics) = resolved(&["-r", alias]);
            assert!(diagnostics.is_empty(), "{alias}: {diagnostics:?}");
        }
    }

    #[test]
    fn a_missing_path_is_an_error() {
        let (_, _, diagnostics) = resolved(&["/definitely/not/here"]);
        assert!(diagnostics.iter().any(Diagnostic::is_error));
    }

    #[test]
    fn an_impossible_confidence_threshold_is_clamped_with_a_warning() {
        let (config, _, diagnostics) = resolved(&["--min-confidence", "200"]);
        assert_eq!(config.min_confidence, 100);
        assert_eq!(diagnostics.len(), 1);
        assert!(!diagnostics[0].is_error());
    }

    #[test]
    fn sizes_parse_the_way_jscpd_parses_them() {
        assert_eq!(parse_size("1kb"), Some(1024));
        assert_eq!(parse_size("2MB"), Some(2 * 1024 * 1024));
        assert_eq!(parse_size(" 100 "), Some(100));
        assert_eq!(parse_size("1.5k"), Some(1536));
        assert_eq!(parse_size("huge"), None);
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("-5kb"), None);
    }

    #[test]
    fn a_bad_size_warns_and_is_ignored_rather_than_failing() {
        let (config, _, diagnostics) = resolved(&["--max-size", "enormous"]);
        assert!(config.max_size.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert!(!diagnostics[0].is_error());
    }

    #[test]
    fn format_mappings_parse_into_a_map() {
        let mappings = parse_format_mappings("typescript:mts,cts;python: pyw ");
        assert_eq!(mappings["typescript"], vec!["mts", "cts"]);
        assert_eq!(mappings["python"], vec!["pyw"]);
        assert!(parse_format_mappings("garbage").is_empty());
    }

    #[test]
    fn repeatable_flags_accumulate() {
        let cli = parse(&[
            "--entry",
            "src/handlers/**",
            "--entry",
            "scripts/*.ts",
            "--ignore",
            "**/generated/**",
        ]);
        assert_eq!(cli.entry.len(), 2);
        assert_eq!(cli.ignore.len(), 1);
    }

    #[test]
    fn no_color_in_the_environment_is_honoured() {
        // The flag alone must be enough; the environment is an additional
        // source, checked in `resolve`.
        let (_, output, _) = resolved(&["--no-colors"]);
        assert!(output.no_colors);
    }

    #[test]
    fn every_flag_parses_together() {
        let cli = parse(&[
            "src",
            "lib",
            "--categories",
            "all",
            "--min-confidence",
            "75",
            "--min-lines",
            "3",
            "--entry",
            "bin/*.ts",
            "--ignore",
            "**/dist/**",
            "--include-tests",
            "--include-entry-exports",
            "--format",
            "typescript",
            "-r",
            "console,json",
            "-o",
            "out",
            "--threshold",
            "5",
            "--exit-code",
            "2",
            "--no-gitignore",
            "--follow-symlinks",
            "--max-size",
            "1mb",
            "--workers",
            "4",
            "--no-colors",
            "--silent",
        ]);
        assert_eq!(cli.paths.len(), 2);
        assert_eq!(cli.reporters, vec!["console", "json"]);
        assert_eq!(cli.workers, Some(4));
        assert!(cli.include_tests && cli.include_entry_exports && cli.silent);
    }

    #[test]
    fn debug_output_is_json_that_names_what_the_run_will_do() {
        let (config, output, _) = resolved(&["--categories", "all", "--min-confidence", "80"]);
        let json: serde_json::Value = serde_json::from_str(&describe(&config, &output)).unwrap();
        assert_eq!(json["minConfidence"], 80);
        assert_eq!(
            json["categories"].as_array().unwrap().len(),
            Category::ALL.len()
        );
        assert_eq!(json["reporters"][0], "console");
        assert!(
            json["formats"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f == "python"),
            "an unrestricted run names the formats it will actually analyze"
        );
    }

    #[test]
    fn the_command_definition_is_internally_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn a_project_definitions_file_extends_the_built_in_frameworks() {
        let tree = crate::test_scan::TempTree::new("cli-frameworks");
        tree.write(
            "house.json",
            r#"{ "frameworks": [ { "name": "house", "detect": { "dependencies": ["@acme/house"] }, "directories": ["screens"] } ] }"#,
        )
        .write("broken.yaml", "frameworks: [{ name: x, entrys: [] }]\n");
        let house = tree.path().join("house.json");
        let broken = tree.path().join("broken.yaml");

        let (config, _, diagnostics) = resolved(&[
            "--frameworks-config",
            house.to_str().unwrap(),
            "--framework",
            "house",
        ]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(config.frameworks.knows("house") && config.frameworks.knows("next"));
        assert_eq!(config.frameworks.forced(), ["house".to_string()]);
        assert!(describe_frameworks(&config.frameworks).contains("dependency @acme/house"));

        // A file that does not load stops the run: without it the scan would
        // report what the file exists to keep alive.
        let (_, _, diagnostics) = resolved(&["--frameworks-config", broken.to_str().unwrap()]);
        assert!(
            matches!(&diagnostics[..], [Diagnostic::Error(message)] if message.contains("broken.yaml")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn framework_flags_are_checked_against_what_is_known() {
        let (_, _, diagnostics) = resolved(&["--framework", "nextjs"]);
        assert!(
            matches!(&diagnostics[..], [Diagnostic::Error(message)] if message.contains("'nextjs'")),
            "{diagnostics:?}"
        );

        let (config, _, diagnostics) = resolved(&["--no-frameworks"]);
        assert!(diagnostics.is_empty());
        assert!(!config.frameworks.is_enabled());

        let (config, _, diagnostics) = resolved(&["--no-frameworks", "--framework", "next"]);
        assert!(matches!(&diagnostics[..], [Diagnostic::Warning(_)]));
        assert!(!config.frameworks.is_enabled());
    }

    #[test]
    fn the_framework_list_names_every_signal() {
        let listing = describe_frameworks(&Registry::default());
        let jest = listing
            .lines()
            .find(|line| line.starts_with("jest "))
            .expect("jest is built in");
        assert!(jest.contains("jest.config."), "{jest}");
        assert!(jest.contains("dependency jest"), "{jest}");
        assert!(jest.contains("package.json \"jest\""), "{jest}");
    }

    #[test]
    fn the_config_files_section_supplies_what_the_flags_leave_out() {
        let tree = crate::test_scan::TempTree::new("cli-section");
        tree.write(
            "jscpd.json",
            r#"{
                "threshold": 1,
                "basta": {
                    "minConfidence": 90,
                    "minLines": 2,
                    "categories": ["unused-file"],
                    "entry": ["tools/*.js"],
                    "ignore": ["**/generated/**"],
                    "includeTests": true,
                    "threshold": 40,
                    "framework": ["house"],
                    "frameworks": [
                        { "name": "house", "detect": { "packageJsonKeys": ["house"] }, "directories": ["screens"] }
                    ]
                }
            }"#,
        );
        let file = tree.path().join("jscpd.json");
        let file = file.to_str().unwrap();

        let (config, output, diagnostics) = resolved(&["--config", file]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(config.min_confidence, 90);
        assert_eq!(config.min_lines, 2);
        assert_eq!(config.categories, vec![Category::UnusedFile]);
        assert_eq!(config.entry, ["tools/*.js"]);
        assert_eq!(config.ignore, ["**/generated/**"]);
        assert!(config.include_tests);
        assert_eq!(
            output.threshold,
            Some(40.0),
            "the section's threshold, not the clone one above it"
        );
        assert!(config.frameworks.knows("house"));
        assert_eq!(config.frameworks.forced(), ["house".to_string()]);

        // A flag replaces what the section says; it does not add to it.
        let (config, output, _) = resolved(&[
            "-c",
            file,
            "--min-confidence",
            "60",
            "--entry",
            "bin/*.js",
            "--categories",
            "exports",
            "--threshold",
            "5",
        ]);
        assert_eq!(config.min_confidence, 60);
        assert_eq!(config.entry, ["bin/*.js"]);
        assert_eq!(config.categories, vec![Category::UnusedExport]);
        assert_eq!(output.threshold, Some(5.0));
    }

    #[test]
    fn a_named_config_that_cannot_be_used_stops_the_run() {
        let tree = crate::test_scan::TempTree::new("cli-section-broken");
        tree.write("typo.json", r#"{ "deadCode": { "minConfidense": 80 } }"#)
            .write(
                "inline.json",
                r#"{ "deadCode": { "frameworks": [{ "name": "x", "directories": ["../up"] }] } }"#,
            )
            .write("switch.json", r#"{ "deadCode": true }"#);
        for (name, complaint) in [
            ("typo.json", "minConfidense"),
            ("inline.json", "must stay inside"),
            ("missing.json", "missing.json"),
        ] {
            let file = tree.path().join(name);
            let (_, _, diagnostics) = resolved(&["--config", file.to_str().unwrap()]);
            assert!(
                matches!(&diagnostics[..], [Diagnostic::Error(message)] if message.contains(complaint)),
                "{name}: {diagnostics:?}"
            );
        }
        // The boolean form is jscpd's mode switch: nothing for basta to read.
        let file = tree.path().join("switch.json");
        let (config, _, diagnostics) = resolved(&["--config", file.to_str().unwrap()]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(config.min_confidence, 60);
    }

    #[test]
    fn rust_diagnostics_are_read_up_front_from_a_file_or_the_section() {
        let tree = crate::test_scan::TempTree::new("cli-rust-diagnostics");
        tree.write("check.json", "{\"reason\":\"compiler-message\"}\n")
            .write(
                "jscpd.json",
                r#"{ "deadCode": { "rustDiagnostics": "check.json" } }"#,
            );
        let check = tree.path().join("check.json");
        let (config, _, diagnostics) = resolved(&["--rust-diagnostics", check.to_str().unwrap()]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let read = config.rust_diagnostics.expect("read at resolve time");
        assert!(read.text.contains("compiler-message"));
        assert_eq!(
            read.base.as_deref(),
            Some(tree.path()),
            "relative paths inside resolve against the file"
        );

        let missing = tree.path().join("nope.json");
        let (config, _, diagnostics) = resolved(&["--rust-diagnostics", missing.to_str().unwrap()]);
        assert!(config.rust_diagnostics.is_none());
        assert!(
            matches!(&diagnostics[..], [Diagnostic::Error(m)] if m.contains("nope.json")),
            "{diagnostics:?}"
        );

        let (config, _, _) = resolved(&[]);
        assert!(
            config.rust_diagnostics.is_none(),
            "nothing asked for, nothing read"
        );
    }
}
