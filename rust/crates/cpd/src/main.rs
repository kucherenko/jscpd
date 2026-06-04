mod cli;
mod options;
mod timer;

use clap::Parser;
use cli::{Cli, load_config};
use options::Options;
use cpd_finder::orchestrate::{run, RunConfig};
use cpd_reporter::reporter::{create_reporter, ReporterOptions};
use cpd_reporter::context::ReportContext;
use cpd_reporter::time::TimeReporter;
use timer::Timer;

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

    // Handle --store warning (no-op in V1)
    if cli.store.is_some() {
        eprintln!("Warning: External stores not supported in V1. --store flag ignored.");
    }

    // Load config file and build options
    let config = load_config(cli.config.as_deref());
    let opts = Options::from_cli_and_config(&cli, &config);

    // If no paths given, scan current directory
    let paths = if opts.paths.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))]
    } else {
        opts.paths.clone()
    };

    // Build RunConfig
    let run_config = RunConfig {
        paths: paths.clone(),
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
        max_lines: opts.max_lines,
        mode: opts.mode,
        formats: opts.formats.clone(),
        ignore_patterns: opts.ignore_patterns.clone(),
        max_size: opts.max_size,
        no_gitignore: opts.no_gitignore,
        follow_symlinks: opts.follow_symlinks,
        skip_local: opts.skip_local,
        blame: opts.blame,
        workers: opts.workers,
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

    // Capture elapsed time
    let elapsed = timer.elapsed();

    // Git blame enrichment (if requested)
    if opts.blame {
        let repo_root = paths.first()
            .and_then(|p| find_git_root(p))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        cpd_finder::blame::enrich(&mut clones, &repo_root);
    }

    // Reporter options
    let reporter_opts = ReporterOptions {
        output_dir: opts.output_dir.clone(),
        threshold: opts.threshold,
        blame: opts.blame,
        no_colors: opts.no_colors,
    };

    // Run reporters (threshold last)
    let mut all_reporters = opts.reporters.clone();
    all_reporters.retain(|r| r != "threshold");
    if opts.reporters.iter().any(|r| r == "threshold") {
        all_reporters.push("threshold".to_string());
    }

    // Separate time from other reporters
    let has_time = all_reporters.iter().any(|r| r == "time");
    let mut base_reporters: Vec<String> = all_reporters.iter()
        .filter(|r| *r != "time")
        .cloned()
        .collect();

    // If only "time" specified, default to silent reporter
    if base_reporters.is_empty() && has_time {
        base_reporters.push("silent".to_string());
    }

    let mut threshold_exceeded = false;
    for reporter_name in &base_reporters {
        let reporter = match create_reporter(reporter_name, &reporter_opts) {
            Some(r) => r,
            None => {
                eprintln!("Warning: unknown reporter '{}'", reporter_name);
                continue;
            }
        };

        // Wrap with TimeReporter if --reporters time specified
        let reporter: Box<dyn cpd_reporter::reporter::Reporter> = if has_time {
            Box::new(TimeReporter::new(reporter))
        } else {
            reporter
        };

        let ctx = ReportContext::new(&statistics, elapsed);
        match reporter.report(&clones, &ctx, &opts.output_dir) {
            Ok(()) => {}
            Err(cpd_reporter::reporter::ReporterError::ThresholdExceeded { actual, threshold }) => {
                eprintln!("Threshold exceeded: {:.1}% > {:.1}%", actual, threshold);
                threshold_exceeded = true;
            }
            Err(e) => {
                eprintln!("Reporter '{}' error: {}", reporter_name, e);
            }
        }
    }

    // Exit code logic
    if threshold_exceeded {
        std::process::exit(1);
    }
    if opts.exit_code && !clones.is_empty() {
        std::process::exit(1);
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
