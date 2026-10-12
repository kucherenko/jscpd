// history.rs — duplication and health trend over git history (`--history`,
// issue #1002).
//
// For every selected commit, materialize its tree in a temporary detached
// worktree (the same machinery as `--baseline-from-ref`), run detection and
// dead-code analysis with the current configuration, and keep the totals
// and the health score. Commits come from `git log` over a revision range or
// a `--since` date, oldest first, thinned by `--history-every` and capped by
// `--history-limit` so a long range stays a readable series. Commits are
// scanned one after another; each scan is already parallel across files,
// and parallel worktrees would only compete for the same cores.

use crate::baseline_ref::{add_worktree, git, map_scan_paths, remove_worktree, temp_worktree_path};
use crate::options::Options;
use basta::config::BastaConfig;
use cpd_core::deadcode::{Report as DeadCodeReport, Stats as DeadCodeStats};
use cpd_core::health::HealthConfig;
use cpd_core::history::{HistoryPoint, PointHealth, WORKING_TREE};
use cpd_core::models::{CpdClone, SourceFile, Statistics};
use cpd_core::summary::{Summary, SummaryMetric, compute_summary};
use cpd_finder::orchestrate::RunConfig;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// What `--history` was asked to walk.
#[derive(Debug, Clone, PartialEq)]
pub struct HistorySpec {
    /// `git log` revision range, e.g. `v5.0.0..HEAD`. `HEAD` when only a date
    /// was given.
    pub range: String,
    /// `--since` date passed to `git log`, if any.
    pub since: Option<String>,
    /// Keep every Nth commit, counted back from the newest.
    pub every: usize,
    /// Maximum number of commits in the series.
    pub limit: usize,
}

impl HistorySpec {
    /// `None` when `--history` / `--history-since` were not given.
    pub fn from_options(opts: &Options) -> Option<Self> {
        if opts.history.is_none() && opts.history_since.is_none() {
            return None;
        }
        Some(Self {
            range: opts.history.clone().unwrap_or_else(|| "HEAD".to_string()),
            since: opts.history_since.clone(),
            every: opts.history_every.max(1),
            limit: opts.history_limit.max(1),
        })
    }

