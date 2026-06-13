mod cli;
mod options;
mod timer;

use clap::Parser;
use cli::{Cli, ConfigSource, load_config, print_diagnostics};
use cpd_finder::orchestrate::{RunConfig, run};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{ReporterOptions, create_reporter};
use options::Options;
use timer::Timer;

fn normalize_reporter_name(name: &str) -> &str {
    match name {
        "full" | "consoleFull" => "console-full",
        other => other,
    }
}

fn is_console_reporter(name: &str) -> bool {
    matches!(
        normalize_reporter_name(name),
        "ai" | "console" | "console-full" | "silent" | "xcode"
    )
}

#[derive(serde::Serialize)]
struct MergedConfig {
    paths: Vec<String>,
    min_tokens: usize,
    min_lines: usize,
    max_lines: Option<usize>,
    mode: String,
    formats: Vec<String>,
    ignore: Vec<String>,
    ignore_patterns: Vec<String>,
    reporters: Vec<String>,
    output_dir: String,
    exit_code: Option<i32>,
    threshold: Option<f64>,
    blame: bool,
    no_gitignore: bool,
    follow_symlinks: bool,
    max_size: Option<u64>,
    workers: Option<usize>,
    no_colors: bool,
    absolute: bool,
    ignore_case: bool,
    formats_exts: std::collections::HashMap<String, Vec<String>>,
    formats_names: std::collections::HashMap<String, Vec<String>>,
    skip_local: bool,
    no_tips: bool,
    silent: bool,
    pattern: Option<String>,
}

