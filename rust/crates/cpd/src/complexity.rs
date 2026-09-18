//! The `--complexity` mode.
//!
//! The `--summary` tables without the clone run: files are walked and
//! tokenized with the same filters, complexity is counted from those tokens,
//! and detection — the part that grows with the size of the codebase — never
//! starts.

use crate::options::Options;
use crate::{canonical_roots, display_source_path, normalize_reporter_name, print_time_and_tips};
use cpd_core::summary::{Summary, SummaryMetric, compute_summary};
use cpd_finder::orchestrate::{RunConfig, build_thread_pool, prepare_scan_in};
use cpd_reporter::shared::{Style, write_report_file};
use cpd_reporter::summary_render::{print_complexity, print_complexity_compact};
use std::path::PathBuf;

/// The summary of a complexity-only scan: ranked by complexity unless
/// `--summary-by` asks for another metric.
pub fn scan(opts: &Options, paths: &[PathBuf], config: &RunConfig) -> Summary {
    let pool = build_thread_pool(config.workers);
    // Function signatures feed clone detection only, and this mode never
    // detects: scanning with --similarity must not pay for the syntax trees.
    let scan_config = RunConfig {
        similarity: 1.0,
        ..config.clone()
    };
    let sources = prepare_scan_in(&pool, &scan_config).sources;
    let by = match opts.summary_by_set {
        true => opts.summary_by,
        false => SummaryMetric::Complexity,
    };
    let roots = canonical_roots(paths);
    compute_summary(&sources, &[], opts.summary_top, by, |id| {
        display_source_path(id, opts.absolute, &roots)
    })
}

/// Run a complexity-only scan and return the process exit code.
pub fn run(opts: &Options, paths: &[PathBuf], config: &RunConfig) -> i32 {
    // This mode never detects clones, so every option that measures or
    // gates on them has nothing to act on. Warn rather than silently doing
    // nothing with a flag the user asked for.
    if opts.threshold.is_some() || opts.exit_code.is_some() {
        eprintln!(
            "Warning: --complexity ignores --threshold and --exit-code: there is no duplication percentage or clone count to gate on"
        );
    }
    if opts.baseline.is_some()
        || opts.baseline_from_ref.is_some()
        || opts.update_baseline
        || opts.fail_on_new_clones.is_some()
    {
        eprintln!(
            "Warning: --complexity ignores the baseline family (--baseline, --baseline-from-ref, --update-baseline, --fail-on-new-clones): it never detects clones to compare"
        );
    }
    if opts.history.is_some() {
        eprintln!(
            "Warning: --complexity ignores --history: there is no duplication trend without clone detection"
        );
    }
    if !opts.kind.is_empty() {
        eprintln!(
            "Warning: --complexity ignores --kind: it filters clone kinds, and this mode detects none"
        );
    }

    let timer = std::time::Instant::now();
    let summary = scan(opts, paths, config);
    let elapsed = timer.elapsed();

    // Every path existed, but nothing matched --format, --ignore or
    // --pattern, or every file was below --min-tokens (#1047's gate,
    // applied the same way the clone-detection and dashboard paths do).
    let empty_scan = summary.total_files == 0;
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

    let style = Style::new(opts.no_colors);
    let mut printed = false;
    let mut code = 0;
    for name in &opts.reporters {
        match normalize_reporter_name(name) {
            "console" | "console-full" if !opts.silent => {
                print_complexity(&summary, &style);
                printed = true;
            }
            "ai" if !opts.silent => {
                print_complexity_compact(&summary);
                printed = true;
            }
            "json" => {
                let json = serde_json::json!({ "summary": summary });
                let written = serde_json::to_string_pretty(&json)
                    .map_err(|e| e.to_string())
                    .and_then(|text| {
                        write_report_file(
                            &opts.output_dir,
                            "jscpd-complexity.json",
                            text,
                            &style,
                            "JSON",
                        )
                        .map_err(|e| e.to_string())
                    });
                if let Err(e) = written {
                    eprintln!("Reporter 'json' error: {e}");
                    code = 1;
                }
            }
            "console" | "console-full" | "ai" | "silent" | "time" => {}
            other => {
                eprintln!(
                    "Warning: reporter '{other}' is not available with --complexity (use console, ai or json)"
                );
            }
        }
    }
    if printed {
        print_time_and_tips(opts, elapsed);
    }
    code
}
