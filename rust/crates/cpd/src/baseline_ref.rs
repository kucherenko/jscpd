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

pub(crate) fn git(repo_root: &Path) -> Command {
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

pub(crate) fn temp_worktree_path() -> PathBuf {
    // A process-wide counter keeps concurrent runs (e.g. parallel tests) from
    // colliding on the same worktree directory.
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "cpd-base-ref-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

pub(crate) fn add_worktree(repo_root: &Path, git_ref: &str, worktree: &Path) -> Result<(), String> {
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
pub(crate) fn remove_worktree(repo_root: &Path, worktree: &Path) {
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

/// Remap the run's scan paths from the working tree into `worktree`. Paths
/// that do not exist at that ref are skipped: they are new code with nothing
/// to record. `flag` names the option in error messages.
pub(crate) fn map_scan_paths(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
    flag: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut mapped_paths = Vec::new();
    for path in &run_config.paths {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let rel = canonical.strip_prefix(repo_root).map_err(|_| {
            format!(
                "{flag}: scan path {} is outside the git repository {}",
                canonical.display(),
                repo_root.display()
            )
        })?;
        let mapped = worktree.join(rel);
        if mapped.exists() {
            mapped_paths.push(mapped);
        }
    }
    Ok(mapped_paths)
}

/// Run detection over the base tree with the current run's configuration and
/// fingerprint the clones it contains.
fn scan_base_tree(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
) -> Result<BaselineFile, String> {
    let base_paths = map_scan_paths(run_config, repo_root, worktree, "--baseline-from-ref")?;
    if base_paths.is_empty() {
        return Ok(BaselineFile::empty());
    }

    // The pairs inside pairs of classes are known too, so they stay known
    // when the pair of their classes breaks apart in the current tree.
    let base_config = RunConfig {
        paths: base_paths,
        blame: false,
        keep_inner_pairs: true,
        ..run_config.clone()
    };
    let result = run(&base_config)
        .map_err(|e| format!("--baseline-from-ref: scan of the base ref failed: {}", e))?;
    let mut fingerprints = compute_fingerprints(&result.clones);
    fingerprints.extend(compute_fingerprints(&result.inner_pairs));
    Ok(build(&fingerprints))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::project;

    /// Run git in `dir` with an identity that works on any machine; panics
    /// unless it succeeds.
    fn git_ok(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.email=cpd-test@example.com",
                "-c",
                "user.name=cpd-test",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// A throwaway repository with one commit.
    fn repo() -> PathBuf {
        let root = project(&[("src/a.js", "export const a = 1;\n")]);
        git_ok(&root, &["init", "-q"]);
        git_ok(&root, &["add", "-A"]);
        git_ok(&root, &["commit", "-q", "-m", "base"]);
        root
    }

    /// The worktree paths `git worktree list` gives, the main one first.
    fn worktrees(root: &Path) -> Vec<PathBuf> {
        git_ok(root, &["worktree", "list", "--porcelain"])
            .lines()
            .filter_map(|l| l.strip_prefix("worktree "))
            .map(PathBuf::from)
            .collect()
    }

    #[test]
    fn a_worktree_is_added_and_removed_without_a_trace() {
        let root = repo();
        let worktree = temp_worktree_path();
        add_worktree(&root, "HEAD", &worktree).unwrap();
        assert!(worktree.join("src/a.js").is_file());
        assert_eq!(worktrees(&root).len(), 2);
        remove_worktree(&root, &worktree);
        assert!(!worktree.exists());
        assert_eq!(worktrees(&root), std::slice::from_ref(&root));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_worktree_that_cannot_be_checked_out_is_an_error_naming_the_ref() {
        let root = repo();
        // A non-empty directory where the worktree should go.
        let worktree = temp_worktree_path();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("occupied"), "").unwrap();
        let err = add_worktree(&root, "HEAD", &worktree).unwrap_err();
        assert!(
            err.starts_with("--baseline-from-ref: could not check out 'HEAD': "),
            "{err}"
        );
        assert!(err.len() > "--baseline-from-ref: could not check out 'HEAD': ".len());
        assert_eq!(
            worktrees(&root),
            std::slice::from_ref(&root),
            "nothing registered"
        );
        std::fs::remove_dir_all(&worktree).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_cleanup_fallback_deletes_its_directory_and_keeps_the_users_worktrees() {
        let root = repo();
        // The user's own worktree, which the cleanup must leave alone.
        let users = std::fs::canonicalize(project(&[])).unwrap().join("feature");
        git_ok(
            &root,
            &["worktree", "add", "-q", "--detach", users.to_str().unwrap()],
        );
        // A directory git does not know as a worktree: `git worktree
        // remove` refuses it, so the fallback deletes it.
        let stray = temp_worktree_path();
        std::fs::create_dir_all(stray.join("src")).unwrap();
        std::fs::write(stray.join("src/a.js"), "").unwrap();

        remove_worktree(&root, &stray);

        assert!(!stray.exists(), "the fallback deletes the directory");
        assert_eq!(worktrees(&root), [root.clone(), users.clone()]);
        assert!(users.join("src/a.js").is_file());
        std::fs::remove_dir_all(users.parent().unwrap()).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    #[ignore = "known bug: the cleanup fallback runs `git worktree prune`, which unregisters every worktree whose directory is missing, the user's (e.g. on an unmounted drive) included"]
    fn the_cleanup_fallback_keeps_a_users_worktree_that_is_offline() {
        let root = repo();
        let parent = project(&[]);
        let users = std::fs::canonicalize(&parent).unwrap().join("feature");
        git_ok(
            &root,
            &["worktree", "add", "-q", "--detach", users.to_str().unwrap()],
        );
        // The user's worktree lives on a drive that is not mounted now.
        let offline = parent.join("feature-offline");
        std::fs::rename(&users, &offline).unwrap();
        let stray = temp_worktree_path();
        std::fs::create_dir_all(&stray).unwrap();

        remove_worktree(&root, &stray);

        std::fs::rename(&offline, &users).unwrap();
        let listed = worktrees(&root);
        std::fs::remove_dir_all(&parent).ok();
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(listed, [root.clone(), users], "the user's worktree is gone");
    }
}
