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
use basta::framework::{Registry, Sources};
use basta::run::{OutputOptions, run_and_report};
use cpd_core::deadcode::Category;
use std::path::{Path, PathBuf};

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
        // Dead code has a budget of its own when the section names one: the
        // top-level `threshold` is a share of duplicated lines, and the two
        // percentages rarely want the same number.
        threshold: cli
            .threshold
            .or(opts.dead_code_section.threshold)
            .or(opts.threshold),
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
    // A bad --dead-code-categories or --min-confidence is a refusal (or, for
    // confidence, a clamp-with-warning) whether or not a dead-code section
    // ends up running at all: they are the same option misused, not a
    // mismatch between what was asked for and what dead-code analysis
    // covers, and must be validated before the format check below can
    // return early.
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

    // jscpd has no flags of its own for frameworks; the config file's
    // dead-code section is where a project says them, and a
    // `basta.frameworks.*` beside it is picked up as the `basta` binary
    // would. Assembled by basta, so both front ends mean the same thing.
    let section = &opts.dead_code_section;
    let (frameworks, problems) = Registry::assemble(Sources {
        file: section.frameworks_config.clone(),
        inline: section.frameworks.as_deref().unwrap_or_default(),
        forced: section.framework.as_deref().unwrap_or_default(),
        disabled: section.no_frameworks.unwrap_or(false),
    });
    if !problems.is_empty() {
        for problem in problems {
            eprintln!("Error: dead-code frameworks: {problem}");
        }
        return Err(1);
    }

    Ok(Some(BastaConfig {
        paths: paths.to_vec(),
        categories,
        min_confidence,
        entry: opts.entry.clone(),
        frameworks,
        // The section's `ignore` narrows a dead-code run only; the clone run
        // beside it in a dashboard still sees those files.
        ignore: opts
            .ignore
            .iter()
            .chain(section.ignore.iter().flatten())
            .cloned()
            .collect(),
        include_tests: opts.include_tests,
        include_entry_exports: opts.include_entry_exports,
        // `--min-lines` means the smallest clone worth reporting; carrying it
        // into dead code would silently hide every one-line constant, which is
        // not what a user who set it for duplication asked for. The section's
        // own `minLines` has no such ambiguity.
        min_lines: section.min_lines.unwrap_or(0),
        no_gitignore: opts.no_gitignore,
        follow_symlinks: opts.follow_symlinks,
        max_size: opts.max_size,
        workers: opts.workers,
        formats,
        formats_exts: opts.formats_exts.clone(),
        // A file only: jscpd's stdin is not basta's to read.
        rust_diagnostics: match &section.rust_diagnostics {
            None => None,
            Some(path) => match std::fs::read_to_string(path) {
                Ok(text) => Some(basta::config::RustDiagnostics {
                    text,
                    base: path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
                }),
                Err(error) => {
                    eprintln!(
                        "Error: deadCode.rustDiagnostics: {}: {error}",
                        path.display()
                    );
                    return Err(1);
                }
            },
        },
    }))
}
