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
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Build an in-memory baseline from the clones present in `git_ref`'s tree,
/// scanned with the same configuration as the current run.
pub fn baseline_from_ref(git_ref: &str, run_config: &RunConfig) -> Result<BaselineFile, String> {
    let repo_root = repo_of(&run_config.paths, "--baseline-from-ref")?;

    check_revision("--baseline-from-ref", git_ref)?;
    verify_ref(&repo_root, git_ref)?;

    let worktree = Worktree::add(&repo_root, git_ref)?;
    scan_base_tree(run_config, &repo_root, worktree.path())
}

pub(crate) fn git(repo_root: &Path) -> Command {
    cpd_finder::git::command(repo_root)
}

/// The git repository of the first scan path. `flag` names the option in
/// error messages.
pub(crate) fn repo_of(paths: &[PathBuf], flag: &str) -> Result<PathBuf, String> {
    let first = paths
        .first()
        .ok_or_else(|| format!("{flag}: no scan paths given"))?;
    crate::find_git_root(first)
        .ok_or_else(|| format!("{flag}: {} is not inside a git repository", first.display()))
}

/// `path`, canonicalized, relative to `repo_root`. `flag` names the option
/// in error messages.
pub(crate) fn repo_relative(repo_root: &Path, path: &Path, flag: &str) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match canonical.strip_prefix(repo_root) {
        Ok(relative) => Ok(relative.to_path_buf()),
        Err(_) => Err(format!(
            "{flag}: scan path {} is outside the git repository {}",
            canonical.display(),
            repo_root.display()
        )),
    }
}

/// Refuse a ref or range that git would read as an option (`--output=...`),
/// whether it came from the command line or a repository's config file.
pub(crate) fn check_revision(flag: &str, value: &str) -> Result<(), String> {
    if cpd_finder::git::looks_like_option(value) {
        return Err(format!(
            "{flag}: '{value}' is not a git revision (it starts with '-')"
        ));
    }
    Ok(())
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

/// A commit checked out in a temporary worktree, removed again when this
/// is dropped: after its scan, on an error, and while a panic unwinds. A
/// build that aborts on a panic runs no destructor; a panic hook removes
/// the worktrees still checked out there (see [`remove_on_abort`]).
pub(crate) struct Worktree {
    repo_root: PathBuf,
    path: PathBuf,
}

impl Worktree {
    /// Check `git_ref` of `repo_root` out in a new temporary worktree.
    pub(crate) fn add(repo_root: &Path, git_ref: &str) -> Result<Self, String> {
        remove_on_abort();
        let worktree = Self {
            repo_root: repo_root.to_path_buf(),
            path: temp_worktree_path(),
        };
        // Listed before the checkout, so that one which fails half-way is
        // removed as well when `worktree` drops.
        checked_out().push((worktree.repo_root.clone(), worktree.path.clone()));
        add_worktree(repo_root, git_ref, &worktree.path)?;
        Ok(worktree)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        remove_worktree(&self.repo_root, &self.path);
        checked_out().retain(|(_, path)| path != &self.path);
    }
}

/// The worktrees of this process that are checked out now, with their
/// repositories.
static CHECKED_OUT: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

fn checked_out() -> MutexGuard<'static, Vec<(PathBuf, PathBuf)>> {
    CHECKED_OUT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// With `panic = "abort"`, as the release build has it, a panic ends the
/// process before any destructor runs, which would leave a worktree
/// registered in the user's repository: a hook removes the ones still
/// checked out, after the hook that reports the panic. Installed once.
fn remove_on_abort() {
    #[cfg(panic = "abort")]
    {
        static INSTALL: std::sync::Once = std::sync::Once::new();
        INSTALL.call_once(|| {
            let report = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                report(info);
                let checked_out = match CHECKED_OUT.try_lock() {
                    Ok(guard) => guard,
                    Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => return,
                };
                for (repo_root, path) in checked_out.iter() {
                    remove_worktree(repo_root, path);
                }
            }));
        });
    }
}

