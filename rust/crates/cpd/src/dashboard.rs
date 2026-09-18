//! The `--dashboard` mode: one screen with the whole picture of a project —
//! its size, duplication, complexity and dead code — from one clone run and,
//! for JavaScript, TypeScript and Python, one dead-code run.

use crate::cli::Cli;
use crate::options::Options;
use crate::{
    Exit, ReportOutcome, canonical_roots, dead_code, display_paths, display_source_path,
    exit_status,
};
use basta::config::BastaConfig;
use cpd_core::deadcode::Report as DeadCodeReport;
use cpd_core::summary::{SummaryMetric, compute_summary};
use cpd_finder::orchestrate::{RunConfig, run as detect};
use cpd_reporter::dashboard::{Dashboard, print_dashboard};
use cpd_reporter::shared::Style;
use std::path::PathBuf;

/// Rows per ranked list unless `--summary-top` says otherwise: enough to act
/// on, few enough to fit a screen.
const TOP: usize = 5;

/// Print the dashboard, then apply the same exit gates as a clone run.
pub fn run(cli: &Cli, opts: &Options, paths: &[PathBuf], config: &RunConfig) -> Result<(), Exit> {
    if !cli.reporters.is_empty() {
        eprintln!("Warning: --dashboard prints to the console; --reporters is ignored");
    }
    // The baseline family needs a clone report to compare and rewrite, which
    // the dashboard does not produce; saying so beats a silently passing gate.
    if opts.baseline.is_some() || opts.baseline_from_ref.is_some() || opts.update_baseline {
        eprintln!(
            "Warning: --dashboard ignores --baseline, --baseline-from-ref and --update-baseline"
        );
    }
    if opts.fail_on_new_clones.is_some() {
        return Err(crate::fatal(
            "--fail-on-new-clones needs a baseline, which --dashboard does not build",
        ));
    }
    // A bad --dead-code-categories or --min-confidence is a refusal in
    // --dead-code, and must stay one here: it is the same option.
    let basta_config = dead_code::config(cli, opts, paths).map_err(Exit)?;

    let timer = std::time::Instant::now();
    let (clone_workers, dead_code_workers) = split_workers(opts.workers);
    let (result, dead_code) = scan(config, basta_config, clone_workers, dead_code_workers);
    let mut clones = result.clones;
    let roots = canonical_roots(paths);
    display_paths(&mut clones, opts.absolute, &roots);
    let summary = compute_summary(
        &result.sources,
        &clones,
        usize::MAX,
        SummaryMetric::Complexity,
        |id| display_source_path(id, opts.absolute, &roots),
    );

    print_dashboard(
        &Dashboard {
            statistics: &result.statistics,
            clones: &clones,
            summary: &summary,
            dead_code: dead_code.as_ref(),
            top: cli.summary_top.unwrap_or(TOP),
            elapsed: timer.elapsed(),
        },
        &Style::new(opts.no_colors),
    );

    // --threshold has no reporter here, so the same comparison is made
    // directly; --exit-code and --fail-on-empty come from exit_status.
    let threshold_exceeded = opts.threshold.is_some_and(|threshold| {
        let actual = result.statistics.total.percentage;
        if actual > threshold {
            eprintln!(
                "ERROR: jscpd found too many duplicates ({:.1}%) over threshold ({:.1}%)",
                actual, threshold
            );
        }
        actual > threshold
    });
    exit_status(
        opts,
        &result.statistics,
        clones.is_empty(),
        ReportOutcome {
            threshold_exceeded,
            failed: false,
        },
    )
}

/// Run the two scans, dead code beside clone detection when there are threads
/// for both: dead-code analysis is mostly single-threaded, so the pair takes
/// about as long as the slower one.
fn scan(
    config: &RunConfig,
    basta_config: BastaConfig,
    clone_workers: usize,
    dead_code_workers: Option<usize>,
) -> (cpd_finder::orchestrate::RunResult, Option<DeadCodeReport>) {
    let clone_config = RunConfig {
        workers: Some(clone_workers),
        ..config.clone()
    };
    let analyze = |workers| {
        let config = BastaConfig {
            workers,
            ..basta_config.clone()
        };
        // Nothing to say about dead code in a tree with no file any analyzer
        // understands; the other sections still stand.
        Some(basta::analyze::run(&config).report).filter(|report| report.statistics.files > 0)
    };
    match dead_code_workers {
        None => {
            let dead_code = analyze(Some(clone_workers));
            let Ok(result) = detect(&clone_config);
            (result, dead_code)
        }
        Some(workers) => std::thread::scope(|scope| {
            let dead_code = scope.spawn(|| analyze(Some(workers)));
            let Ok(result) = detect(&clone_config);
            (result, dead_code.join().unwrap_or_default())
        }),
    }
}

/// Split `--workers` between the two scans so the dashboard never schedules
/// more threads — or more 64 MiB tokenizer stacks — than a single scan would.
/// `None` for the second means the scans run one after the other, which is
/// what a single-worker run asks for.
fn split_workers(workers: Option<usize>) -> (usize, Option<usize>) {
    let total = workers.unwrap_or_else(|| {
        std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
    });
    match total {
        0 | 1 => (1, None),
        total => {
            let dead_code = total / 2;
            (total - dead_code, Some(dead_code))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_workers;

    #[test]
    fn workers_are_a_budget_for_the_whole_run() {
        assert_eq!(split_workers(Some(8)), (4, Some(4)));
        assert_eq!(split_workers(Some(3)), (2, Some(1)));
        assert_eq!(split_workers(Some(2)), (1, Some(1)));
        assert_eq!(
            split_workers(Some(1)),
            (1, None),
            "one worker means one scan at a time"
        );
    }
}
