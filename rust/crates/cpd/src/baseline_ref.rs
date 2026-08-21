// baseline_ref.rs — ephemeral clone baseline from a git ref (issue #944, phase 2).
//
// `--baseline-from-ref <ref>` is the stateless variant of the committed
// baseline: materialize the base ref's tree in a temporary detached git
// worktree, run the same detection configuration against it, fingerprint the
// clones it contains, and compare the current run against that in-memory
// baseline. Nothing is committed to the repository and the worktree is
// removed afterwards. Like blame enrichment, this shells out to the `git`
// binary rather than linking a git implementation.
//
// Cost: the corpus is scanned twice (base tree + working tree). The
// committed-baseline mode (`--baseline`) needs a single scan and no git
// history — prefer it where a baseline file can be committed.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_reporter::baseline::{BaselineFile, build, compute_fingerprints};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build an in-memory baseline from the clones present in `git_ref`'s tree,
/// scanned with the same configuration as the current run.
pub fn baseline_from_ref(git_ref: &str, run_config: &RunConfig) -> Result<BaselineFile, String> {
    let first = run_config
        .paths
        .first()
        .ok_or("--baseline-from-ref: no scan paths given")?;
    let repo_root = crate::find_git_root(first).ok_or_else(|| {
        format!(
            "--baseline-from-ref: {} is not inside a git repository",
            first.display()
        )
    })?;

    verify_ref(&repo_root, git_ref)?;

    let worktree = temp_worktree_path();
    add_worktree(&repo_root, git_ref, &worktree)?;
    let result = scan_base_tree(run_config, &repo_root, &worktree);
    remove_worktree(&repo_root, &worktree);
    result
}

fn git(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_root);
    cmd
}

fn verify_ref(repo_root: &Path, git_ref: &str) -> Result<(), String> {
    let output = git(repo_root)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{}^{{commit}}", git_ref))
        .output()
        .map_err(|e| format!("--baseline-from-ref: failed to run git: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "--baseline-from-ref: git ref '{}' not found in {} — in shallow CI checkouts fetch \
             the base ref first (e.g. `git fetch origin main`, or actions/checkout with \
             `fetch-depth: 0`)",
            git_ref,
            repo_root.display()
        ));
    }
    Ok(())
}

fn temp_worktree_path() -> PathBuf {
    // A process-wide counter keeps concurrent runs (e.g. parallel tests) from
    // colliding on the same worktree directory.
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "cpd-base-ref-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

fn add_worktree(repo_root: &Path, git_ref: &str, worktree: &Path) -> Result<(), String> {
    let output = git(repo_root)
        .args(["worktree", "add", "--detach"])
        .arg(worktree)
        .arg(git_ref)
        .output()
        .map_err(|e| format!("--baseline-from-ref: failed to run git: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "--baseline-from-ref: could not check out '{}': {}",
            git_ref,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Best-effort cleanup: `git worktree remove` unregisters and deletes in one
/// step; fall back to deleting the directory and pruning the registration.
fn remove_worktree(repo_root: &Path, worktree: &Path) {
    let removed = git(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !removed {
        let _ = std::fs::remove_dir_all(worktree);
        let _ = git(repo_root).args(["worktree", "prune"]).output();
    }
}

/// Run detection over the base tree with the current run's configuration and
/// fingerprint the clones it contains. Scan paths are remapped from the
/// working tree into the worktree; paths that don't exist in the base ref are
/// simply new code with nothing to record.
fn scan_base_tree(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
) -> Result<BaselineFile, String> {
    let mut base_paths = Vec::new();
    for path in &run_config.paths {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let rel = canonical.strip_prefix(repo_root).map_err(|_| {
            format!(
                "--baseline-from-ref: scan path {} is outside the git repository {}",
                canonical.display(),
                repo_root.display()
            )
        })?;
        let mapped = worktree.join(rel);
        if mapped.exists() {
            base_paths.push(mapped);
        }
    }
    if base_paths.is_empty() {
        return Ok(BaselineFile::empty());
    }

    let base_config = RunConfig {
        paths: base_paths,
        blame: false,
        ..run_config.clone()
    };
    let result = run(&base_config)
        .map_err(|e| format!("--baseline-from-ref: scan of the base ref failed: {}", e))?;
    Ok(build(&compute_fingerprints(&result.clones)))
}