impl MergedConfig {
    fn from_options(opts: &Options) -> Self {
        Self {
            paths: opts
                .paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            min_tokens: opts.min_tokens,
            min_lines: opts.min_lines,
            max_lines: opts.max_lines,
            mode: format!("{:?}", opts.mode).to_lowercase(),
            formats: opts.formats.clone(),
            ignore: opts.ignore.clone(),
            ignore_patterns: opts.ignore_patterns.clone(),
            reporters: opts.reporters.clone(),
            output_dir: opts.output_dir.to_string_lossy().to_string(),
            exit_code: opts.exit_code,
            threshold: opts.threshold,
            blame: opts.blame,
            no_gitignore: opts.no_gitignore,
            follow_symlinks: opts.follow_symlinks,
            max_size: opts.max_size,
            workers: opts.workers,
            no_colors: opts.no_colors,
            absolute: opts.absolute,
            ignore_case: opts.ignore_case,
            formats_exts: opts.formats_exts.clone(),
            formats_names: opts.formats_names.clone(),
            skip_local: opts.skip_local,
            no_tips: opts.no_tips,
            silent: opts.silent,
            pattern: opts.pattern.clone(),
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // Handle --list flag: print all supported formats and exit 0
    if cli.list {
        let mut formats = cpd_tokenizer::formats::list_formats();
        formats.sort();
        for f in formats {
            println!("{}", f);
        }
        std::process::exit(0);
    }

    // Handle --store warning
    if cli.store.is_some() {
        eprintln!(
            "Warning: External stores not supported, use jscpd v4.x instead. --store flag ignored."
        );
    }

    // Load config file and build options
    let config_result = load_config(cli.config.as_deref());

    // Report which config source was used
    if let Some(ref source) = config_result.source {
        match source {
            ConfigSource::Explicit(p) => {
                eprintln!("Using config from {}", p.display());
            }
            ConfigSource::AutoJscpdJson => {
                eprintln!("Using config from .jscpd.json");
            }
            ConfigSource::AutoPackageJson => {
                eprintln!("Using config from package.json");
            }
        }
    }

    // Print any diagnostics
    print_diagnostics(&config_result.diagnostics);

    // For explicit --config, exit with error code 1 only on fatal diagnostics
    // (IO errors and parse errors). Unknown fields and invalid values are warnings.
    if matches!(config_result.source, Some(ConfigSource::Explicit(_)))
        && config_result.diagnostics.iter().any(|d| d.is_fatal())
    {
        std::process::exit(1);
    }

    // CLI mode validation: warn on invalid --mode value
    if cli.mode.is_some() {
        let mode_str = cli.mode.as_deref().unwrap();
        match mode_str {
            "mild" | "weak" | "strict" => {}
            _ => {
                eprintln!(
                    "Warning: invalid mode '{}': must be one of: mild, weak, strict (defaulting to mild)",
                    mode_str
                );
            }
        }
    }

    let config = config_result.config;
    let opts = Options::from_cli_and_config(&cli, &config);

    // Handle --debug: print merged config as JSON and exit
    if cli.debug {
        let merged = MergedConfig::from_options(&opts);
        match serde_json::to_string_pretty(&merged) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Error serializing config: {}", e),
        }
        std::process::exit(0);
    }

    // If no paths given, scan current directory
    let paths = if opts.paths.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))]
    } else {
        opts.paths.clone()
    };

    // If --absolute, canonicalize all paths
    let paths: Vec<std::path::PathBuf> = if opts.absolute {
        paths
            .into_iter()
            .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
            .collect()
    } else {
        paths
    };

    // Build RunConfig
    let run_config = RunConfig {
        paths: paths.clone(),
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
        max_lines: opts.max_lines,
        mode: opts.mode,
        formats: opts.formats.clone(),
        ignore: opts.ignore.clone(),
        code_ignore_patterns: opts.ignore_patterns.clone(),
        max_size: opts.max_size,
        no_gitignore: opts.no_gitignore,
        follow_symlinks: opts.follow_symlinks,
        skip_local: opts.skip_local,
        blame: opts.blame,
        workers: opts.workers,
        ignore_case: opts.ignore_case,
        formats_exts: opts.formats_exts.clone(),
        formats_names: opts.formats_names.clone(),
        pattern: opts.pattern.clone(),
    };

    // Start timing before detection
    let timer = Timer::start();

    // Run detection
    let run_result = match run(&run_config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let mut clones = run_result.clones;
    let statistics = run_result.statistics;

    // If --absolute, convert all source_ids to absolute paths
    if opts.absolute {
        for clone in &mut clones {
            make_path_absolute(&mut clone.fragment_a.source_id);
            make_path_absolute(&mut clone.fragment_b.source_id);
        }
    }

    // Git blame enrichment (if requested)
    let blame_data = if opts.blame {
        let repo_root = paths
            .first()
            .and_then(|p| find_git_root(p))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        cpd_finder::blame::enrich(&mut clones, &repo_root)
    } else {
        std::collections::HashMap::new()
    };

    // Capture elapsed time (after blame so it's included)
    let elapsed = timer.elapsed();

    // Reporter options
    let reporter_opts = ReporterOptions {
        output_dir: opts.output_dir.clone(),
        threshold: opts.threshold,
        blame: opts.blame,
        no_colors: opts.no_colors,
        blame_data,
        absolute: opts.absolute,
    };

    // --silent: remove console reporters, add silent, suppress time/tips
    // Run reporters (threshold last, "time" reporter removed — timing is automatic)
    let mut all_reporters: Vec<String> = opts
        .reporters
        .iter()
        .filter(|r| *r != "time")
        .cloned()
        .collect();
    if opts.silent {
        all_reporters.retain(|r| !is_console_reporter(r));
        all_reporters.push("silent".to_string());
    }
    all_reporters.retain(|r| r != "threshold");
    // Auto-include threshold reporter when --threshold is set
    if opts.threshold.is_some() || opts.reporters.iter().any(|r| r == "threshold") {
        all_reporters.push("threshold".to_string());
    }

    let is_silent =
        opts.silent || all_reporters.is_empty() || all_reporters.iter().all(|r| r == "silent");

    // Threshold reporter runs last and only once. Extract it before partitioning.
    let has_threshold = all_reporters.iter().any(|r| r == "threshold");
    all_reporters.retain(|r| r != "threshold");

    // Console-type reporters print to stdout (ai, console, console-full, silent, xcode)
    // File-type reporters write files and print "saved to" messages (badge, csv, html, json, markdown, sarif, xml)
    let (console_names, file_names): (Vec<String>, Vec<String>) = all_reporters
        .iter()
        .cloned()
        .partition(|r| is_console_reporter(r));

    let mut threshold_exceeded = false;

    let run_batch = |names: &[String]| -> bool {
        let mut threshold_exceeded = false;
        for reporter_name in names {
            let reporter =
                match create_reporter(normalize_reporter_name(reporter_name), &reporter_opts) {
                    Some(r) => r,
                    None => {
                        eprintln!("Warning: unknown reporter '{}'", reporter_name);
                        continue;
                    }
                };

            let ctx = ReportContext::new(&statistics, elapsed);
            match reporter.report(&clones, &ctx, &opts.output_dir) {
                Ok(()) => {}
                Err(cpd_reporter::reporter::ReporterError::ThresholdExceeded {
                    actual,
                    threshold,
                }) => {
                    eprintln!(
                        "ERROR: jscpd found too many duplicates ({:.1}%) over threshold ({:.1}%)",
                        actual, threshold
                    );
                    threshold_exceeded = true;
                }
                Err(e) => {
                    eprintln!("Reporter '{}' error: {}", reporter_name, e);
                }
            }
        }
        threshold_exceeded
    };

    threshold_exceeded |= run_batch(&console_names);
    threshold_exceeded |= run_batch(&file_names);
    if has_threshold {
        threshold_exceeded |= run_batch(&["threshold".to_string()]);
    }

    // Print execution time if not silent
    if !is_silent {
        let duration_ms = elapsed.as_secs_f64() * 1000.0;
        let (prefix, suffix) = if opts.no_colors {
            ("", "")
        } else {
            ("\x1b[90m", "\x1b[39m")
        };
        if duration_ms < 1000.0 {
            println!("{}time: {:.3}ms{}", prefix, duration_ms, suffix);
        } else {
            println!("{}time: {:.2}s{}", prefix, elapsed.as_secs_f64(), suffix);
        }

        if !opts.no_tips {
            let (bold, bold_off) = if opts.no_colors {
                ("", "")
            } else {
                ("\x1b[1m", "\x1b[22m")
            };
            println!();
            println!(
                "{}\u{1f4a1} Auto-refactor with AI: {}{}npx skills add https://github.com/kucherenko/jscpd --skill dry-refactoring{}{}",
                prefix, bold, suffix, prefix, bold_off
            );
            println!(
                "{}\u{1f3a9} New: Gangsta Agents \u{2014} discipline your AI coding \u{2192} gangsta.page{}",
                prefix, suffix
            );
            println!(
                "{}\u{1f496} Support jscpd project \u{2192} https://opencollective.com/jscpd{}",
                prefix, suffix
            );
        }
    }

    // Exit code logic
    if threshold_exceeded {
        std::process::exit(1);
    }
    if let Some(code) = opts.exit_code {
        if !clones.is_empty() {
            std::process::exit(code);
        }
    }
}

