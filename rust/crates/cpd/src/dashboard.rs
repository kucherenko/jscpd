//! The `--dashboard` mode: one screen with the whole picture of a project —
//! its size, duplication, complexity and dead code — from one clone run and,
//! for JavaScript, TypeScript and Python, one dead-code run.

use crate::cli::Cli;
use crate::options::Options;
use crate::{canonical_roots, dead_code, display_paths, display_source_path};
use cpd_core::summary::{SummaryMetric, compute_summary};
use cpd_finder::orchestrate::{RunConfig, run as detect};
use cpd_reporter::dashboard::{Dashboard, print_dashboard};
use cpd_reporter::shared::Style;
use std::path::PathBuf;

/// Rows per ranked list unless `--summary-top` says otherwise: enough to act
/// on, few enough to fit a screen.
const TOP: usize = 5;

/// Print the dashboard and return the process exit code.
pub fn run(cli: &Cli, opts: &Options, paths: &[PathBuf], config: &RunConfig) -> i32 {
    if !cli.reporters.is_empty() {
        eprintln!("Warning: --dashboard prints to the console; --reporters is ignored");
    }
    let timer = std::time::Instant::now();
    // Dead code runs beside clone detection: it is mostly one thread, so the
    // two passes together take about as long as the slower one.
    let (result, dead_code) = std::thread::scope(|scope| {
        let dead_code = scope.spawn(|| {
            // A refused dead-code configuration was already explained on
            // stderr; the rest of the dashboard is still worth showing.
            dead_code::config(cli, opts, paths)
                .ok()
                .map(|basta| basta::analyze::run(&basta).report)
                .filter(|report| report.statistics.files > 0)
        });
        let Ok(result) = detect(config);
        (result, dead_code.join().unwrap_or_default())
    });
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
    0
}
