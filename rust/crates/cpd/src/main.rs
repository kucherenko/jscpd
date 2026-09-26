mod baseline_ref;
mod cli;
mod complexity;
mod dashboard;
mod dead_code;
mod history;
mod mcp;
mod options;
mod semantic;

use cli::{Cli, ConfigSource, load_config, print_diagnostics};
use cpd_core::models::{CpdClone, KindFilter, Statistics};
use cpd_finder::blame::BlameMap;
use cpd_finder::orchestrate::{RunConfig, SemanticConfig, run};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{ReporterError, ReporterOptions, create_reporter};
use options::Options;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

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
    max_gap_lines: usize,
    similarity: f32,
    semantic: Option<semantic::SemanticOptions>,
    kind: Vec<String>,
    mode: String,
    formats: Vec<String>,
    ignore: Vec<String>,
    ignore_patterns: Vec<String>,
    reporters: Vec<String>,
    output_dir: String,
    exit_code: Option<i32>,
    threshold: Option<f64>,
    baseline: Option<String>,
    update_baseline: bool,
    fail_on_new_clones: Option<u64>,
    fail_on_empty: bool,
    baseline_from_ref: Option<String>,
    blame: bool,
    no_gitignore: bool,
    follow_symlinks: bool,
    max_size: Option<u64>,
    workers: Option<usize>,
    no_colors: bool,
    absolute: bool,
    ignore_case: bool,
    ignore_identifiers: bool,
    ignore_literals: bool,
    ignore_annotations: bool,
    formats_exts: std::collections::HashMap<String, Vec<String>>,
    formats_names: std::collections::HashMap<String, Vec<String>>,
    cross_formats: Vec<Vec<String>>,
    skip_local: bool,
    skip_isolated: Vec<Vec<String>>,
    no_tips: bool,
    silent: bool,
    pattern: Option<String>,
    summary: bool,
    summary_top: usize,
    summary_by: String,
    history: Option<String>,
    history_since: Option<String>,
    history_every: usize,
    history_limit: usize,
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
            max_gap_lines: opts.max_gap_lines,
            similarity: opts.similarity,
            semantic: opts.semantic.clone(),
            kind: opts.kind.clone(),
            mode: format!("{:?}", opts.mode).to_lowercase(),
            formats: opts.formats.clone(),
            ignore: opts.ignore.clone(),
            ignore_patterns: opts.ignore_patterns.clone(),
            reporters: opts.reporters.clone(),
            output_dir: opts.output_dir.to_string_lossy().to_string(),
            exit_code: opts.exit_code,
            threshold: opts.threshold,
            baseline: opts
                .baseline
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            update_baseline: opts.update_baseline,
            fail_on_new_clones: opts.fail_on_new_clones,
            fail_on_empty: opts.fail_on_empty,
            baseline_from_ref: opts.baseline_from_ref.clone(),
            blame: opts.blame,
            no_gitignore: opts.no_gitignore,
            follow_symlinks: opts.follow_symlinks,
            max_size: opts.max_size,
            workers: opts.workers,
            no_colors: opts.no_colors,
            absolute: opts.absolute,
            ignore_case: opts.ignore_case,
            ignore_identifiers: opts.ignore_identifiers,
            ignore_literals: opts.ignore_literals,
            ignore_annotations: opts.ignore_annotations,
            formats_exts: opts.formats_exts.clone(),
            formats_names: opts.formats_names.clone(),
            cross_formats: opts.cross_formats.clone(),
            skip_local: opts.skip_local,
            skip_isolated: opts.skip_isolated.clone(),
            no_tips: opts.no_tips,
            silent: opts.silent,
            pattern: opts.pattern.clone(),
            summary: opts.summary,
            summary_top: opts.summary_top,
            summary_by: opts.summary_by.to_string(),
            history: opts.history.clone(),
            history_since: opts.history_since.clone(),
            history_every: opts.history_every,
            history_limit: opts.history_limit,
        }
    }
}

/// Where a run stops early: the exit code the process finishes with. Only
/// `main` exits the process, so every phase below can be read, and tested,
/// as a function that either succeeds or says how the run ends.
struct Exit(i32);

/// Print `Error: {message}` and end the run with exit code 1.
fn fatal(message: impl std::fmt::Display) -> Exit {
    eprintln!("Error: {message}");
    Exit(1)
}