/// A hooks folder that cannot exist, for `core.hooksPath`: `/dev/null` is
/// no directory, and the temporary folder of a Windows user is theirs.
fn no_hooks() -> PathBuf {
    match cfg!(windows) {
        true => std::env::temp_dir().join("cpd-no-hooks"),
        false => PathBuf::from("/dev/null"),
    }
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
    // The checkout is jscpd's, not the user's: a post-checkout hook (an
    // `npm install`, say) has no business running on it, once per commit of
    // a series, and Git LFS would download every object of the commit,
    // where a pointer is all a scan needs.
    let output = git(repo_root)
        .arg("-c")
        .arg(format!("core.hooksPath={}", no_hooks().display()))
        .args(["worktree", "add", "--detach"])
        .arg(worktree)
        .arg(git_ref)
        .env("GIT_LFS_SKIP_SMUDGE", "1")
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
/// step. If it fails, delete the directory and this worktree's own entry
/// under `.git/worktrees`. `git worktree prune` is not an option: it also
/// unregisters the user's worktrees whose directories are absent for the
/// moment, such as one on an unmounted drive.
pub(crate) fn remove_worktree(repo_root: &Path, worktree: &Path) {
    let removed = git(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !removed {
        let admin_dir = worktree_admin_dir(worktree);
        let _ = std::fs::remove_dir_all(worktree);
        if let Some(admin_dir) = admin_dir {
            let _ = std::fs::remove_dir_all(admin_dir);
        }
    }
}

/// The `.git/worktrees/<name>` directory a linked worktree's `.git` file
/// points at, if it is one.
fn worktree_admin_dir(worktree: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(worktree.join(".git")).ok()?;
    let target = PathBuf::from(link.strip_prefix("gitdir:")?.trim());
    let target = if target.is_absolute() {
        target
    } else {
        worktree.join(target)
    };
    // Only ever delete an entry inside a `worktrees` folder of a git dir.
    let parent = target.parent()?;
    (parent.file_name()? == "worktrees").then_some(target)
}

/// The run's configuration for a scan of `worktree`, a checkout of a commit
/// of `repo_root`: the scan paths, the `--skip-isolated` folders and the
/// skipped files moved into the worktree, and each scan path standing for
/// the working tree's, so that `--ignore` and `--pattern` take the files
/// the working tree's scan takes wherever the worktree lives. Scan paths
/// that do not exist at that commit are left out; `None` when none does.
/// `flag` names the option in error messages.
pub(crate) fn worktree_config(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
    flag: &str,
) -> Result<Option<RunConfig>, String> {
    let mut paths = Vec::new();
    let mut match_as = Vec::new();
    for path in &run_config.paths {
        let mapped = worktree.join(repo_relative(repo_root, path, flag)?);
        if mapped.exists() {
            paths.push(mapped);
            match_as.push(path.clone());
        }
    }
    if paths.is_empty() {
        return Ok(None);
    }
    // A path outside the repository has no copy in the worktree.
    let moved = |path: &PathBuf| match repo_relative(repo_root, path, flag) {
        Ok(relative) => worktree.join(relative),
        Err(_) => path.clone(),
    };
    Ok(Some(RunConfig {
        paths,
        match_as,
        skip_isolated: run_config
            .skip_isolated
            .iter()
            .map(|group| group.iter().map(moved).collect())
            .collect(),
        // Compared with the canonical paths the walk gives its files.
        skip_files: run_config
            .skip_files
            .iter()
            .map(|file| {
                let moved = moved(file);
                std::fs::canonicalize(&moved).unwrap_or(moved)
            })
            .collect(),
        blame: false,
        ..run_config.clone()
    }))
}

/// Run detection over the base tree with the current run's configuration and
/// fingerprint the clones it contains.
fn scan_base_tree(
    run_config: &RunConfig,
    repo_root: &Path,
    worktree: &Path,
) -> Result<BaselineFile, String> {
    let Some(base_config) =
        worktree_config(run_config, repo_root, worktree, "--baseline-from-ref")?
    else {
        return Ok(BaselineFile::empty());
    };
    let result = run(&base_config)
        .map_err(|e| format!("--baseline-from-ref: scan of the base ref failed: {}", e))?;
    Ok(build(&compute_fingerprints(&result.clones)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{git_ok, project};

    /// A throwaway repository with one commit.
    fn repo() -> PathBuf {
        let root = project(&[("src/a.js", "export const a = 1;\n")]);
        git_ok(&root, &["init", "-q"]);
        git_ok(&root, &["add", "-A"]);
        git_ok(&root, &["commit", "-q", "-m", "base"]);
        root
    }

    /// `path` as git takes and prints it: on Windows without the `\\?\`
    /// prefix `canonicalize` adds, which git cannot use.
    fn plain(path: &Path) -> PathBuf {
        let text = path.to_string_lossy();
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
    }

    /// `path` in the form `git worktree list` prints, for comparisons.
    fn listed(path: &Path) -> String {
        plain(path).to_string_lossy().replace('\\', "/")
    }

    /// The worktree paths `git worktree list` gives, the main one first.
    fn worktrees(root: &Path) -> Vec<String> {
        git_ok(root, &["worktree", "list", "--porcelain"])
            .lines()
            .filter_map(|l| l.strip_prefix("worktree "))
            .map(|p| listed(Path::new(p)))
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
        assert_eq!(worktrees(&root), [listed(&root)]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_checked_out_worktree_goes_when_it_drops_and_a_failed_one_leaves_nothing() {
        let root = repo();
        let worktree = Worktree::add(&root, "HEAD").unwrap();
        let path = worktree.path().to_path_buf();
        assert!(path.join("src/a.js").is_file());
        assert!(checked_out().iter().any(|(_, p)| p == &path));
        drop(worktree);
        assert!(!path.exists());
        assert!(!checked_out().iter().any(|(_, p)| p == &path));
        assert_eq!(worktrees(&root), [listed(&root)]);

        let err = Worktree::add(&root, "nosuchref").err().unwrap();
        assert!(err.contains("could not check out 'nosuchref'"), "{err}");
        assert_eq!(worktrees(&root), [listed(&root)], "nothing registered");
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_runs_none_of_the_repositorys_hooks() {
        use std::os::unix::fs::PermissionsExt;
        let root = repo();
        let marker = root.join("hook-ran");
        let hook = root.join(".git/hooks/post-checkout");
        std::fs::write(
            &hook,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let worktree = Worktree::add(&root, "HEAD");
        let ran = marker.exists();
        drop(worktree);
        std::fs::remove_dir_all(&root).ok();
        assert!(!ran, "the post-checkout hook ran");
    }

    #[test]
    fn a_worktrees_config_moves_the_runs_paths_into_it() {
        let root = repo();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let worktree = Worktree::add(&root, "HEAD").unwrap();
        let wt = worktree.path().to_path_buf();
        let config = RunConfig {
            paths: vec![root.join("src"), root.join("lib")],
            skip_isolated: vec![vec![root.join("src/a"), root.join("src/b")]],
            skip_files: vec![root.join("src/a.js")],
            blame: true,
            ..RunConfig::default()
        };
        let mapped = worktree_config(&config, &root, &wt, "--history")
            .unwrap()
            .expect("src is there");
        assert_eq!(mapped.paths, [wt.join("src")], "lib is not committed");
        assert_eq!(mapped.match_as, [root.join("src")]);
        assert_eq!(
            mapped.skip_isolated,
            [vec![wt.join("src/a"), wt.join("src/b")]]
        );
        assert_eq!(
            mapped.skip_files,
            [std::fs::canonicalize(wt.join("src/a.js")).unwrap()]
        );
        assert!(!mapped.blame);
        let none = RunConfig {
            paths: vec![root.join("lib")],
            ..RunConfig::default()
        };
        assert!(
            worktree_config(&none, &root, &wt, "--history")
                .unwrap()
                .is_none()
        );
        drop(worktree);
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
        assert_eq!(worktrees(&root), [listed(&root)], "nothing registered");
        std::fs::remove_dir_all(&worktree).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_cleanup_fallback_deletes_its_directory_and_keeps_the_users_worktrees() {
        let root = repo();
        // The user's own worktree, which the cleanup must leave alone.
        let users = plain(&project(&[])).join("feature");
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
        assert_eq!(worktrees(&root), [listed(&root), listed(&users)]);
        assert!(users.join("src/a.js").is_file());
        std::fs::remove_dir_all(users.parent().unwrap()).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_cleanup_fallback_unregisters_its_own_worktree_and_no_other() {
        let root = repo();
        let parent = project(&[]);
        let users = plain(&parent).join("feature");
        git_ok(
            &root,
            &["worktree", "add", "-q", "--detach", users.to_str().unwrap()],
        );
        let offline = parent.join("feature-offline");
        std::fs::rename(&users, &offline).unwrap();
        // A lock makes `git worktree remove --force` refuse our worktree, so
        // the fallback deletes it and its entry under `.git/worktrees`.
        let worktree = temp_worktree_path();
        add_worktree(&root, "HEAD", &worktree).unwrap();
        git_ok(&root, &["worktree", "lock", worktree.to_str().unwrap()]);

        remove_worktree(&root, &worktree);

        std::fs::rename(&offline, &users).unwrap();
        let listed_now = worktrees(&root);
        let entries = std::fs::read_dir(root.join(".git/worktrees"))
            .map(|dir| dir.count())
            .unwrap_or(0);
        let gone = !worktree.exists();
        std::fs::remove_dir_all(&parent).ok();
        std::fs::remove_dir_all(&root).ok();
        assert!(gone, "the fallback deletes the directory");
        assert_eq!(listed_now, [listed(&root), listed(&users)]);
        assert_eq!(entries, 1, "only the user's entry is left");
    }

    #[test]
    fn the_cleanup_fallback_keeps_a_users_worktree_that_is_offline() {
        let root = repo();
        let parent = project(&[]);
        let users = plain(&parent).join("feature");
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
        let listed_now = worktrees(&root);
        std::fs::remove_dir_all(&parent).ok();
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(
            listed_now,
            [listed(&root), listed(&users)],
            "the user's worktree is gone"
        );
    }
}