/// Convert a source_id path to absolute if it isn't already.
fn make_path_absolute(source_id: &mut String) {
    let path = std::path::Path::new(source_id);
    if !path.is_absolute() {
        if let Ok(abs) = std::fs::canonicalize(path) {
            *source_id = abs.to_string_lossy().into_owned();
        }
    }
}

/// Walk up from path to find the nearest `.git` directory.
fn find_git_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_reporter_name_full() {
        assert_eq!(normalize_reporter_name("full"), "console-full");
    }

    #[test]
    fn normalize_reporter_name_consolefull() {
        assert_eq!(normalize_reporter_name("consoleFull"), "console-full");
    }

    #[test]
    fn normalize_reporter_name_console_full() {
        assert_eq!(normalize_reporter_name("console-full"), "console-full");
    }

    #[test]
    fn normalize_reporter_name_console() {
        assert_eq!(normalize_reporter_name("console"), "console");
    }

    #[test]
    fn normalize_reporter_name_json() {
        assert_eq!(normalize_reporter_name("json"), "json");
    }

    #[test]
    fn is_console_reporter_aliases() {
        assert!(is_console_reporter("full"));
        assert!(is_console_reporter("consoleFull"));
        assert!(is_console_reporter("console-full"));
        assert!(is_console_reporter("console"));
        assert!(is_console_reporter("ai"));
        assert!(is_console_reporter("xcode"));
        assert!(is_console_reporter("silent"));
        assert!(!is_console_reporter("json"));
        assert!(!is_console_reporter("html"));
    }
}