fn main() {
    let cli = Cli::parse_invoked();
    if let Err(Exit(code)) = run_cli(&cli) {
        std::process::exit(code);
    }
}

fn run_cli(cli: &Cli) -> Result<(), Exit> {
    if cli.list {
        let mut formats = cpd_tokenizer::formats::list_formats();
        formats.sort();
        for f in formats {
            println!("{}", f);
        }
        return Err(Exit(0));
    }
    if cli.store.is_some() {
        eprintln!(
            "Warning: External stores not supported, use jscpd v4.x instead. --store flag ignored."
        );
    }
    if cli.min_duplicated_lines.is_some() {
        eprintln!(
            "Warning: --min-duplicated-lines has never had an effect and will be removed. \
             Use --threshold to fail on too much duplication, or --min-lines to set the smallest clone."
        );
    }

    let opts = load_options(cli)?;
    if cli.debug {
        match serde_json::to_string_pretty(&MergedConfig::from_options(&opts)) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Error serializing config: {}", e),
        }
        return Err(Exit(0));
    }
    validate_baseline_flags(&opts)?;
    let paths = scan_paths(&opts)?;
    let run_config = run_config(&opts, &paths);

    if let Some(settings) = &opts.semantic_download {
        semantic::download(settings, opts.silent)
            .map_err(|e| fatal(format!("--semantic-download: {e}")))?;
        if opts.semantic.is_none() {
            return Err(Exit(0));
        }
    }
    warn_semantic_ignored(cli, &opts);

    // --dead-code: a different question about the same tree. It walks with the
    // same filters and reports through the same reporter names, so everything
    // a user knows about `jscpd` carries over — but a clone report and a
    // dead-code report share no data, so the two modes do not share a run.
    if opts.dead_code {
        if cli.complexity || cli.dashboard || cli.health {
            return Err(fatal(
                "--dead-code cannot be combined with --complexity, --dashboard or --health",
            ));
        }
        return Err(Exit(dead_code::run(cli, &opts, &paths)));
    }
    if cli.complexity {
        return Err(Exit(complexity::run(&opts, &paths, &run_config)));
    }
    if cli.dashboard {
        return dashboard::run(cli, &opts, &paths, &run_config);
    }
    if cli.health {
        return dashboard::run_health(cli, &opts, &paths, &run_config);
    }
    // --mcp: serve the Model Context Protocol over stdio instead of running a
    // one-shot detection. stdout carries protocol messages only, so this must
    // branch before any reporter output.
    if cli.mcp {
        return Err(Exit(mcp::serve(run_config)));
    }
    detect_and_report(&opts, &paths, &run_config)
}

/// The merged options, after reporting the config file used, its
/// diagnostics, and every flag value that is ignored or corrected.
fn load_options(cli: &Cli) -> Result<Options, Exit> {
    let config_result = load_config(cli.config.as_deref());
    if let Some(
        ConfigSource::Explicit(path)
        | ConfigSource::AutoJscpdJson(path)
        | ConfigSource::AutoDotConfig(path)
        | ConfigSource::AutoPackageJson(path),
    ) = &config_result.source
    {
        eprintln!("Using config from {}", path.display());
    }
    print_diagnostics(&config_result.diagnostics);
    // For explicit --config, exit with error code 1 only on fatal diagnostics
    // (IO errors and parse errors). Unknown fields and invalid values are warnings.
    if matches!(config_result.source, Some(ConfigSource::Explicit(_)))
        && config_result.diagnostics.iter().any(|d| d.is_fatal())
    {
        return Err(Exit(1));
    }

    if let Some(mode) = cli.mode.as_deref()
        && !matches!(mode, "mild" | "weak" | "strict")
    {
        eprintln!(
            "Warning: invalid mode '{}': must be one of: mild, weak, strict (defaulting to mild)",
            mode
        );
    }
    if let Some(metric) = cli.summary_by.as_deref()
        && let Err(e) = metric.parse::<cpd_core::summary::SummaryMetric>()
    {
        eprintln!("Warning: --summary-by: {} (defaulting to tokens)", e);
    }

    let mut opts = Options::from_cli_and_config(cli, &config_result.config);
    // --similarity is a ratio in (0, 1]; anything else is a typo, not a request.
    if !(opts.similarity > 0.0 && opts.similarity <= 1.0) {
        eprintln!(
            "Warning: --similarity: {} is outside (0, 1]; using 1 (exact matches only)",
            opts.similarity
        );
        opts.similarity = 1.0;
    }
    if let Some(semantic) = &mut opts.semantic
        && !(semantic.threshold > 0.0 && semantic.threshold <= 1.0)
    {
        eprintln!(
            "Warning: --semantic-threshold: {} is outside (0, 1]; using {}",
            semantic.threshold,
            semantic::DEFAULT_THRESHOLD
        );
        semantic.threshold = semantic::DEFAULT_THRESHOLD;
    }
    if opts.semantic.is_none() && opts.semantic_flags {
        eprintln!(
            "Warning: --semantic-threshold, --semantic-model, --semantic-url, --semantic-provider and --semantic-scope have no effect without --semantic"
        );
    }
    // The tokenizer skips an --ignore-pattern that fails to compile, so a
    // typo would otherwise disable the pattern with no feedback.
    for pattern in &opts.ignore_patterns {
        if let Err(e) = regex::Regex::new(pattern) {
            eprintln!(
                "Warning: --ignore-pattern: invalid regex '{}' is skipped: {}",
                pattern,
                e.to_string().lines().last().unwrap_or_default()
            );
        }
    }
    check_format_names(&opts)?;
    check_kinds(&opts)?;
    Ok(opts)
}

