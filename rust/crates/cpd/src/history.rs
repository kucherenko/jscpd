// history.rs — duplication and health trend over git history (`--history`,
// issue #1002).
//
// For every selected commit, materialize its tree in a temporary detached
// worktree (the same machinery as `--baseline-from-ref`), run detection and
// dead-code analysis with the current configuration, and keep the totals
// and the health score. Commits come from `git log --first-parent` over a
// revision range or a `--since` date, oldest first, so that every point is
// a state of the branch itself and a merged branch counts as its merge.
// `--history-every` thins them and `--history-limit` caps them so a long
// range stays a readable series; with neither a range nor a date, the
// series is the newest commits of the current branch, as many as it takes.
// The commits are chosen before anything is scanned, so a range git cannot
// read fails at once. They are scanned one after another; each scan is
// already parallel across files, and parallel worktrees would only compete
// for the same cores.

use crate::baseline_ref::{Worktree, git, worktree_config};
use crate::dashboard::full_summary;
use crate::options::Options;
use basta::config::BastaConfig;
use cpd_core::deadcode::{Report as DeadCodeReport, Stats as DeadCodeStats};
use cpd_core::health::HealthConfig;
use cpd_core::history::{HistoryPoint, PointHealth, WORKING_TREE};
use cpd_core::models::{CpdClone, Statistics};
use cpd_core::summary::Summary;
use cpd_finder::orchestrate::RunConfig;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// The range walked when none was given: the current branch. A bare
/// `--history` takes its newest commits, `--history-since` alone every
/// commit since the date.
pub const DEFAULT_RANGE: &str = "HEAD";

/// What `--history` was asked to walk.
#[derive(Debug, Clone, PartialEq)]
pub struct HistorySpec {
    /// `git log` revision range, e.g. `v5.0.0..HEAD`. `HEAD` when none was
    /// given.
    pub range: String,
    /// `--since` date passed to `git log`, if any.
    pub since: Option<String>,
    /// Keep every Nth commit, counted back from the newest.
    pub every: usize,
    /// Maximum number of commits in the series.
    pub limit: usize,
    /// Neither a range nor a date was given: walk only the newest commits,
    /// as many as the series takes, rather than sample the whole branch.
    pub latest: bool,
}

impl HistorySpec {
    /// `None` when `--history` / `--history-since` were not given.
    pub fn from_options(opts: &Options) -> Option<Self> {
        if opts.history.is_none() && opts.history_since.is_none() {
            return None;
        }
        // An empty range is a bare `--history` the config gave no range.
        let range = opts.history.as_deref().filter(|range| !range.is_empty());
        Some(Self {
            range: range.unwrap_or(DEFAULT_RANGE).to_string(),
            since: opts.history_since.clone(),
            every: opts.history_every.max(1),
            limit: opts.history_limit.max(1),
            latest: range.is_none() && opts.history_since.is_none(),
        })
    }

    /// How many commits `git log` lists: for the newest commits, just enough
    /// for `limit` points `every` commits apart; else no cap. git reads the
    /// count as a C `int`.
    fn max_count(&self) -> Option<usize> {
        self.latest.then(|| {
            (self.limit - 1)
                .saturating_mul(self.every)
                .saturating_add(1)
                .min(i32::MAX as usize)
        })
    }

    /// Display label for the report header.
    pub fn label(&self) -> String {
        match &self.since {
            Some(since) if self.range == DEFAULT_RANGE => format!("since {since}"),
            Some(since) => format!("{} since {since}", self.range),
            None => self.range.clone(),
        }
    }
}

/// A commit selected for scanning.
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub hash: String,
    pub date: String,
    pub subject: String,
}

