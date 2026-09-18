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
    let config = match config(cli, opts, paths, true) {
        Ok(Some(config)) => config,
        // `strict: true` never returns `Ok(None)`: an empty analyzable
        // format set is `Err` there, since the whole point of this mode is
        // a dead-code report.
        Ok(None) => unreachable!("config(.., strict: true) always errors on no format"),
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
    let outcome = run_and_report(&config, &output);
    // basta has no concept of `--fail-on-empty`: it is jscpd's flag, so the
    // gate lives here, the same one `exit_status` applies to a clone run
    // (#1047) — every path existed, but nothing matched the analyzable
    // formats, `--ignore` or `--pattern`.
    let empty_scan = outcome.report.statistics.files == 0;
    if empty_scan && opts.fail_on_empty {
        eprintln!(
            "ERROR: jscpd analyzed no files (--fail-on-empty): check the paths and the --format, --ignore and --pattern filters"
        );
        return 1;
    } else if empty_scan {
        eprintln!(
            "Warning: jscpd analyzed no files: check the paths and the --format, --ignore and --pattern filters"
        );
    }
    outcome.exit_code
}

/// jscpd's options translated into basta's, or the exit code of a refusal.
///
/// `strict` is true for standalone `--dead-code`, where finding no format
/// basta can analyze is the whole reason to fail. `--dashboard`/`--health`
/// pass `false`: dead code is only one of several sections there, so a
/// `--format` that excludes JavaScript, TypeScript and Python should just
/// leave that section out (`Ok(None)`), the same way a project with none of
/// those files already does, not abort the whole report — and the warning
/// about formats basta cannot analyze does not apply either, since nothing
/// there was asked to analyze them. Bad `--dead-code-categories` or
/// `--min-confidence` stay a refusal either way: they are the same option
/// misused, not a mismatch between what was asked for and what dead-code
/// analysis covers.
pub fn config(
    cli: &Cli,
    opts: &Options,
    paths: &[PathBuf],
    strict: bool,
) -> Result<Option<BastaConfig>, i32> {
    // Formats jscpd knows but basta cannot analyze are dropped rather than
    // refused: `jscpd --dead-code --format java,typescript` should analyze the
    // TypeScript and say why the Java was skipped.
    let supported = basta::lang::supported_formats();
    let (formats, skipped): (Vec<String>, Vec<String>) = opts
        .formats
        .iter()
        .cloned()
        .partition(|f| supported.contains(&f.as_str()));
    if strict && !skipped.is_empty() {
        eprintln!(
            "Warning: --dead-code does not analyze {}; it supports {}",
            skipped.join(", "),
            supported.join(", ")
        );
    }
    if !opts.formats.is_empty() && formats.is_empty() {
        if !strict {
            return Ok(None);
        }
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

    // clap already rejects anything outside a u8, which leaves 101-255 as
    // the only reachable mistake: a threshold nothing can satisfy. The
    // standalone `basta` binary already clamps this with a warning
    // (`basta::cli`); jscpd builds `BastaConfig` directly, bypassing that,
    // so the same clamp belongs here too.
    let min_confidence = match cli.min_confidence.or(opts.min_confidence) {
        Some(value) if value > 100 => {
            eprintln!(
                "Warning: --min-confidence: {value} is above 100, which would hide every finding; using 100"
            );
            100
        }
        Some(value) => value,
        None => BastaConfig::default().min_confidence,
    };

    Ok(Some(BastaConfig {
        paths: paths.to_vec(),
        categories,
        min_confidence,
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
    }))
}