/// An unknown `--kind` is an error: a typo would otherwise filter out every
/// clone and report a clean scan. A kind whose detector is off is a warning —
/// the filter never switches detection on behind the user's back.
fn check_kinds(opts: &Options) -> Result<(), Exit> {
    let kinds = parse_kinds(&opts.kind).map_err(|e| fatal(format!("--kind: {e}")))?;
    let normalizing = opts.ignore_identifiers || opts.ignore_literals || opts.ignore_annotations;
    let gap = opts.max_gap_lines > 0;
    let ast = opts.similarity < 1.0;
    for kind in kinds {
        let missing = match kind {
            KindFilter::Renamed if !normalizing => {
                "--ignore-identifiers, --ignore-literals or --ignore-annotations"
            }
            KindFilter::Gap if !gap => "--max-gap-lines",
            KindFilter::Ast if !ast => "--similarity",
            KindFilter::Similar if !gap && !ast => "--max-gap-lines or --similarity",
            KindFilter::Semantic if opts.semantic.is_none() => "--semantic",
            _ => continue,
        };
        eprintln!(
            "Warning: --kind {}: no such clones are found without {missing}",
            kind.as_str()
        );
    }
    Ok(())
}

fn parse_kinds(raw: &[String]) -> Result<Vec<KindFilter>, String> {
    raw.iter().map(|k| k.parse()).collect()
}

/// Unknown names in --format are an error: a typo like 'cs' would otherwise
/// match 0 files and report a clean scan with exit 0 (#964, #1047). In
/// --cross-formats they are a warning. Custom formats from --formats-exts
/// (and, for --format, --formats-names) are legal in both.
fn check_format_names(opts: &Options) -> Result<(), Exit> {
    let known = cpd_tokenizer::formats::list_formats();
    let unknown: Vec<&str> = opts
        .formats
        .iter()
        .map(String::as_str)
        .filter(|f| {
            !known.contains(f)
                && !opts.formats_exts.contains_key(*f)
                && !opts.formats_names.contains_key(*f)
        })
        .collect();
    if !unknown.is_empty() {
        eprintln!(
            "Error: --format: '{}' is not a supported format (run with --list to see supported formats)",
            unknown.join("', '")
        );
        return Err(Exit(1));
    }
    for format in opts.cross_formats.iter().flatten() {
        if !known.contains(&format.as_str()) && !opts.formats_exts.contains_key(format) {
            eprintln!("Warning: --cross-formats: unknown format '{}'", format);
        }
    }
    Ok(())
}

/// The dependent baseline flags are gates and must not silently no-op without
/// a baseline source. Clap already rejects CLI-level combinations of
/// --baseline-from-ref with --baseline/--update-baseline; this catches
/// config-file/CLI mixes too.
fn validate_baseline_flags(opts: &Options) -> Result<(), Exit> {
    let has_baseline = opts.baseline.is_some();
    let has_ref = opts.baseline_from_ref.is_some();
    let problem = if has_baseline && has_ref {
        Some("use either --baseline or --baseline-from-ref, not both")
    } else if opts.update_baseline && !has_baseline {
        Some("--update-baseline requires --baseline <file>")
    } else if opts.fail_on_new_clones.is_some() && !has_baseline && !has_ref {
        Some("--fail-on-new-clones requires --baseline <file> or --baseline-from-ref <ref>")
    } else {
        None
    };
    problem.map_or(Ok(()), |message| Err(fatal(message)))
}