/// List the commits `git log` yields for the spec, oldest first: the first
/// parents only, the states the branch itself went through.
pub fn list_commits(repo_root: &Path, spec: &HistorySpec) -> Result<Vec<Commit>, String> {
    crate::baseline_ref::check_revision("--history", &spec.range)?;
    let mut cmd = git(repo_root);
    // `log.showSignature` would print each signed commit's signature check
    // on stdout, among the commit lines (#1195).
    cmd.args([
        "log",
        "--no-show-signature",
        "--first-parent",
        "--reverse",
        "--format=%H%x1f%cs%x1f%s",
    ]);
    if let Some(since) = &spec.since {
        cmd.arg(format!("--since={since}"));
    }
    // Applied before --reverse: the newest commits, listed oldest first.
    if let Some(count) = spec.max_count() {
        cmd.arg(format!("--max-count={count}"));
    }
    cmd.arg(&spec.range).arg("--");
    let output = cmd
        .output()
        .map_err(|e| format!("--history: failed to run git: {e}"))?;
    if !output.status.success() {
        let mut message = format!(
            "--history: git log {} failed: {}",
            spec.range,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        // The range is optional, so `jscpd --history src` reads the path
        // as the range.
        if Path::new(&spec.range).exists() {
            message.push_str(&format!(
                "\n'{0}' is a path, not a git revision: write the paths before the flag (jscpd {0} --history), or a range with an equals sign (jscpd --history=v5.0.0..HEAD {0})",
                spec.range
            ));
        }
        return Err(message);
    }
    let commits = parse_log(&String::from_utf8_lossy(&output.stdout));
    if commits.is_empty() {
        return Err(format!(
            "--history: no commits match {} in {}",
            spec.label(),
            repo_root.display()
        ));
    }
    Ok(commits)
}

/// The commits in the output of `git log --format=%H%x1f%cs%x1f%s`. A line
/// that does not start with a full commit hash is something else git printed,
/// such as a signature check, and is skipped.
fn parse_log(stdout: &str) -> Vec<Commit> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\u{1f}');
            let hash = parts.next()?.trim();
            let date = parts.next()?;
            if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Some(Commit {
                hash: hash.to_string(),
                date: date.to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// The commit of `listed` that a shallow clone fetched without its
/// parents, if there is one: the walk could go no further there, wherever
/// the range ends. Git lists such commits in the `shallow` file of the
/// repository.
fn shallow_edge<'a>(repo_root: &Path, listed: &'a [Commit]) -> Option<&'a Commit> {
    let output = git(repo_root)
        .args(["rev-parse", "--git-path", "shallow"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let path = match path.is_absolute() {
        true => path,
        false => repo_root.join(path),
    };
    let shallow = std::fs::read_to_string(path).ok()?;
    let edges: HashSet<&str> = shallow.lines().map(str::trim).collect();
    listed
        .iter()
        .find(|commit| edges.contains(commit.hash.as_str()))
}

/// The commits of a series, chosen before anything is scanned.
#[derive(Debug)]
pub struct Selection {
    repo_root: PathBuf,
    /// Oldest first.
    commits: Vec<Commit>,
    /// What was walked, for display: [`HistorySpec::label`], or for the
    /// newest commits of a longer branch the range that holds them,
    /// `HEAD~30..HEAD`.
    pub range: String,
    /// How many commits the range has, when the series samples fewer.
    pub sampled_from: Option<usize>,
}

/// List the commits of `spec` in the repository of the scan `paths`, say so
/// when a shallow clone cut the walk short, and sample them.
pub fn select(spec: &HistorySpec, paths: &[PathBuf]) -> Result<Selection, String> {
    let repo_root = crate::baseline_ref::repo_of(paths, "--history")?;
    let listed = list_commits(&repo_root, spec)?;
    // `jscpd --history docs` walks a branch named docs, which the reader
    // may not have meant.
    if spec.range != DEFAULT_RANGE && Path::new(&spec.range).exists() {
        eprintln!(
            "Warning: --history: '{0}' is a git revision and a path; the series walks the revision. To scan the path, write it before the flag: jscpd {0} --history",
            spec.range
        );
    }
    // A walk that listed as many commits as it asked for did not run out of
    // history, wherever a shallow clone ends.
    let full = spec.max_count().filter(|count| listed.len() >= *count);
    if full.is_none()
        && let Some(edge) = shallow_edge(&repo_root, &listed)
    {
        eprintln!(
            "Warning: --history: the repository is a shallow clone and the walk stops at {} ({}), the oldest commit fetched, so the series may miss older commits; fetch them first (git fetch --unshallow, or fetch-depth: 0 for actions/checkout)",
            short(&edge.hash),
            edge.date
        );
    }
    let range = match full {
        Some(count) if has_ancestor(&repo_root, count) => format!("HEAD~{count}..HEAD"),
        _ => spec.label(),
    };
    let commits = sample(&listed, spec.every, spec.limit);
    Ok(Selection {
        sampled_from: (commits.len() < listed.len()).then_some(listed.len()),
        commits,
        range,
        repo_root,
    })
}

/// Whether `HEAD` has a first-parent ancestor `count` commits back: then
/// `HEAD~count..HEAD` is the range of its newest `count` commits.
fn has_ancestor(repo_root: &Path, count: usize) -> bool {
    git(repo_root)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{DEFAULT_RANGE}~{count}^{{commit}}"))
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A commit hash as a table shows it.
fn short(hash: &str) -> &str {
    hash.get(..7).unwrap_or(hash)
}

/// Thin a series: keep every Nth item counted back from the newest (so the
/// newest always stays), then cap the length by evenly spaced sampling that
/// keeps both ends.
pub fn sample<T: Clone>(items: &[T], every: usize, limit: usize) -> Vec<T> {
    let every = every.max(1);
    let n = items.len();
    let thinned: Vec<T> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| (n - 1 - i).is_multiple_of(every))
        .map(|(_, item)| item.clone())
        .collect();
    let limit = limit.max(1);
    let m = thinned.len();
    if m <= limit {
        return thinned;
    }
    if limit == 1 {
        return vec![thinned[m - 1].clone()];
    }
    (0..limit)
        .map(|k| thinned[k * (m - 1) / (limit - 1)].clone())
        .collect()
}

/// How each point of a series is scored for health: the dead-code run, when
/// a format it reads is selected, and the `health` config. The metrics of
/// other tools (`--health-input`) and the compiler's Rust diagnostics are
/// left out of every point, the working tree's included: they describe the
/// working tree alone, and a score that had them at one point only would
/// not compare with the rest.
#[derive(Debug, Clone)]
pub struct HealthScan {
    dead_code: Option<BastaConfig>,
    config: HealthConfig,
}

impl HealthScan {
    pub fn new(dead_code: Option<BastaConfig>, mut config: HealthConfig) -> Self {
        config.metrics.clear();
        // A dimension of no weight is left out of the score: nothing to
        // scan for.
        let weighed = config.dead_code.weight != Some(0.0);
        Self {
            dead_code: dead_code.filter(|_| weighed).map(|config| BastaConfig {
                rust_diagnostics: None,
                ..config
            }),
            config,
        }
    }

    /// Whether a point is scored from its dead code: a format dead-code
    /// analysis reads was selected, and the dimension weighs something.
    pub fn reads_dead_code(&self) -> bool {
        self.dead_code.is_some()
    }

    /// The dead-code run over a tree scanned at `paths`; a commit's paths
    /// stand for the working tree's `match_as` (see
    /// [`RunConfig::match_as`]). `None` when the points read no dead code.
    fn dead_code_config(&self, paths: &[PathBuf], match_as: &[PathBuf]) -> Option<BastaConfig> {
        self.dead_code.as_ref().map(|config| BastaConfig {
            paths: paths.to_vec(),
            match_as: match_as.to_vec(),
            ..config.clone()
        })
    }

    /// Dead code in the tree at `paths`, the way a point is scored from it,
    /// analyzed by `workers` threads.
    pub fn dead_code(&self, paths: &[PathBuf], workers: Option<usize>) -> Option<DeadCodeReport> {
        let config = BastaConfig {
            workers,
            ..self.dead_code_config(paths, &[])?
        };
        crate::dashboard::dead_code_report(&config).report
    }

    /// The health of a tree, from the summary of every file, its clones
    /// with the paths the summary has, and its dead code.
    pub fn score(
        &self,
        summary: &Summary,
        clones: &[CpdClone],
        dead_code: Option<&DeadCodeStats>,
    ) -> PointHealth {
        PointHealth::from(&cpd_core::health::compute(
            summary,
            clones,
            dead_code,
            &self.config,
        ))
    }
}

/// Scan every commit of `selection` and return one point per commit,
/// oldest first, scored for health when there is a `health` scan. With
/// `progress`, a line on a terminal's stderr says which commit is being
/// scanned.
pub fn collect_history(
    selection: &Selection,
    run_config: &RunConfig,
    health: Option<&HealthScan>,
    progress: bool,
) -> Result<Vec<HistoryPoint>, String> {
    let mut progress = Progress::new(progress, selection.commits.len());
    let mut points = Vec::with_capacity(selection.commits.len());
    for commit in &selection.commits {
        progress.next(commit);
        let worktree = Worktree::add(&selection.repo_root, &commit.hash)
            .map_err(|e| e.replace("--baseline-from-ref", "--history"))?;
        let (stats, health) =
            scan_commit(run_config, &selection.repo_root, worktree.path(), health)?;
        drop(worktree);
        points.push(point_from_stats(
            commit.hash.clone(),
            short(&commit.hash).to_string(),
            commit.date.clone(),
            commit.subject.clone(),
            &stats,
            health,
        ));
    }
    Ok(points)
}

/// The series' last point: the current run's own totals and health.
pub fn working_tree_point(stats: &Statistics, health: Option<PointHealth>) -> HistoryPoint {
    point_from_stats(
        WORKING_TREE.to_string(),
        "working".to_string(),
        stats.detection_date.chars().take(10).collect(),
        String::new(),
        stats,
        health,
    )
}

fn point_from_stats(
    commit: String,
    short: String,
    date: String,
    subject: String,
    stats: &Statistics,
    health: Option<PointHealth>,
) -> HistoryPoint {
    let t = &stats.total;
    HistoryPoint {
        commit,
        short,
        date,
        subject,
        sources: t.sources,
        lines: t.lines,
        tokens: t.tokens,
        clones: t.clones,
        duplicated_lines: t.duplicated_lines,
        percentage: t.percentage,
        health,
    }
}

/// The totals and the health of the tree checked out at `worktree`: the
/// clone scan and the dead-code scan side by side, as `--dashboard` runs
/// them.
fn scan_commit(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
    health: Option<&HealthScan>,
) -> Result<(Statistics, Option<PointHealth>), String> {
    let Some(config) = worktree_config(run_config, repo_root, worktree, "--history")? else {
        // The scan paths did not exist at this commit: an honest zero, and
        // no code to score.
        let nothing = full_summary(&[], &[], str::to_string);
        let stats = Statistics {
            total: Default::default(),
            formats: Default::default(),
            detection_date: String::new(),
        };
        return Ok((
            stats,
            health.map(|health| health.score(&nothing, &[], None)),
        ));
    };
    let (clone_workers, dead_code_workers) = series_workers(config.workers);
    let (result, dead_code) = crate::dashboard::scan(
        &config,
        health.and_then(|health| health.dead_code_config(&config.paths, &config.match_as)),
        clone_workers,
        dead_code_workers,
    );
    let mut clones = result.clones;
    let roots = crate::canonical_roots(&config.paths);
    crate::display_paths(&mut clones, false, &roots);
    let summary = full_summary(&result.sources, &clones, |id| {
        crate::display_source_path(id, false, &roots)
    });
    let health = health.map(|health| {
        health.score(
            &summary,
            &clones,
            dead_code.report.as_ref().map(|report| &report.statistics),
        )
    });
    Ok((result.statistics, health))
}

/// The threads of the clone scan of a tree of a series, and of the
/// dead-code analysis beside it; `None` for the second when the two run one
/// after the other. `--workers` is a budget for the whole run, which the
/// two share as they do in `--dashboard`. Without it, the clone scan takes
/// every core and the analysis runs beside it with half as many threads:
/// most of its time is one thread resolving the module graph, which then
/// overlaps the clone scan rather than halving its share. On two cores
/// that takes a tenth off a series of vite, and on eight it is as fast.
pub(crate) fn series_workers(workers: Option<usize>) -> (usize, Option<usize>) {
    if workers.is_some() {
        return crate::dashboard::split_workers(workers);
    }
    match std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get) {
        1 => (1, None),
        cores => (cores, Some(cores / 2)),
    }
}

/// Which commit is being scanned, on one line of stderr that each commit
/// overwrites and the end of the walk clears. Only on a terminal: in a log,
/// a line per commit would be noise.
struct Progress {
    live: bool,
    total: usize,
    done: usize,
    /// Characters of the line on screen, to clear.
    width: usize,
}

impl Progress {
    fn new(enabled: bool, total: usize) -> Self {
        Self {
            live: enabled && std::io::stderr().is_terminal(),
            total,
            done: 0,
            width: 0,
        }
    }

    fn next(&mut self, commit: &Commit) {
        self.done += 1;
        if !self.live {
            return;
        }
        let line = format!(
            "  history: scanning commit {} of {} ({} {})",
            self.done,
            self.total,
            short(&commit.hash),
            commit.date
        );
        let width = line.chars().count();
        eprint!("\r{line}{}", " ".repeat(self.width.saturating_sub(width)));
        self.width = width;
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if self.live && self.width > 0 {
            eprint!("\r{}\r", " ".repeat(self.width));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_health_scan_leaves_out_what_describes_the_working_tree_alone() {
        let dead_code = BastaConfig {
            rust_diagnostics: Some(basta::config::RustDiagnostics {
                text: String::new(),
                base: None,
            }),
            ..BastaConfig::default()
        };
        let config = HealthConfig {
            metrics: vec![cpd_core::health::ExternalMetric {
                id: "coverage".to_string(),
                score: Some(10.0),
                ..Default::default()
            }],
            complex_file: Some(80),
            ..HealthConfig::default()
        };
        let scan = HealthScan::new(Some(dead_code.clone()), config.clone());
        assert!(scan.config.metrics.is_empty());
        assert_eq!(scan.config.complex_file, Some(80), "tuning stays");
        let paths = [PathBuf::from("worktree/src")];
        let tree = [PathBuf::from("src")];
        let commit = scan.dead_code_config(&paths, &tree).unwrap();
        assert!(commit.rust_diagnostics.is_none());
        assert_eq!(commit.paths, paths);
        assert_eq!(commit.match_as, tree, "ignore matches as the working tree");
        assert!(
            HealthScan::new(None, HealthConfig::default())
                .dead_code_config(&paths, &[])
                .is_none()
        );
        // Dead code of no weight is left out of the score: not scanned.
        let weightless = HealthConfig {
            dead_code: cpd_core::health::Tuning {
                weight: Some(0.0),
                ..Default::default()
            },
            ..config
        };
        assert!(!HealthScan::new(Some(dead_code), weightless).reads_dead_code());
    }

    #[test]
    fn workers_given_are_a_budget_and_otherwise_the_clone_scan_takes_every_core() {
        assert_eq!(series_workers(Some(8)), (4, Some(4)));
        assert_eq!(series_workers(Some(1)), (1, None));
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let (clones, dead_code) = series_workers(None);
        assert_eq!(clones, cores);
        assert_eq!(dead_code, (cores > 1).then_some(cores / 2));
    }

    #[test]
    fn sample_keeps_newest_when_thinning() {
        let items: Vec<u32> = (1..=10).collect();
        assert_eq!(sample(&items, 3, 100), vec![1, 4, 7, 10]);
        assert_eq!(sample(&items, 1, 100), items);
    }

    #[test]
    fn sample_caps_length_keeping_both_ends() {
        let items: Vec<u32> = (1..=10).collect();
        assert_eq!(sample(&items, 1, 4), vec![1, 4, 7, 10]);
        assert_eq!(sample(&items, 1, 1), vec![10]);
        assert_eq!(sample(&items, 1, 2), vec![1, 10]);
        assert_eq!(sample(&[1u32], 5, 5), vec![1]);
        assert!(sample(&Vec::<u32>::new(), 1, 3).is_empty());
    }

    #[test]
    fn parse_log_skips_lines_that_are_not_commits() {
        let hash = "25e660d789a2eebcf26ee9b3d2d2cadae42be8b8";
        let stdout = format!(
            "Good \"git\" signature for repro@example.com with ED25519 key SHA256:nCNRGw9p\n\
             {hash}\u{1f}2026-10-11\u{1f}one\n\
             No signature\n\
             \n"
        );
        assert_eq!(
            parse_log(&stdout),
            vec![Commit {
                hash: hash.to_string(),
                date: "2026-10-11".to_string(),
                subject: "one".to_string(),
            }]
        );
    }

    #[test]
    fn label_describes_range_and_since() {
        let spec = |range: &str, since: Option<&str>| HistorySpec {
            range: range.to_string(),
            since: since.map(str::to_string),
            every: 1,
            limit: 30,
            latest: false,
        };
        assert_eq!(spec("v5.0.0..HEAD", None).label(), "v5.0.0..HEAD");
        assert_eq!(spec("HEAD", Some("2026-01-01")).label(), "since 2026-01-01");
        assert_eq!(
            spec("main", Some("2026-01-01")).label(),
            "main since 2026-01-01"
        );
    }

    #[test]
    fn a_bare_history_walks_the_newest_commits_and_a_range_or_a_date_all_of_them() {
        use clap::Parser;
        let spec = |args: &[&str], config: Option<&str>| {
            let cli = crate::cli::Cli::parse_from(args);
            let config = crate::cli::ConfigFile {
                history: config.map(str::to_string),
                ..crate::cli::ConfigFile::default()
            };
            let opts = Options::from_cli_and_config(&cli, &config);
            HistorySpec::from_options(&opts).map(|spec| (spec.range.clone(), spec.max_count()))
        };
        let head = |count| Some(("HEAD".to_string(), count));
        assert_eq!(spec(&["cpd", "--history"], None), head(Some(30)));
        assert_eq!(
            spec(&["cpd", "--history", "--history-limit", "5"], None),
            head(Some(5))
        );
        assert_eq!(
            spec(
                &[
                    "cpd",
                    "--history",
                    "--history-limit",
                    "5",
                    "--history-every",
                    "3"
                ],
                None
            ),
            head(Some(13)),
            "5 points 3 commits apart"
        );
        assert_eq!(
            spec(&["cpd", "--history", "--history-limit", "3000000000"], None),
            head(Some(i32::MAX as usize)),
            "git reads the count as an int"
        );
        assert_eq!(spec(&["cpd", "--history=HEAD"], None), head(None));
        assert_eq!(
            spec(&["cpd", "--history-since", "2026-01-01"], None),
            head(None)
        );
        assert_eq!(
            spec(&["cpd", "--history", "--history-since", "2026-01-01"], None),
            head(None)
        );
        assert_eq!(
            spec(&["cpd", "--history"], Some("v5.0.0..HEAD")),
            Some(("v5.0.0..HEAD".to_string(), None)),
            "the config's range"
        );
        assert_eq!(spec(&["cpd"], None), None);
        // The listed window thins to exactly the points asked for.
        let window: Vec<u32> = (1..=13).collect();
        assert_eq!(sample(&window, 3, 5), vec![1, 4, 7, 10, 13]);
    }
}
