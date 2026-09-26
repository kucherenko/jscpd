//! The `--dashboard` and `--health` modes. Both rest on the same scan — one
//! clone run and, for JavaScript, TypeScript and Python, one dead-code run —
//! and the same health score; the dashboard prints every section under the
//! health badge, `--health` prints the badge alone.

use crate::cli::Cli;
use crate::options::Options;
use crate::{
    Exit, ReportOutcome, canonical_roots, dead_code, display_paths, display_source_path,
    exit_status, fatal, normalize_reporter_name,
};
use basta::config::BastaConfig;
use cpd_core::deadcode::Report as DeadCodeReport;
use cpd_core::health::{Health, HealthConfig};
use cpd_core::summary::{SummaryMetric, compute_summary};
use cpd_finder::orchestrate::{RunConfig, run as detect};
use cpd_reporter::dashboard::{Dashboard, DashboardView, print_dashboard};
use cpd_reporter::health_render;
use cpd_reporter::shared::{Style, write_report_file};
use std::path::PathBuf;

/// Rows per ranked list unless `--summary-top` says otherwise: enough to act
/// on, few enough to fit a screen.
const TOP: usize = 5;

/// Which of the two modes is reporting.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Dashboard,
    Health,
}

/// Print the dashboard, then apply the same exit gates as a clone run.
pub fn run(cli: &Cli, opts: &Options, paths: &[PathBuf], config: &RunConfig) -> Result<(), Exit> {
    report(Mode::Dashboard, cli, opts, paths, config)
}

/// Print the health badge alone, with the same exit gates.
pub fn run_health(
    cli: &Cli,
    opts: &Options,
    paths: &[PathBuf],
    config: &RunConfig,
) -> Result<(), Exit> {
    report(Mode::Health, cli, opts, paths, config)
}

/// The `health` config object with the `--health-input` file laid over it.
fn health_config(opts: &Options) -> Result<HealthConfig, Exit> {
    let mut config = opts.health.clone();
    if let Some(path) = &opts.health_input {
        let text = std::fs::read_to_string(path)
            .map_err(|e| fatal(format!("--health-input: {}: {e}", path.display())))?;
        let input: HealthConfig = serde_json::from_str(&text)
            .map_err(|e| fatal(format!("--health-input: {}: {e}", path.display())))?;
        config = config.merge(input);
    }
    cpd_core::health::validate(&config).map_err(|e| fatal(format!("health: {e}")))?;
    Ok(config)
}

fn report(
    mode: Mode,
    cli: &Cli,
    opts: &Options,
    paths: &[PathBuf],
    config: &RunConfig,
) -> Result<(), Exit> {
    // The baseline family needs a clone report to compare and rewrite, which
    // these modes do not produce; saying so beats a silently passing gate.
    if opts.baseline.is_some() || opts.baseline_from_ref.is_some() || opts.update_baseline {
        eprintln!(
            "Warning: --dashboard and --health ignore --baseline, --baseline-from-ref and --update-baseline"
        );
    }
    if opts.fail_on_new_clones.is_some() {
        return Err(fatal(
            "--fail-on-new-clones needs a baseline, which --dashboard and --health do not build",
        ));
    }
    // The dead-code scan needs whole files to build an accurate import
    // graph, so basta has no line-count skip to hand this to (unlike
    // --max-size, which it does honour) — the dead-code section can end up
    // covering files the duplication and complexity sections skipped.
    if opts.max_lines.is_some() {
        eprintln!(
            "Warning: --max-lines applies to duplication and complexity, not the dead-code section"
        );
    }
    // A bad --dead-code-categories or --min-confidence is a refusal in
    // --dead-code, and must stay one here: it is the same option. A
    // --format that excludes every language dead-code analysis reads is
    // not: `strict: false` leaves that section out instead (`None`) rather
    // than refusing the whole dashboard over one of several sections.
    let basta_config = dead_code::config(cli, opts, paths, false).map_err(Exit)?;
    let health_config = health_config(opts)?;

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
    let health = cpd_core::health::compute(
        &summary,
        &clones,
        dead_code.as_ref().map(|report| &report.statistics),
        &health_config,
    );
    let top = cli.summary_top.unwrap_or(TOP);
    let view = Dashboard {
        health: &health,
        statistics: &result.statistics,
        clones: &clones,
        summary: &summary,
        dead_code: dead_code.as_ref(),
        top,
        elapsed: timer.elapsed(),
    }
    .view();
    let failed = run_reporters(mode, opts, &view, top, timer.elapsed());

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
            failed,
        },
    )
}

