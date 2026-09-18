//! The `--dead-code` mode.
//!
//! jscpd answers two questions about a codebase: what is written twice, and
//! what is never run. The second is basta's, and this module is the seam: it
//! translates jscpd's options into basta's and hands the result to the same
//! reporter names, so `jscpd --dead-code -r sarif -o report src` behaves
//! exactly like the duplication run a user already knows.

use crate::cli::Cli;
use crate::options::Options;
use basta::config::BastaConfig;
use basta::run::{OutputOptions, run_and_report};
use cpd_core::deadcode::Category;
use std::path::PathBuf;

/// Run dead-code detection and return the process exit code.
pub fn run(cli: &Cli, opts: &Options, paths: &[PathBuf]) -> i32 {
    let config = match config(cli, opts, paths) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let output = OutputOptions {
        reporters: opts.reporters.clone(),
        output_dir: opts.output_dir.clone(),
        no_colors: opts.no_colors,
        silent: opts.silent,
        threshold: opts.threshold,
        exit_code: opts.exit_code,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    run_and_report(&config, &output).exit_code
}

/// jscpd's options translated into basta's, or the exit code of a refusal.
pub fn config(cli: &Cli, opts: &Options, paths: &[PathBuf]) -> Result<BastaConfig, i32> {
    // Formats jscpd knows but basta cannot analyze are dropped rather than
    // refused: `jscpd --dead-code --format java,typescript` should analyze the
    // TypeScript and say why the Java was skipped.
    let supported = basta::lang::supported_formats();
    let (formats, skipped): (Vec<String>, Vec<String>) = opts
        .formats
        .iter()
        .cloned()
        .partition(|f| supported.contains(&f.as_str()));
    if !skipped.is_empty() {
        eprintln!(
            "Warning: --dead-code does not analyze {}; it supports {}",
            skipped.join(", "),
            supported.join(", ")
        );
    }
    if !opts.formats.is_empty() && formats.is_empty() {
        eprintln!(
            "Error: --format selected no format --dead-code can analyze (supported: {})",
            supported.join(", ")
        );
        return Err(1);
    }

    let mut categories = Vec::new();
    for raw in &opts.dead_code_categories {
        if raw.eq_ignore_ascii_case("all") {
            categories.extend_from_slice(Category::ALL);
            continue;
        }
        match raw.parse::<Category>() {
            Ok(category) => categories.push(category),
            Err(message) => {
                eprintln!("Error: --dead-code-categories: {message}");
                return Err(1);
            }
        }
    }
    categories.sort();
    categories.dedup();

    Ok(BastaConfig {
        paths: paths.to_vec(),
        categories,
        min_confidence: cli
            .min_confidence
            .or(opts.min_confidence)
            .unwrap_or_else(|| BastaConfig::default().min_confidence),
        entry: opts.entry.clone(),
        ignore: opts.ignore.clone(),
        include_tests: opts.include_tests,
        include_entry_exports: opts.include_entry_exports,
        // `--min-lines` means the smallest clone worth reporting; carrying it
        // into dead code would silently hide every one-line constant, which is
        // not what a user who set it for duplication asked for.
        min_lines: 0,
        no_gitignore: opts.no_gitignore,
        follow_symlinks: opts.follow_symlinks,
        max_size: opts.max_size,
        workers: opts.workers,
        formats,
        formats_exts: opts.formats_exts.clone(),
    })
}