    /// Display label for the report header.
    pub fn label(&self) -> String {
        match &self.since {
            Some(since) if self.range == "HEAD" => format!("since {since}"),
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

/// List the commits `git log` yields for the spec, oldest first.
pub fn list_commits(repo_root: &Path, spec: &HistorySpec) -> Result<Vec<Commit>, String> {
    crate::baseline_ref::check_revision("--history", &spec.range)?;
    let mut cmd = git(repo_root);
    // `log.showSignature` would print each signed commit's signature check
    // on stdout, among the commit lines (#1195).
    cmd.args([
        "log",
        "--no-show-signature",
        "--reverse",
        "--format=%H%x1f%cs%x1f%s",
    ]);
    if let Some(since) = &spec.since {
        cmd.arg(format!("--since={since}"));
    }
    cmd.arg(&spec.range).arg("--");
    let output = cmd
        .output()
        .map_err(|e| format!("--history: failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "--history: git log {} failed: {}",
            spec.range,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
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
        Self {
            dead_code: dead_code.map(|config| BastaConfig {
                rust_diagnostics: None,
                ..config
            }),
            config,
        }
    }

    /// The dead-code run over a tree scanned at `paths`; `None` when no
    /// format it reads was selected.
    fn dead_code_config(&self, paths: &[PathBuf]) -> Option<BastaConfig> {
        self.dead_code.as_ref().map(|config| BastaConfig {
            paths: paths.to_vec(),
            ..config.clone()
        })
    }

    /// Dead code in the tree at `paths`, the way a point is scored from it.
    pub fn dead_code(&self, paths: &[PathBuf]) -> Option<DeadCodeReport> {
        crate::dashboard::dead_code_report(&self.dead_code_config(paths)?)
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

/// Every file of a scanned tree, ranked by complexity: what the health
/// score reads. `display_path` must give the paths the clones carry.
pub fn full_summary(
    sources: &[SourceFile],
    clones: &[CpdClone],
    display_path: impl Fn(&str) -> String,
) -> Summary {
    compute_summary(
        sources,
        clones,
        usize::MAX,
        SummaryMetric::Complexity,
        display_path,
    )
}

/// Scan every selected commit and return one point per commit, oldest
/// first. With `progress`, a line on a terminal's stderr says which commit
/// is being scanned.
pub fn collect_history(
    spec: &HistorySpec,
    run_config: &RunConfig,
    health: &HealthScan,
    progress: bool,
) -> Result<Vec<HistoryPoint>, String> {
    let repo_root = crate::baseline_ref::repo_of(&run_config.paths, "--history")?;
    let commits = sample(&list_commits(&repo_root, spec)?, spec.every, spec.limit);
    let mut progress = Progress::new(progress, commits.len());
    let mut points = Vec::with_capacity(commits.len());
    for commit in &commits {
        progress.next(commit);
        let worktree = temp_worktree_path();
        add_worktree(&repo_root, &commit.hash, &worktree)
            .map_err(|e| e.replace("--baseline-from-ref", "--history"))?;
        let result = scan_commit(run_config, &repo_root, &worktree, health);
        remove_worktree(&repo_root, &worktree);
        let (stats, health) = result?;
        points.push(point_from_stats(
            commit.hash.clone(),
            commit.hash.chars().take(7).collect(),
            commit.date.clone(),
            commit.subject.clone(),
            &stats,
            health,
        ));
    }
    Ok(points)
}

/// The series' last point: the current run's own totals and health.
pub fn working_tree_point(stats: &Statistics, health: PointHealth) -> HistoryPoint {
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
    health: PointHealth,
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
        health: Some(health),
    }
}

/// The totals and the health of the tree checked out at `worktree`: the
/// clone scan and the dead-code scan side by side, as `--dashboard` runs
/// them.
fn scan_commit(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
    health: &HealthScan,
) -> Result<(Statistics, PointHealth), String> {
    let paths = map_scan_paths(run_config, repo_root, worktree, "--history")?;
    if paths.is_empty() {
        // The scan paths did not exist at this commit: an honest zero, and
        // no code to score.
        let nothing = full_summary(&[], &[], str::to_string);
        let stats = Statistics {
            total: Default::default(),
            formats: Default::default(),
            detection_date: String::new(),
        };
        return Ok((stats, health.score(&nothing, &[], None)));
    }
    let config = RunConfig {
        paths,
        blame: false,
        ..run_config.clone()
    };
    let (clone_workers, dead_code_workers) = crate::dashboard::split_workers(config.workers);
    let (result, dead_code) = crate::dashboard::scan(
        &config,
        health.dead_code_config(&config.paths),
        clone_workers,
        dead_code_workers,
    );
    let mut clones = result.clones;
    let roots = crate::canonical_roots(&config.paths);
    crate::display_paths(&mut clones, false, &roots);
    let summary = full_summary(&result.sources, &clones, |id| {
        crate::display_source_path(id, false, &roots)
    });
    let health = health.score(
        &summary,
        &clones,
        dead_code.as_ref().map(|report| &report.statistics),
    );
    Ok((result.statistics, health))
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
            commit.hash.get(..7).unwrap_or(&commit.hash),
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
        let scan = HealthScan::new(Some(dead_code), config);
        assert!(scan.config.metrics.is_empty());
        assert_eq!(scan.config.complex_file, Some(80), "tuning stays");
        let paths = [PathBuf::from("worktree/src")];
        let commit = scan.dead_code_config(&paths).unwrap();
        assert!(commit.rust_diagnostics.is_none());
        assert_eq!(commit.paths, paths);
        assert!(
            HealthScan::new(None, HealthConfig::default())
                .dead_code_config(&paths)
                .is_none()
        );
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
        };
        assert_eq!(spec("v5.0.0..HEAD", None).label(), "v5.0.0..HEAD");
        assert_eq!(spec("HEAD", Some("2026-01-01")).label(), "since 2026-01-01");
        assert_eq!(
            spec("main", Some("2026-01-01")).label(),
            "main since 2026-01-01"
        );
    }
}
