mod baseline_ref;
mod cli;
mod history;
mod mcp;
mod options;
mod timer;

use cli::{Cli, ConfigSource, load_config, print_diagnostics};
use cpd_finder::orchestrate::{RunConfig, run};
use cpd_reporter::context::ReportContext;
use cpd_reporter::reporter::{ReporterOptions, create_reporter};
use options::Options;
use std::io::IsTerminal;
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
    max_gap_lines: usize,
    similarity: f32,
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

fn main() {
    let cli = Cli::parse_invoked();

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
            ConfigSource::AutoDotConfig(p) => {
                eprintln!("Using config from {}", p.display());
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

    // CLI summary-by validation: warn on invalid value (parallels --mode)
    if let Some(metric) = cli.summary_by.as_deref()
        && let Err(e) = metric.parse::<cpd_core::summary::SummaryMetric>()
    {
        eprintln!("Warning: --summary-by: {} (defaulting to tokens)", e);
    }

    let config = config_result.config;
    let mut opts = Options::from_cli_and_config(&cli, &config);

    // --similarity is a ratio in (0, 1]; anything else is a typo, not a request.
    if !(opts.similarity > 0.0 && opts.similarity <= 1.0) {
        eprintln!(
            "Warning: --similarity: {} is outside (0, 1]; using 1 (exact matches only)",
            opts.similarity
        );
        opts.similarity = 1.0;
    }

    // Warn about --ignore-pattern regexes that fail to compile: the tokenizer
    // skips them, so a typo would otherwise disable the pattern with no feedback.
    for pattern in &opts.ignore_patterns {
        if let Err(e) = regex::Regex::new(pattern) {
            eprintln!(
                "Warning: --ignore-pattern: invalid regex '{}' is skipped: {}",
                pattern,
                e.to_string().lines().last().unwrap_or_default()
            );
        }
    }

    // Reject unknown format names in --format: a typo like 'cs' would
    // otherwise match 0 files and report a clean scan with exit 0 (#964,
    // #1047). Custom formats introduced via --formats-exts or --formats-names
    // are legal.
    if !opts.formats.is_empty() {
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
            std::process::exit(1);
        }
    }

    // Warn about unknown format names in --cross-formats. Custom formats
    // introduced via --formats-exts are legal, so validate against both.
    if !opts.cross_formats.is_empty() {
        let known = cpd_tokenizer::formats::list_formats();
        for group in &opts.cross_formats {
            for format in group {
                if !known.contains(&format.as_str()) && !opts.formats_exts.contains_key(format) {
                    eprintln!("Warning: --cross-formats: unknown format '{}'", format);
                }
            }
        }
    }

    // Handle --debug: print merged config as JSON and exit
    if cli.debug {
        let merged = MergedConfig::from_options(&opts);
        match serde_json::to_string_pretty(&merged) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Error serializing config: {}", e),
        }
        std::process::exit(0);
    }

    // Baseline flag validation: the dependent flags are gates and must not
    // silently no-op without a baseline source. Clap already rejects CLI-level
    // combinations of --baseline-from-ref with --baseline/--update-baseline;
    // this catches config-file/CLI mixes too.
    if opts.baseline.is_some() && opts.baseline_from_ref.is_some() {
        eprintln!("Error: use either --baseline or --baseline-from-ref, not both");
        std::process::exit(1);
    }
    if opts.update_baseline && opts.baseline.is_none() {
        eprintln!("Error: --update-baseline requires --baseline <file>");
        std::process::exit(1);
    }
    if opts.fail_on_new_clones.is_some()
        && opts.baseline.is_none()
        && opts.baseline_from_ref.is_none()
    {
        eprintln!(
            "Error: --fail-on-new-clones requires --baseline <file> or --baseline-from-ref <ref>"
        );
        std::process::exit(1);
    }

    // If no paths given, scan current directory
    let paths = if opts.paths.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))]
    } else {
        opts.paths.clone()
    };

    // A scan path that does not exist is an error, not an empty scan (#1047):
    // the walker would otherwise skip it and report a clean run with exit 0.
    let missing: Vec<&std::path::PathBuf> = paths.iter().filter(|p| !p.exists()).collect();
    if !missing.is_empty() {
        for p in &missing {
            eprintln!("Error: path does not exist: {}", p.display());
        }
        std::process::exit(1);
    }

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
            .map(|group| group.iter().map(std::path::PathBuf::from).collect())
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
    };

    // --mcp: serve the Model Context Protocol over stdio instead of running a
    // one-shot detection. stdout carries protocol messages only, so this must
    // branch before any reporter output.
    if cli.mcp {
        std::process::exit(mcp::serve(run_config));
    }

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
    let mut statistics = run_result.statistics;

    // Path normalization: make source_ids scan-root-relative for display and
    // SARIF output, while storing the scan root on Fragment.source_root so
    // reporters can reconstruct the absolute path for file reading.
    //
    // Source IDs arrive canonicalized from the finder. We canonicalize scan
    // roots here too for reliable prefix stripping (macOS /var → /private/var).
    let canonical_roots: Vec<std::path::PathBuf> = paths
        .iter()
        .map(|p| {
            let canonical = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            if canonical.is_file() {
                canonical
                    .parent()
                    .map(|d| d.to_path_buf())
                    .unwrap_or(canonical)
            } else {
                canonical
            }
        })
        .collect();
    for clone in &mut clones {
        if opts.absolute {
            make_path_absolute(&mut clone.fragment_a.source_id);
            make_path_absolute(&mut clone.fragment_b.source_id);
        } else {
            relativize_to_scan_root(&mut clone.fragment_a, &canonical_roots);
            relativize_to_scan_root(&mut clone.fragment_b, &canonical_roots);
        }
    }

    // Baseline (issue #944): mark clones absent from the baseline as new and
    // fill the newClones/newDuplicatedLines statistics before reporters run.
    // Runs after path relativization so fingerprint file reads resolve.
    if let Some(ref baseline_path) = opts.baseline {
        match cpd_reporter::baseline::apply(
            &mut clones,
            &mut statistics,
            baseline_path,
            opts.update_baseline,
        ) {
            Ok(outcome) => {
                if let Some(update) = outcome.update {
                    eprintln!(
                        "Baseline {} updated: {} fingerprints added, {} removed ({} total)",
                        baseline_path.display(),
                        update.added,
                        update.removed,
                        update.total
                    );
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else if let Some(ref base_ref) = opts.baseline_from_ref {
        // Stateless variant: scan the base ref's tree with the same
        // configuration and compare against that ephemeral baseline.
        match baseline_ref::baseline_from_ref(base_ref, &run_config) {
            Ok(base) => {
                cpd_reporter::baseline::apply_in_memory(&mut clones, &mut statistics, &base);
            }
            Err(msg) => {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }
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

    // Opt-in codebase summary. Computed after detection from data already in
    // memory; when --summary is off this is a no-op and detection output is
    // byte-identical to previous releases.
    let summary = if opts.summary {
        Some(cpd_core::summary::compute_summary(
            &run_result.sources,
            &clones,
            opts.summary_top,
            opts.summary_by,
            |id| display_source_path(id, opts.absolute, &canonical_roots),
        ))
    } else {
        None
    };

    // Opt-in duplication trend over git history (#1002): one scan per
    // selected commit in a temporary worktree, plus the current run as the
    // last point. Nothing here runs without --history / --history-since.
    let history = match history::HistorySpec::from_options(&opts) {
        None => None,
        Some(spec) => match history::collect_history(&spec, &run_config) {
            Ok(mut points) => {
                points.push(history::working_tree_point(&statistics));
                Some(cpd_core::history::History {
                    range: spec.label(),
                    threshold: opts.threshold,
                    points,
                })
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Reporter options
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
    let mut reporter_failed = false;

    // Returns (threshold exceeded, a reporter failed).
    let run_batch = |names: &[String]| -> (bool, bool) {
        let mut threshold_exceeded = false;
        let mut failed = false;
        for reporter_name in names {
            let reporter =
                match create_reporter(normalize_reporter_name(reporter_name), &reporter_opts) {
                    Some(r) => r,
                    None => {
                        eprintln!("Warning: unknown reporter '{}'", reporter_name);
                        continue;
                    }
                };

            let ctx = ReportContext::new(&statistics, elapsed)
                .with_summary(summary.as_ref())
                .with_history(history.as_ref());
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
                    failed = true;
                }
            }
        }
        (threshold_exceeded, failed)
    };

    let mut batches: Vec<&[String]> = vec![&console_names, &file_names];
    let threshold_batch = ["threshold".to_string()];
    if has_threshold {
        batches.push(&threshold_batch);
    }
    for batch in batches {
        let (exceeded, failed) = run_batch(batch);
        threshold_exceeded |= exceeded;
        reporter_failed |= failed;
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

        // Tips are interactive hints: skip them when stdout is a pipe or a
        // file (CI logs, agent hooks, `| grep`), like npm and cargo do.
        if !opts.no_tips && std::io::stdout().is_terminal() {
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

    // Empty scans (#1047): every path exists, but nothing was analyzed — the
    // --format, --ignore or --pattern filters matched no file, or every file
    // was below --min-tokens. Warn by default so an intentionally empty tree
    // still passes; --fail-on-empty makes it fatal for CI jobs where an empty
    // result means a misconfigured scan. Reports were already written above,
    // so a CI job can still inspect them.
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
    if reporter_failed {
        eprintln!("ERROR: a reporter failed to write its output (see the message above)");
    }
    if reporter_failed || (empty_scan && opts.fail_on_empty) {
        std::process::exit(1);
    }

    // Exit code logic
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
    if threshold_exceeded || new_clones_exceeded {
        std::process::exit(1);
    }
    if let Some(code) = opts.exit_code
        && !clones.is_empty()
    {
        std::process::exit(code);
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
/// works on identical strings.
fn display_source_path(id: &str, absolute: bool, canonical_roots: &[std::path::PathBuf]) -> String {
    if absolute {
        return id.to_string();
    }
    let path = std::path::Path::new(id);
    for root in canonical_roots {
        if let Ok(stripped) = path.strip_prefix(root) {
            return strip_dot_prefix(&stripped.to_string_lossy());
        }
    }
    strip_dot_prefix(id)
}

/// Strip a leading `./` or `.\` component so paths are not dot-prefixed.
fn strip_dot_prefix(s: &str) -> String {
    let mut chars = s.chars();
    if chars.next() == Some('.') {
        match chars.next() {
            Some('/') | Some('\\') => chars.as_str().to_string(),
            _ => s.to_string(),
        }
    } else {
        s.to_string()
    }
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