/// The paths to scan: the current directory when none are given, each one
/// required to exist, canonicalized under --absolute.
fn scan_paths(opts: &Options) -> Result<Vec<PathBuf>, Exit> {
    let paths = if opts.paths.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
    } else {
        opts.paths.clone()
    };
    // A scan path that does not exist is an error, not an empty scan (#1047):
    // the walker would otherwise skip it and report a clean run with exit 0.
    let missing: Vec<&PathBuf> = paths.iter().filter(|p| !p.exists()).collect();
    if !missing.is_empty() {
        for p in &missing {
            eprintln!("Error: path does not exist: {}", p.display());
        }
        return Err(Exit(1));
    }
    Ok(match opts.absolute {
        true => paths
            .into_iter()
            .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
            .collect(),
        false => paths,
    })
}

fn run_config(opts: &Options, paths: &[PathBuf]) -> RunConfig {
    RunConfig {
        paths: paths.to_vec(),
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
        max_lines: opts.max_lines,
        max_gap_lines: opts.max_gap_lines,
        similarity: opts.similarity,
        mode: opts.mode,
        formats: opts.formats.clone(),
        ignore: opts.ignore.clone(),
        code_ignore_patterns: opts.ignore_patterns.clone(),
        max_size: opts.max_size,
        no_gitignore: opts.no_gitignore,
        follow_symlinks: opts.follow_symlinks,
        skip_local: opts.skip_local,
        skip_isolated: opts
            .skip_isolated
            .iter()
            .map(|group| group.iter().map(PathBuf::from).collect())
            .collect(),
        blame: opts.blame,
        workers: opts.workers,
        ignore_case: opts.ignore_case,
        ignore_identifiers: opts.ignore_identifiers,
        ignore_literals: opts.ignore_literals,
        ignore_annotations: opts.ignore_annotations,
        formats_exts: opts.formats_exts.clone(),
        formats_names: opts.formats_names.clone(),
        pattern: opts.pattern.clone(),
        cross_formats: opts.cross_formats.clone(),
        // Validated by check_kinds while the options were loaded.
        kinds: parse_kinds(&opts.kind).unwrap_or_default(),
        // Only the detection run embeds; see `with_semantic`.
        semantic: None,
    }
}

/// `--semantic` only changes the one-shot detection run (and the base-ref
/// scan it is compared with): the other modes read percentages that must not
/// move because an embedding model was asked, and scanning every commit of
/// `--history` through a model would take hours.
fn warn_semantic_ignored(cli: &Cli, opts: &Options) {
    if opts.semantic.is_none() {
        return;
    }
    let mode = if opts.dead_code {
        "--dead-code"
    } else if cli.complexity {
        "--complexity"
    } else if cli.dashboard {
        "--dashboard"
    } else if cli.health {
        "--health"
    } else if cli.mcp {
        "--mcp"
    } else {
        return;
    };
    eprintln!("Warning: --semantic is ignored by {mode}");
}

/// The detection run with the semantic pass switched on when `--semantic`
/// asks for it. Fails before the scan when the embedder cannot work: the
/// local model is missing, say.
fn with_semantic(opts: &Options, run_config: &RunConfig) -> Result<RunConfig, Exit> {
    let semantic = match &opts.semantic {
        Some(options) => Some(SemanticConfig {
            threshold: options.threshold,
            scope: options.scope,
            embedder: semantic::embedder(options, opts.silent)
                .map_err(|e| fatal(format!("--semantic: {e}")))?,
        }),
        None => None,
    };
    Ok(RunConfig {
        semantic,
        ..run_config.clone()
    })
}

