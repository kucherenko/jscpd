// history.rs — duplication trend over git history (`--history`, issue #1002).
//
// For every selected commit, materialize its tree in a temporary detached
// worktree (the same machinery as `--baseline-from-ref`), run detection with
// the current configuration, and keep the totals. Commits come from
// `git log` over a revision range or a `--since` date, oldest first, thinned
// by `--history-every` and capped by `--history-limit` so a long range stays
// a readable series. Scans run one after another; each scan is already
// parallel across files, and parallel worktrees would only compete for the
// same cores.

use crate::baseline_ref::{add_worktree, git, map_scan_paths, remove_worktree};
use crate::options::Options;
use cpd_core::history::{HistoryPoint, WORKING_TREE};
use cpd_core::models::Statistics;
use cpd_finder::orchestrate::{RunConfig, run};
use std::path::Path;

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
    let mut cmd = git(repo_root);
    cmd.args(["log", "--reverse", "--format=%H%x1f%cs%x1f%s"]);
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
    let commits: Vec<Commit> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\u{1f}');
            let hash = parts.next()?.trim();
            if hash.is_empty() {
                return None;
            }
            Some(Commit {
                hash: hash.to_string(),
                date: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect();
    if commits.is_empty() {
        return Err(format!(
            "--history: no commits match {} in {}",
            spec.label(),
            repo_root.display()
        ));
    }
    Ok(commits)
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

/// Scan every selected commit and return one point per commit, oldest first.
pub fn collect_history(
    spec: &HistorySpec,
    run_config: &RunConfig,
) -> Result<Vec<HistoryPoint>, String> {
    let first = run_config
        .paths
        .first()
        .ok_or("--history: no scan paths given")?;
    let repo_root = crate::find_git_root(first).ok_or_else(|| {
        format!(
            "--history: {} is not inside a git repository",
            first.display()
        )
    })?;

    let commits = sample(&list_commits(&repo_root, spec)?, spec.every, spec.limit);
    let mut points = Vec::with_capacity(commits.len());
    for commit in &commits {
        let worktree = crate::baseline_ref::temp_worktree_path();
        add_worktree(&repo_root, &commit.hash, &worktree)
            .map_err(|e| e.replace("--baseline-from-ref", "--history"))?;
        let result = scan_commit(run_config, &repo_root, &worktree);
        remove_worktree(&repo_root, &worktree);
        let stats = result?;
        points.push(point_from_stats(
            commit.hash.clone(),
            commit.hash.chars().take(7).collect(),
            commit.date.clone(),
            commit.subject.clone(),
            &stats,
        ));
    }
    Ok(points)
}

/// The series' last point: the current run's own totals.
pub fn working_tree_point(stats: &Statistics) -> HistoryPoint {
    point_from_stats(
        WORKING_TREE.to_string(),
        "working".to_string(),
        stats.detection_date.chars().take(10).collect(),
        String::new(),
        stats,
    )
}

fn point_from_stats(
    commit: String,
    short: String,
    date: String,
    subject: String,
    stats: &Statistics,
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
    }
}

fn scan_commit(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
) -> Result<Statistics, String> {
    let paths = map_scan_paths(run_config, repo_root, worktree, "--history")?;
    if paths.is_empty() {
        // The scan paths did not exist at this commit: an honest zero.
        return Ok(Statistics {
            total: Default::default(),
            formats: Default::default(),
            detection_date: String::new(),
        });
    }
    let config = RunConfig {
        paths,
        blame: false,
        ..run_config.clone()
    };
    run(&config)
        .map(|r| r.statistics)
        .map_err(|e| format!("--history: scan failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
