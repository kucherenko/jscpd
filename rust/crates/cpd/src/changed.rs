// changed.rs — `--changed`: the clones of the files git lists as changed.
//
// The changed files are the ones `git status` lists under the scan paths:
// staged, unstaged and untracked files, a renamed file under its new name.
// Deleted files are left out. Every file is still scanned, and a clone stays
// in the report when one of its fragments lies in a changed file, so a
// changed file's copy of unchanged code is reported too. `--changed-only`
// scans the changed files alone, and they match only one another.
//
// Which of those clones are new comes from a baseline file built from HEAD
// the way `--baseline-from-ref HEAD` builds one and saved with the commit it
// was built from. Later runs read the file instead of scanning HEAD again,
// until HEAD moves on and the file is built anew. When nothing under the
// scan paths has changed, the working tree is HEAD and gives the baseline
// without a checkout. A baseline file that names no commit, such as one
// `--update-baseline` wrote, is used as it is.

use cpd_core::models::CpdClone;
use cpd_finder::orchestrate::RunConfig;
use cpd_reporter::baseline::{self, BaselineError, BaselineFile, build, compute_fingerprints};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The baseline file `--changed` uses without `--baseline`, at the root of
/// the repository.
pub const DEFAULT_BASELINE: &str = ".jscpd-baseline.json";

/// The changed files of the repository the scan paths are in.
pub struct Changed {
    repo_root: PathBuf,
    /// Canonical paths of the changed files that exist.
    files: Arc<HashSet<PathBuf>>,
    /// Whether `git status` listed nothing under the scan paths: there the
    /// working tree is HEAD.
    clean: bool,
    /// The commit HEAD points at; `None` before the first commit.
    head: Option<String>,
}