fn detect_and_report(
    opts: &Options,
    paths: &[PathBuf],
    run_config: &RunConfig,
) -> Result<(), Exit> {
    let timer = std::time::Instant::now();
    let semantic_config = with_semantic(opts, run_config)?;
    let run_result = run(&semantic_config).map_err(fatal)?;
    let mut clones = run_result.clones;
    let mut statistics = run_result.statistics;

    let canonical_roots = canonical_roots(paths);
    display_paths(&mut clones, opts.absolute, &canonical_roots);
    apply_baseline(opts, &semantic_config, &mut clones, &mut statistics)?;
    let blame_data = blame(opts, paths, &mut clones);
    // Captured after blame, so its time is included.
    let elapsed = timer.elapsed();

    // Opt-in codebase summary. Computed after detection from data already in
    // memory; when --summary is off this is a no-op and detection output is
    // byte-identical to previous releases.
    let summary = opts.summary.then(|| {
        cpd_core::summary::compute_summary(
            &run_result.sources,
            &clones,
            opts.summary_top,
            opts.summary_by,
            |id| display_source_path(id, opts.absolute, &canonical_roots),
        )
    });
    // Past commits are scanned without the semantic pass; the working tree's
    // point must match them.
    let history_statistics = match opts.semantic {
        Some(_) if opts.history.is_some() || opts.history_since.is_some() => {
            let plain: Vec<CpdClone> = clones
                .iter()
                .filter(|c| !c.kind.is_semantic())
                .cloned()
                .collect();
            cpd_finder::statistics::compute(&run_result.sources, &plain)
        }
        _ => statistics.clone(),
    };
    let history = history(opts, run_config, &history_statistics)?;

    let reporter_opts = ReporterOptions {
        output_dir: opts.output_dir.clone(),
        threshold: opts.threshold,
        blame: opts.blame,
        no_colors: opts.no_colors,
        blame_data,
        absolute: opts.absolute,
        // Bundled at build time; matches what `cpd --version` prints (#915).
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        sarif_error_tokens: opts.sarif_error_tokens,
    };
    let plan = plan_reporters(opts);
    let ctx = ReportContext::new(&statistics, elapsed)
        .with_summary(summary.as_ref())
        .with_history(history.as_ref());
    let outcome = run_reporters(&plan, &reporter_opts, &clones, &ctx, &opts.output_dir);
    if !plan.silent {
        print_time_and_tips(opts, elapsed);
    }
    exit_status(opts, &statistics, clones.is_empty(), outcome)
}

/// Canonical scan roots, a file's being its directory, so fragment paths can
/// be stripped of them (macOS /var → /private/var included).
fn canonical_roots(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| {
            let canonical = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            match canonical.is_file() {
                true => canonical
                    .parent()
                    .map(|d| d.to_path_buf())
                    .unwrap_or(canonical),
                false => canonical,
            }
        })
        .collect()
}

/// Make source ids scan-root-relative for display and SARIF, keeping the scan
/// root on each fragment so reporters can still read the file; or absolute
/// under --absolute.
///
/// Source ids arrive from the finder as the walked path anchored at the
/// canonical scan root: a file reached through a symlink keeps the name it
/// was found by (#1059).
fn display_paths(clones: &mut [CpdClone], absolute: bool, canonical_roots: &[PathBuf]) {
    for clone in clones {
        for fragment in [&mut clone.fragment_a, &mut clone.fragment_b] {
            match absolute {
                true => make_path_absolute(&mut fragment.source_id),
                false => relativize_to_scan_root(fragment, canonical_roots),
            }
        }
    }
}

/// Mark clones absent from the baseline as new and fill the newClones and
/// newDuplicatedLines statistics (#944). Runs after path relativization so
/// fingerprint file reads resolve.
fn apply_baseline(
    opts: &Options,
    run_config: &RunConfig,
    clones: &mut [CpdClone],
    statistics: &mut Statistics,
) -> Result<(), Exit> {
    if let Some(baseline_path) = &opts.baseline {
        let outcome =
            cpd_reporter::baseline::apply(clones, statistics, baseline_path, opts.update_baseline)
                .map_err(fatal)?;
        if let Some(update) = outcome.update {
            eprintln!(
                "Baseline {} updated: {} fingerprints added, {} removed ({} total)",
                baseline_path.display(),
                update.added,
                update.removed,
                update.total
            );
        }
    } else if let Some(base_ref) = &opts.baseline_from_ref {
        // Stateless variant: scan the base ref's tree with the same
        // configuration and compare against that ephemeral baseline.
        let base = baseline_ref::baseline_from_ref(base_ref, run_config).map_err(fatal)?;
        cpd_reporter::baseline::apply_in_memory(clones, statistics, &base);
    }
    Ok(())
}