/// Run the reporters these modes have; returns true when one failed to write.
/// `console` prints the dashboard or the badge, `ai` the one-line health,
/// `json` writes `jscpd-dashboard.json` or `jscpd-health.json`, `badge`
/// writes `jscpd-health-badge.svg`, and `markdown`/`html` write the same
/// content as `jscpd-dashboard.md`/`.html` or `jscpd-health.md`/`.html`.
fn run_reporters(
    mode: Mode,
    opts: &Options,
    view: &DashboardView,
    top: usize,
    elapsed: std::time::Duration,
) -> bool {
    let style = Style::new(opts.no_colors);
    let health: &Health = &view.health;
    let mut failed = false;
    let mut write = |name: &str, file: &str, label: &str, content: Result<String, String>| {
        let written = content.and_then(|text| {
            write_report_file(&opts.output_dir, file, text, &style, label)
                .map_err(|e| e.to_string())
        });
        if let Err(e) = written {
            eprintln!("Reporter '{name}' error: {e}");
            failed = true;
        }
    };
    for name in &opts.reporters {
        match (normalize_reporter_name(name), mode) {
            ("console" | "console-full", _) if opts.silent => {}
            ("console" | "console-full", Mode::Dashboard) => {
                print_dashboard(view, top, elapsed, &style)
            }
            ("console" | "console-full", Mode::Health) => {
                health_render::print_badge(health, &style)
            }
            ("ai", _) if opts.silent => {}
            ("ai", _) => health_render::print_compact(health),
            ("json", Mode::Dashboard) => write(
                "json",
                "jscpd-dashboard.json",
                "JSON",
                serde_json::to_string_pretty(view).map_err(|e| e.to_string()),
            ),
            ("json", Mode::Health) => write(
                "json",
                "jscpd-health.json",
                "JSON",
                serde_json::to_string_pretty(&serde_json::json!({ "health": health }))
                    .map_err(|e| e.to_string()),
            ),
            ("badge", _) => write(
                "badge",
                "jscpd-health-badge.svg",
                "Badge",
                Ok(cpd_reporter::badge::health_badge(health)),
            ),
            ("markdown", Mode::Dashboard) => write(
                "markdown",
                "jscpd-dashboard.md",
                "Markdown",
                Ok(cpd_reporter::dashboard::render_markdown(view)),
            ),
            ("markdown", Mode::Health) => write(
                "markdown",
                "jscpd-health.md",
                "Markdown",
                Ok(health_render::render_markdown(health)),
            ),
            ("html", Mode::Dashboard) => write(
                "html",
                "jscpd-dashboard.html",
                "HTML",
                Ok(cpd_reporter::dashboard::render_html(view)),
            ),
            ("html", Mode::Health) => write(
                "html",
                "jscpd-health.html",
                "HTML",
                Ok(health_render::render_html(health)),
            ),
            ("silent" | "time", _) => {}
            (other, _) => eprintln!(
                "Warning: reporter '{other}' is not available with --dashboard or --health (use console, ai, json, badge, markdown or html)"
            ),
        }
    }
    failed
}

/// Only the semantic pass can fail a detection run, and the dashboard runs
/// without it.
const SEMANTIC_OFF: &str = "the dashboard scans without --semantic";

/// Run the two scans, dead code beside clone detection when there are threads
/// for both: dead-code analysis is mostly single-threaded, so the pair takes
/// about as long as the slower one. `basta_config` is `None` when no format
/// dead-code analysis reads was selected: the clone scan then gets the
/// whole worker budget rather than a share held back for a scan that will
/// not run.
fn scan(
    config: &RunConfig,
    basta_config: Option<BastaConfig>,
    clone_workers: usize,
    dead_code_workers: Option<usize>,
) -> (cpd_finder::orchestrate::RunResult, Option<DeadCodeReport>) {
    let Some(basta_config) = basta_config else {
        let result = detect(config).expect(SEMANTIC_OFF);
        return (result, None);
    };
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
            let result = detect(&clone_config).expect(SEMANTIC_OFF);
            (result, dead_code)
        }
        Some(workers) => std::thread::scope(|scope| {
            let dead_code = scope.spawn(|| analyze(Some(workers)));
            let result = detect(&clone_config).expect(SEMANTIC_OFF);
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