impl Changed {
    /// Ask git which files under `paths` changed.
    pub fn list(paths: &[PathBuf]) -> Result<Self, String> {
        let first = paths.first().ok_or("--changed: no scan paths given")?;
        let repo_root = crate::find_git_root(first).ok_or_else(|| {
            format!(
                "--changed: {} is not inside a git repository",
                first.display()
            )
        })?;
        let mut command = crate::baseline_ref::git(&repo_root);
        // A file name with `*` or `[` in it is a name, not a pattern.
        command.args([
            "--literal-pathspecs",
            "status",
            "--porcelain",
            "-z",
            "--no-renames",
            "--untracked-files=all",
            "--",
        ]);
        for path in paths {
            command.arg(pathspec(&repo_root, path)?);
        }
        let output = command
            .output()
            .map_err(|e| format!("--changed: failed to run git: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "--changed: git status failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let listed = status_paths(&output.stdout);
        let clean = listed.is_empty();
        let files = listed
            .into_iter()
            .map(|path| repo_root.join(path))
            .filter(|path| path.is_file())
            .filter_map(|path| std::fs::canonicalize(path).ok())
            .collect();
        let head = head_commit(&repo_root);
        Ok(Self {
            repo_root,
            files: Arc::new(files),
            clean,
            head,
        })
    }

    /// The baseline file used when neither `--baseline` nor
    /// `--baseline-from-ref` gives one.
    pub fn default_baseline(&self) -> PathBuf {
        self.repo_root.join(DEFAULT_BASELINE)
    }

    /// The changed files, for a run that scans them alone.
    pub fn files(&self) -> Arc<HashSet<PathBuf>> {
        Arc::clone(&self.files)
    }

    /// The baseline at `path`, built from HEAD and saved when the file is
    /// missing or was built from another commit. `clones` are the run's own
    /// when it scanned every file, which on a clean tree are HEAD's; else
    /// `config` scans the files again.
    pub fn baseline(
        &self,
        path: &Path,
        config: &RunConfig,
        clones: Option<&[CpdClone]>,
    ) -> Result<BaselineFile, String> {
        let stored = match baseline::load(path) {
            Ok(file) => Some(file),
            Err(BaselineError::Missing { .. }) => None,
            Err(e) => return Err(e.to_string()),
        };
        // Before the first commit every clone is new, and there is no HEAD
        // to save.
        let Some(head) = &self.head else {
            return Ok(stored.unwrap_or_else(BaselineFile::empty));
        };
        let was = match stored {
            Some(file) if file.head.is_none() || file.head.as_ref() == Some(head) => {
                return Ok(file);
            }
            Some(file) => file.head,
            None => None,
        };
        let mut built = self.scan_head(head, config, clones)?;
        built.head = Some(head.clone());
        baseline::save(path, &built).map_err(|e| e.to_string())?;
        let shown = from_working_dir(path).display();
        let count: u64 = built.fingerprints.values().sum();
        match was {
            Some(was) => eprintln!(
                "Baseline {shown} rebuilt for HEAD {} (was {}): {count} fingerprints",
                short(head),
                short(&was)
            ),
            None => eprintln!(
                "Baseline {shown} saved from HEAD {}: {count} fingerprints",
                short(head)
            ),
        }
        Ok(built)
    }

    /// The clones of HEAD: the working tree's when nothing changed, else
    /// those of a checkout of `head`.
    fn scan_head(
        &self,
        head: &str,
        config: &RunConfig,
        clones: Option<&[CpdClone]>,
    ) -> Result<BaselineFile, String> {
        if !self.clean {
            return crate::baseline_ref::baseline_from_ref(head, config)
                .map_err(|e| e.replace("--baseline-from-ref", "--changed"));
        }
        let fingerprints = match clones {
            Some(clones) => compute_fingerprints(clones),
            None => {
                let result = cpd_finder::orchestrate::run(config)
                    .map_err(|e| format!("--changed: scan of HEAD failed: {e}"))?;
                compute_fingerprints(&result.clones)
            }
        };
        Ok(build(&fingerprints))
    }

    /// Keep the clones with a fragment in a changed file.
    pub fn retain(&self, clones: &mut Vec<CpdClone>) {
        let mut seen: HashMap<String, bool> = HashMap::new();
        clones.retain(|clone| {
            [&clone.fragment_a, &clone.fragment_b]
                .iter()
                .any(|fragment| {
                    let path = cpd_core::paths::resolve_fragment_path(fragment);
                    *seen.entry(path).or_insert_with_key(|path| {
                        std::fs::canonicalize(path).is_ok_and(|path| self.files.contains(&path))
                    })
                })
        });
    }
}

/// A commit as `git log --oneline` shows it.
fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// `path` as seen from the working directory when it lies below it.
fn from_working_dir(path: &Path) -> &Path {
    std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .ok()
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path)
}

/// `path` as a pathspec relative to `repo_root`, `/`-separated.
fn pathspec(repo_root: &Path, path: &Path) -> Result<OsString, String> {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let relative = canonical.strip_prefix(repo_root).map_err(|_| {
        format!(
            "--changed: scan path {} is outside the git repository {}",
            canonical.display(),
            repo_root.display()
        )
    })?;
    let mut spec = OsString::new();
    for part in relative.components() {
        if !spec.is_empty() {
            spec.push("/");
        }
        spec.push(part.as_os_str());
    }
    if spec.is_empty() {
        spec.push(".");
    }
    Ok(spec)
}

/// The paths of `git status --porcelain -z --no-renames`, relative to the
/// repository root: each entry is two status letters, a space and the path.
fn status_paths(output: &[u8]) -> Vec<PathBuf> {
    output
        .split(|&byte| byte == 0)
        .filter(|entry| entry.len() > 3)
        .map(|entry| path_from_bytes(&entry[3..]))
        .collect()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// Git prints paths in UTF-8 outside Unix.
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// The commit HEAD points at, `None` in a repository without commits.
fn head_commit(repo_root: &Path) -> Option<String> {
    let output = crate::baseline_ref::git(repo_root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .output()
        .ok()?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !commit.is_empty()).then_some(commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::project;
    use cpd_core::models::{CloneKind, Fragment, Location};
    use std::process::Command;

    /// Run git in `dir` with an identity that works on any machine; panics
    /// unless it succeeds.
    fn git_ok(dir: &Path, args: &[&str]) {
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
    }

    /// A repository whose one commit holds `files`.
    fn repo(files: &[(&str, &str)]) -> PathBuf {
        let root = project(files);
        git_ok(&root, &["init", "-q"]);
        git_ok(&root, &["add", "-A"]);
        git_ok(&root, &["commit", "-q", "-m", "base"]);
        root
    }

    /// The changed files, relative to `root` and sorted.
    fn names(changed: &Changed, root: &Path) -> Vec<String> {
        let mut names: Vec<String> = changed
            .files
            .iter()
            .map(|path| {
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    fn fragment(root: &Path, name: &str) -> Fragment {
        let location = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        Fragment {
            source_id: name.to_string(),
            source_root: Some(root.to_string_lossy().into_owned()),
            start: location.clone(),
            end: location,
            range: [0, 0],
            blame: None,
        }
    }

    fn clone_of(root: &Path, a: &str, b: &str) -> CpdClone {
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: fragment(root, a),
            fragment_b: fragment(root, b),
            token_count: 50,
            is_new: false,
            kind: CloneKind::Exact,
            similarity: None,
            similarity_method: None,
            structure: None,
            unmatched_lines: [0, 0],
        }
    }

    #[test]
    fn status_entries_give_their_paths_with_spaces_kept() {
        let output = b" M src/a b.js\0?? new.js\0 D gone.js\0A  staged.js\0";
        let paths: Vec<PathBuf> = status_paths(output);
        let expected: Vec<PathBuf> = ["src/a b.js", "new.js", "gone.js", "staged.js"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(paths, expected);
        assert!(status_paths(b"").is_empty());
    }

    #[test]
    fn edited_staged_and_untracked_files_are_changed_and_deleted_ones_are_not() {
        let root = repo(&[
            ("src/edited.js", "export const a = 1;\n"),
            ("src/staged.js", "export const b = 2;\n"),
            ("src/deleted.js", "export const c = 3;\n"),
            ("src/kept.js", "export const d = 4;\n"),
        ]);
        std::fs::write(root.join("src/edited.js"), "export const a = 10;\n").unwrap();
        std::fs::write(root.join("src/staged.js"), "export const b = 20;\n").unwrap();
        git_ok(&root, &["add", "src/staged.js"]);
        std::fs::remove_file(root.join("src/deleted.js")).unwrap();
        std::fs::write(root.join("src/new file [1].js"), "export const e = 5;\n").unwrap();

        let changed = Changed::list(&[root.join("src")]).unwrap();
        assert_eq!(
            names(&changed, &root),
            ["src/edited.js", "src/new file [1].js", "src/staged.js"]
        );
        assert!(!changed.clean, "a deleted file is a change");
    }

    #[test]
    fn a_renamed_file_is_changed_under_its_new_name() {
        let root = repo(&[("src/old.js", "export const a = 1;\n")]);
        git_ok(&root, &["mv", "src/old.js", "src/new.js"]);
        let changed = Changed::list(std::slice::from_ref(&root)).unwrap();
        assert_eq!(names(&changed, &root), ["src/new.js"]);
    }

    #[test]
    fn only_changes_under_the_scan_paths_count() {
        let root = repo(&[
            ("src/a.js", "export const a = 1;\n"),
            ("docs/b.js", "export const b = 2;\n"),
        ]);
        std::fs::write(root.join("docs/b.js"), "export const b = 20;\n").unwrap();
        let changed = Changed::list(&[root.join("src")]).unwrap();
        assert!(names(&changed, &root).is_empty());
        assert!(changed.clean);
    }

    #[test]
    fn a_path_outside_a_repository_is_an_error() {
        let root = project(&[("a.js", "export const a = 1;\n")]);
        match Changed::list(&[root]) {
            Err(message) => assert!(
                message.contains("is not inside a git repository"),
                "{message}"
            ),
            Ok(_) => {
                // The temp folder can sit inside a repository on some
                // machines; then there is nothing to check here.
            }
        }
    }

    #[test]
    fn a_clone_stays_when_either_fragment_is_in_a_changed_file() {
        let root = repo(&[
            ("a.js", "export const a = 1;\n"),
            ("b.js", "export const b = 2;\n"),
            ("c.js", "export const c = 3;\n"),
        ]);
        std::fs::write(root.join("b.js"), "export const b = 20;\n").unwrap();
        let changed = Changed::list(std::slice::from_ref(&root)).unwrap();

        let mut clones = vec![
            clone_of(&root, "a.js", "b.js"),
            clone_of(&root, "b.js", "c.js"),
            clone_of(&root, "a.js", "c.js"),
        ];
        changed.retain(&mut clones);
        let kept: Vec<(String, String)> = clones
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.clone(),
                    c.fragment_b.source_id.clone(),
                )
            })
            .collect();
        assert_eq!(
            kept,
            [
                ("a.js".to_string(), "b.js".to_string()),
                ("b.js".to_string(), "c.js".to_string())
            ]
        );
    }

    #[test]
    fn a_code_block_counts_as_its_file() {
        let root = repo(&[("README.md", "# a\n"), ("a.js", "export const a = 1;\n")]);
        std::fs::write(root.join("README.md"), "# b\n").unwrap();
        let changed = Changed::list(std::slice::from_ref(&root)).unwrap();
        let mut clones = vec![clone_of(&root, "README.md:javascript", "a.js")];
        changed.retain(&mut clones);
        assert_eq!(clones.len(), 1);
    }

    #[test]
    fn a_repository_without_commits_has_an_empty_baseline_and_saves_none() {
        let root = project(&[("a.js", "export const a = 1;\n")]);
        git_ok(&root, &["init", "-q"]);
        let changed = Changed::list(std::slice::from_ref(&root)).unwrap();
        assert!(!changed.clean, "an untracked file is a change");
        assert!(changed.head.is_none());
        let config = RunConfig {
            paths: vec![root.clone()],
            ..RunConfig::default()
        };
        let path = root.join(DEFAULT_BASELINE);
        let baseline = changed.baseline(&path, &config, Some(&[])).unwrap();
        assert!(baseline.fingerprints.is_empty());
        assert!(!path.exists());
    }

    /// A clean repository, the scan config of its root and the path of its
    /// baseline file.
    fn clean_repo() -> (Changed, RunConfig, PathBuf) {
        let root = repo(&[("a.js", "export const a = 1;\n")]);
        let changed = Changed::list(std::slice::from_ref(&root)).unwrap();
        assert!(changed.clean);
        let config = RunConfig {
            paths: vec![root.clone()],
            ..RunConfig::default()
        };
        (changed, config, root.join(DEFAULT_BASELINE))
    }

    fn stored(path: &Path, head: Option<&str>) -> BaselineFile {
        let mut file = build(&["cafe".to_string()]);
        file.head = head.map(str::to_string);
        baseline::save(path, &file).unwrap();
        file
    }

    #[test]
    fn a_missing_baseline_is_saved_with_the_head_commit() {
        let (changed, config, path) = clean_repo();
        let built = changed.baseline(&path, &config, Some(&[])).unwrap();
        assert_eq!(built.head, changed.head);
        assert_eq!(baseline::load(&path).unwrap(), built);
    }

    #[test]
    fn a_baseline_of_the_same_commit_is_read_as_it_is() {
        let (changed, config, path) = clean_repo();
        let file = stored(&path, changed.head.as_deref());
        assert_eq!(changed.baseline(&path, &config, Some(&[])).unwrap(), file);
    }

    #[test]
    fn a_baseline_of_another_commit_is_built_anew() {
        let (changed, config, path) = clean_repo();
        stored(&path, Some("0123456789abcdef0123456789abcdef01234567"));
        let built = changed.baseline(&path, &config, Some(&[])).unwrap();
        assert_eq!(built.head, changed.head);
        assert!(built.fingerprints.is_empty(), "the clones of HEAD");
        assert_eq!(baseline::load(&path).unwrap(), built);
    }

    #[test]
    fn a_baseline_that_names_no_commit_is_used_as_it_is() {
        let (changed, config, path) = clean_repo();
        let file = stored(&path, None);
        assert_eq!(changed.baseline(&path, &config, Some(&[])).unwrap(), file);
        assert_eq!(baseline::load(&path).unwrap(), file);
    }

    #[test]
    fn a_broken_baseline_is_an_error() {
        let (changed, config, path) = clean_repo();
        std::fs::write(&path, "{not json").unwrap();
        assert!(changed.baseline(&path, &config, Some(&[])).is_err());
    }
}