/// Git blame for every fragment, when --blame asks for it.
fn blame(opts: &Options, paths: &[PathBuf], clones: &mut [CpdClone]) -> BlameMap {
    if !opts.blame {
        return BlameMap::new();
    }
    let repo_root = paths
        .first()
        .and_then(|p| find_git_root(p))
        .unwrap_or_else(|| PathBuf::from("."));
    cpd_finder::blame::enrich(clones, &repo_root)
}

/// The duplication trend over git history (#1002): one scan per selected
/// commit in a temporary worktree, plus the current run as the last point.
/// Nothing here runs without --history / --history-since.
fn history(
    opts: &Options,
    run_config: &RunConfig,
    statistics: &Statistics,
) -> Result<Option<cpd_core::history::History>, Exit> {
    let Some(spec) = history::HistorySpec::from_options(opts) else {
        return Ok(None);
    };
    let mut points = history::collect_history(&spec, run_config).map_err(fatal)?;
    points.push(history::working_tree_point(statistics));
    Ok(Some(cpd_core::history::History {
        range: spec.label(),
        threshold: opts.threshold,
        points,
    }))
}

/// Which reporters run, in which order.
#[derive(Debug, PartialEq)]
struct ReporterPlan {
    /// Print to stdout: ai, console, console-full, silent, xcode.
    console: Vec<String>,
    /// Write files and print "saved to": badge, csv, html, json, markdown,
    /// sarif, xml.
    files: Vec<String>,
    /// The threshold check runs last, and once.
    threshold: bool,
    /// No timing line or tips.
    silent: bool,
}

/// `time` is dropped (timing is automatic); --silent swaps console reporters
/// for `silent`; `threshold` is pulled out to run last, and added whenever
/// --threshold is set.
fn plan_reporters(opts: &Options) -> ReporterPlan {
    let mut names: Vec<String> = opts
        .reporters
        .iter()
        .filter(|r| *r != "time")
        .cloned()
        .collect();
    if opts.silent {
        names.retain(|r| !is_console_reporter(r));
        names.push("silent".to_string());
    }
    names.retain(|r| r != "threshold");
    let threshold = opts.threshold.is_some() || opts.reporters.iter().any(|r| r == "threshold");
    // The threshold reporter prints, so it counts against silence; with no
    // reporter at all, or only `silent`, there is nothing to time.
    let silent = opts.silent || (!threshold && names.iter().all(|r| r == "silent"));
    let (console, files) = names.into_iter().partition(|r| is_console_reporter(r));
    ReporterPlan {
        console,
        files,
        threshold,
        silent,
    }
}

/// What running the reporters found.
struct ReportOutcome {
    threshold_exceeded: bool,
    failed: bool,
}

fn run_reporters(
    plan: &ReporterPlan,
    reporter_opts: &ReporterOptions,
    clones: &[CpdClone],
    ctx: &ReportContext,
    output_dir: &Path,
) -> ReportOutcome {
    let threshold = ["threshold".to_string()];
    let last: &[String] = if plan.threshold { &threshold } else { &[] };
    let mut outcome = ReportOutcome {
        threshold_exceeded: false,
        failed: false,
    };
    for reporter_name in plan.console.iter().chain(&plan.files).chain(last) {
        let Some(reporter) = create_reporter(normalize_reporter_name(reporter_name), reporter_opts)
        else {
            eprintln!("Warning: unknown reporter '{}'", reporter_name);
            continue;
        };
        match reporter.report(clones, ctx, output_dir) {
            Ok(()) => {}
            Err(ReporterError::ThresholdExceeded { actual, threshold }) => {
                eprintln!(
                    "ERROR: jscpd found too many duplicates ({:.1}%) over threshold ({:.1}%)",
                    actual, threshold
                );
                outcome.threshold_exceeded = true;
            }
            Err(e) => {
                eprintln!("Reporter '{}' error: {}", reporter_name, e);
                outcome.failed = true;
            }
        }
    }
    outcome
}

fn print_time_and_tips(opts: &Options, elapsed: std::time::Duration) {
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

    // Tips are interactive hints: skip them when stdout is a pipe or a file
    // (CI logs, agent hooks, `| grep`), like npm and cargo do.
    if opts.no_tips || !std::io::stdout().is_terminal() {
        return;
    }
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

/// How a completed run exits, after its reports were written.
fn exit_status(
    opts: &Options,
    statistics: &Statistics,
    no_clones: bool,
    outcome: ReportOutcome,
) -> Result<(), Exit> {
    // Empty scans (#1047): every path exists, but nothing was analyzed — the
    // --format, --ignore or --pattern filters matched no file, or every file
    // was below --min-tokens. Warn by default so an intentionally empty tree
    // still passes; --fail-on-empty makes it fatal for CI jobs where an empty
    // result means a misconfigured scan. Reports were already written, so a
    // CI job can still inspect them.
    let empty_scan = statistics.total.sources == 0;
    if empty_scan && opts.fail_on_empty {
        eprintln!(
            "ERROR: jscpd analyzed no files (--fail-on-empty): check the paths and the --format, --ignore and --pattern filters"
        );
    } else if empty_scan {
        eprintln!(
            "Warning: jscpd analyzed no files: check the paths and the --format, --ignore and --pattern filters"
        );
    }
    if outcome.failed {
        eprintln!("ERROR: a reporter failed to write its output (see the message above)");
    }
    if outcome.failed || (empty_scan && opts.fail_on_empty) {
        return Err(Exit(1));
    }

    let new_clones_exceeded = match opts.fail_on_new_clones {
        // --update-baseline absorbs the current state; gating a run that just
        // accepted its clones would always fail on the pre-update state.
        Some(max_new) if !opts.update_baseline && statistics.total.new_clones > max_new => {
            eprintln!(
                "ERROR: jscpd found {} new clones not in the baseline (allowed: {})",
                statistics.total.new_clones, max_new
            );
            true
        }
        _ => false,
    };
    if outcome.threshold_exceeded || new_clones_exceeded {
        return Err(Exit(1));
    }
    match opts.exit_code {
        Some(code) if !no_clones => Err(Exit(code)),
        _ => Ok(()),
    }
}

/// Convert a source_id path to absolute if it isn't already.
fn make_path_absolute(source_id: &mut String) {
    let path = std::path::Path::new(source_id);
    if !path.is_absolute()
        && let Ok(abs) = std::fs::canonicalize(path)
    {
        *source_id = abs.to_string_lossy().into_owned();
    }
}

/// Display path for a summary entry: the same relativization applied to clone
/// fragments in `relativize_to_scan_root`, so per-file duplication matching
/// works on identical strings. Separators are normalized to `/` so a report
/// reads the same on Windows as everywhere else.
fn display_source_path(id: &str, absolute: bool, canonical_roots: &[std::path::PathBuf]) -> String {
    if absolute {
        return id.replace('\\', "/");
    }
    let path = std::path::Path::new(id);
    for root in canonical_roots {
        if let Ok(stripped) = path.strip_prefix(root) {
            return strip_dot_prefix(&stripped.to_string_lossy().replace('\\', "/"));
        }
    }
    strip_dot_prefix(&id.replace('\\', "/"))
}

/// Strip a leading `./` or `.\` component so paths are not dot-prefixed.
fn strip_dot_prefix(s: &str) -> String {
    s.strip_prefix("./")
        .or_else(|| s.strip_prefix(".\\"))
        .unwrap_or(s)
        .to_string()
}

/// Relativize a canonicalized `source_id` to its scan root and store the root
/// on the fragment so reporters can reconstruct the absolute path for file I/O.
///
/// The first matching canonical scan root is used. If no root matches, the
/// source_id is left unchanged (absolute) and source_root is set to None.
fn relativize_to_scan_root(
    fragment: &mut cpd_core::models::Fragment,
    canonical_roots: &[std::path::PathBuf],
) {
    let path = std::path::Path::new(&fragment.source_id);
    for root in canonical_roots {
        if let Ok(stripped) = path.strip_prefix(root) {
            fragment.source_root = Some(root.to_string_lossy().into_owned());
            fragment.source_id = strip_dot_prefix(&stripped.to_string_lossy());
            return;
        }
    }
    fragment.source_id = strip_dot_prefix(&fragment.source_id);
}

/// Walk up from path to find the nearest `.git` directory.
///
/// Canonicalizes `start` first: walking up a relative path terminates at the
/// empty path (e.g. parent of `pkg` is `""`), which both mis-reports a repo
/// rooted at the CWD as `""` and breaks the callers that canonicalize the
/// returned root.
pub(crate) fn find_git_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        current = current.parent()?.to_path_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(args: &[&str]) -> ReporterPlan {
        use clap::Parser;
        let argv: Vec<&str> = ["cpd"]
            .into_iter()
            .chain(args.iter().copied())
            .chain(["."])
            .collect();
        let cli = Cli::parse_from(argv);
        plan_reporters(&Options::from_cli_and_config(
            &cli,
            &cli::ConfigFile::default(),
        ))
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reporters_run_console_first_then_files_then_the_threshold() {
        let plan = plan(&["--reporters", "json,full,html,threshold,time"]);
        assert_eq!(plan.console, names(&["full"]));
        assert_eq!(plan.files, names(&["json", "html"]));
        assert!(plan.threshold, "listed as a reporter");
        assert!(!plan.silent);
    }

    #[test]
    fn silent_replaces_every_console_reporter() {
        let plan = plan(&["--silent", "--reporters", "console,ai,json"]);
        assert_eq!(plan.console, names(&["silent"]));
        assert_eq!(plan.files, names(&["json"]));
        assert!(plan.silent);
    }

    #[test]
    fn a_threshold_check_is_output_so_the_run_is_not_silent() {
        // `--reporters silent` alone prints nothing, not even the timing line;
        // adding --threshold adds a reporter that prints.
        assert!(plan(&["--reporters", "silent"]).silent);
        let with_threshold = plan(&["--reporters", "silent", "--threshold", "5"]);
        assert!(with_threshold.threshold);
        assert!(!with_threshold.silent);
        // --silent itself stays silent either way.
        assert!(plan(&["--silent", "--threshold", "5"]).silent);
    }

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

    #[test]
    fn strip_dot_prefix_unix() {
        assert_eq!(strip_dot_prefix("./src/foo.rs"), "src/foo.rs");
        assert_eq!(strip_dot_prefix("src/foo.rs"), "src/foo.rs");
        assert_eq!(strip_dot_prefix(".hidden"), ".hidden");
    }

    #[test]
    fn strip_dot_prefix_windows() {
        assert_eq!(strip_dot_prefix(".\\src\\foo.rs"), "src\\foo.rs");
        assert_eq!(strip_dot_prefix(".\\foo.rs"), "foo.rs");
    }

    fn make_fragment(source_id: &str) -> cpd_core::models::Fragment {
        cpd_core::models::Fragment {
            source_id: source_id.to_string(),
            source_root: None,
            start: cpd_core::models::Location {
                line: 1,
                column: 0,
                offset: 0,
            },
            end: cpd_core::models::Location {
                line: 1,
                column: 0,
                offset: 0,
            },
            range: [0, 0],
            blame: None,
        }
    }

    #[test]
    fn relativize_strips_scan_root_prefix_and_sets_root() {
        let mut frag = make_fragment("/project/frontend/src/foo.rs");
        let roots = vec![std::path::PathBuf::from("/project/frontend")];
        relativize_to_scan_root(&mut frag, &roots);
        assert_eq!(frag.source_id, "src/foo.rs");
        assert_eq!(frag.source_root.as_deref(), Some("/project/frontend"));
    }

    #[test]
    fn relativize_keeps_absolute_when_outside_all_roots() {
        let mut frag = make_fragment("/elsewhere/src/foo.rs");
        let roots = vec![std::path::PathBuf::from("/project")];
        relativize_to_scan_root(&mut frag, &roots);
        assert_eq!(frag.source_id, "/elsewhere/src/foo.rs");
        assert!(frag.source_root.is_none());
    }

    #[test]
    fn relativize_uses_first_matching_root() {
        let mut frag = make_fragment("/project/frontend/src/foo.rs");
        let roots = vec![
            std::path::PathBuf::from("/project"),
            std::path::PathBuf::from("/project/frontend"),
        ];
        relativize_to_scan_root(&mut frag, &roots);
        assert_eq!(frag.source_id, "frontend/src/foo.rs");
        assert_eq!(frag.source_root.as_deref(), Some("/project"));
    }

    #[test]
    fn relativize_strips_dot_prefix() {
        let mut frag = make_fragment("./src/foo.rs");
        let roots = vec![std::path::PathBuf::from("/project")];
        relativize_to_scan_root(&mut frag, &roots);
        assert_eq!(frag.source_id, "src/foo.rs");
    }
}
