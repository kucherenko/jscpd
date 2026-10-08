// changed.rs — `--changed`: the clones of the files git lists as changed.
//
// The changed files are the ones `git status` lists under the scan paths:
// staged, unstaged and untracked files, a renamed file under its new name.
// Deleted files and the baseline file are left out. Every file is still
// scanned, and a clone stays in the report when one of its fragments lies in
// a changed file, so a changed file's copy of unchanged code is reported too.
// `--changed-only` scans the changed files alone, and they match only one
// another.
//
// Which of those clones are new comes from a baseline file built from HEAD
// the way `--baseline-from-ref HEAD` builds one. The file names the commit
// and the scan (paths and options) it was built with, so later runs read it
// instead of scanning HEAD again until HEAD moves on or the scan changes.
// When nothing under the scan paths has changed, the working tree is HEAD
// and gives the baseline without a checkout, from the files git tracks. A
// baseline file that names no commit, such as one `--update-baseline` wrote,
// is used as it is.

use cpd_core::models::{CpdClone, Fragment};
use cpd_finder::orchestrate::RunConfig;
use cpd_reporter::baseline::{self, BaselineError, BaselineFile, build};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The baseline file `--changed` keeps without `--baseline`, at the root of
/// the repository.
pub const DEFAULT_BASELINE: &str = ".jscpd-baseline.json";

/// The changed files of the repository the scan paths are in.
pub struct Changed {
    repo_root: PathBuf,
    /// The scan paths as git pathspecs, relative to the repository root.
    pathspecs: Vec<OsString>,
    /// Canonical paths of the changed files that exist.
    files: Arc<HashSet<PathBuf>>,
    /// Whether `git status` listed nothing under the scan paths: there the
    /// working tree is HEAD.
    clean: bool,
    /// The commit HEAD points at; `None` before the first commit.
    head: Option<String>,
    /// The baseline file, absolute: the one `--baseline` names, or the
    /// default. It never counts as a changed file.
    baseline_path: PathBuf,
}

impl Changed {
    /// Ask git which files under `paths` changed. `baseline` is the file
    /// `--baseline` names.
    pub fn list(paths: &[PathBuf], baseline: Option<&Path>) -> Result<Self, String> {
        let repo_root = crate::baseline_ref::repo_of(paths, "--changed")?;
        let pathspecs = paths
            .iter()
            .map(|path| pathspec(&repo_root, path))
            .collect::<Result<Vec<_>, _>>()?;
        let baseline_path = normalized(&match baseline {
            Some(path) => std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            None => repo_root.join(DEFAULT_BASELINE),
        });
        let status = git_list(
            &repo_root,
            &[
                "status",
                "--porcelain",
                "-z",
                "--no-renames",
                "--untracked-files=all",
            ],
            &pathspecs,
        )?;
        let listed: Vec<PathBuf> = entries(&status)
            .filter(|entry| entry.len() > 3)
            .map(|entry| normalized(&repo_root.join(path_from_bytes(&entry[3..]))))
            .filter(|path| *path != baseline_path)
            .collect();
        let clean = listed.is_empty();
        let files = listed
            .into_iter()
            .filter(|path| path.is_file())
            .filter_map(|path| std::fs::canonicalize(path).ok())
            .collect();
        let head = head_commit(&repo_root);
        Ok(Self {
            repo_root,
            pathspecs,
            files: Arc::new(files),
            clean,
            head,
            baseline_path,
        })
    }

    /// The baseline file, to leave out of the scan.
    pub fn baseline_path(&self) -> &Path {
        &self.baseline_path
    }

    /// The changed files, for a run that scans them alone.
    pub fn files(&self) -> Arc<HashSet<PathBuf>> {
        Arc::clone(&self.files)
    }

    /// Which of `clones` have a fragment in a changed file.
    pub fn touching(&self, clones: &[CpdClone]) -> Vec<bool> {
        let mut resolved = Resolved::default();
        clones
            .iter()
            .map(|clone| {
                [&clone.fragment_a, &clone.fragment_b]
                    .into_iter()
                    .any(|fragment| {
                        resolved
                            .of(fragment)
                            .is_some_and(|p| self.files.contains(p))
                    })
            })
            .collect()
    }

    /// Keep the clones with a fragment in a changed file.
    pub fn retain(&self, clones: &mut Vec<CpdClone>) {
        let mut keep = self.touching(clones).into_iter();
        clones.retain(|_| keep.next().unwrap_or(false));
    }

    /// A hash of what the clones of a scan depend on besides the files: the
    /// scan paths, `options` and the jscpd version. A baseline built with
    /// another one is built again.
    pub fn scan_key(&self, options: &Value) -> String {
        let paths: Vec<String> = self
            .pathspecs
            .iter()
            .map(|spec| spec.to_string_lossy().into_owned())
            .collect();
        let key = serde_json::json!({
            "paths": paths,
            "options": sorted(options.clone()),
            "version": env!("CARGO_PKG_VERSION"),
        });
        // xxh3 of the text.
        format!("{:016x}", cpd_core::hash::token_hash(0, &key.to_string()))
    }

    /// The baseline of HEAD for a scan with `config` and `scan_key`: the
    /// baseline file when it was built for both, else built and saved.
    /// `own` are the clones and fingerprints of this run when it scanned
    /// every file; `--changed-only` has none, and a clean tree then has
    /// nothing to mark, so no baseline is built.
    pub fn baseline(
        &self,
        config: &RunConfig,
        scan_key: &str,
        own: Option<(&[CpdClone], &[String])>,
    ) -> Result<BaselineFile, String> {
        let path = &self.baseline_path;
        let stored = match baseline::load(path) {
            Ok(file) => Some(file),
            Err(BaselineError::Missing { .. }) => None,
            // A run stopped while writing it, say.
            Err(e @ BaselineError::Parse { .. }) => {
                eprintln!("Warning: {e}; building it again");
                None
            }
            Err(e) => return Err(e.to_string()),
        };
        // Before the first commit every clone is new, and there is no HEAD
        // to save.
        let Some(head) = &self.head else {
            return Ok(stored.unwrap_or_else(BaselineFile::empty));
        };
        let was = match stored {
            // The user's file: --update-baseline wrote it, or it's committed.
            Some(file) if file.head.is_none() => return Ok(file),
            Some(file)
                if file.head.as_ref() == Some(head) && file.scan.as_deref() == Some(scan_key) =>
            {
                return Ok(file);
            }
            Some(file) => file.head,
            None => None,
        };
        let Some(mut built) = self.scan_head(head, config, own)? else {
            return Ok(BaselineFile::empty());
        };
        built.head = Some(head.clone());
        built.scan = Some(scan_key.to_string());
        let shown = from_working_dir(path).display();
        let count: u64 = built.fingerprints.values().sum();
        match (baseline::save(path, &built), was) {
            (Err(e), _) => eprintln!("Warning: {e}; the baseline of HEAD is not saved"),
            (Ok(()), None) => eprintln!(
                "Baseline {shown} saved from HEAD {}: {count} fingerprints",
                short(head)
            ),
            (Ok(()), Some(was)) if was == *head => {
                eprintln!(
                    "Baseline {shown} rebuilt for other paths or options: {count} fingerprints"
                )
            }
            (Ok(()), Some(was)) => eprintln!(
                "Baseline {shown} rebuilt for HEAD {} (was {}): {count} fingerprints",
                short(head),
                short(&was)
            ),
        }
        Ok(built)
    }

    /// The clones of HEAD: on a clean tree those of this run between files
    /// git tracks, as a checkout of HEAD has no other files; else those of a
    /// checkout of `head`. `None` when this run has no clones of its own.
    fn scan_head(
        &self,
        head: &str,
        config: &RunConfig,
        own: Option<(&[CpdClone], &[String])>,
    ) -> Result<Option<BaselineFile>, String> {
        if !self.clean {
            return crate::baseline_ref::baseline_from_ref(head, config)
                .map(Some)
                .map_err(|e| e.replace("--baseline-from-ref", "--changed"));
        }
        let Some((clones, fingerprints)) = own else {
            return Ok(None);
        };
        let listing = git_list(&self.repo_root, &["ls-files", "-z"], &self.pathspecs)?;
        let tracked: HashSet<PathBuf> = entries(&listing)
            .filter_map(|entry| {
                std::fs::canonicalize(self.repo_root.join(path_from_bytes(entry))).ok()
            })
            .collect();
        let mut resolved = Resolved::default();
        let kept: Vec<String> = clones
            .iter()
            .zip(fingerprints)
            .filter(|(clone, _)| {
                [&clone.fragment_a, &clone.fragment_b]
                    .into_iter()
                    .all(|fragment| resolved.of(fragment).is_some_and(|p| tracked.contains(p)))
            })
            .map(|(_, fingerprint)| fingerprint.clone())
            .collect();
        Ok(Some(build(&kept)))
    }
}

/// The canonical path of a fragment's file, each file resolved once.
#[derive(Default)]
struct Resolved(HashMap<String, Option<PathBuf>>);

impl Resolved {
    fn of(&mut self, fragment: &Fragment) -> Option<&PathBuf> {
        let path = cpd_core::paths::resolve_fragment_path(fragment);
        self.0
            .entry(path)
            .or_insert_with_key(|path| std::fs::canonicalize(path).ok())
            .as_ref()
    }
}

/// `value` with the keys of every object in order, so equal options give
/// equal text whatever order a map held them in.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut fields: Vec<(String, Value)> = map.into_iter().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(fields.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// A commit as `git log --oneline` shows it.
fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// `path` as seen from the working directory when it lies below it.
pub fn from_working_dir(path: &Path) -> &Path {
    std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .ok()
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path)
}

/// `path` with its folder canonicalized, so a path that doesn't exist, such
/// as a deleted file, compares with canonical ones.
fn normalized(path: &Path) -> PathBuf {
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => std::fs::canonicalize(dir)
            .map(|dir| dir.join(name))
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// `path` as a pathspec relative to `repo_root`, `/`-separated.
fn pathspec(repo_root: &Path, path: &Path) -> Result<OsString, String> {
    let relative = crate::baseline_ref::repo_relative(repo_root, path, "--changed")?;
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

/// The output of a git command that lists paths under `pathspecs`. It takes
/// no lock, so a commit running at the same time does not fail on one, and
/// reads a name with `*` or `[` in it as a name.
fn git_list(repo_root: &Path, args: &[&str], pathspecs: &[OsString]) -> Result<Vec<u8>, String> {
    let output = crate::baseline_ref::git(repo_root)
        .args(["--no-optional-locks", "--literal-pathspecs"])
        .args(args)
        .arg("--")
        .args(pathspecs)
        .output()
        .map_err(|e| format!("--changed: failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "--changed: git {} failed: {}",
            args[0],
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// The NUL-separated entries of a `-z` listing.
fn entries(output: &[u8]) -> impl Iterator<Item = &[u8]> {
    output
        .split(|&byte| byte == 0)
        .filter(|entry| !entry.is_empty())
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
    use crate::testing::{git_ok, project};
    use cpd_core::models::{CloneKind, Location};

    /// A repository whose one commit holds `files`.
    fn repo(files: &[(&str, &str)]) -> PathBuf {
        let root = project(files);
        git_ok(&root, &["init", "-q"]);
        git_ok(&root, &["add", "-A"]);
        git_ok(&root, &["commit", "-q", "-m", "base"]);
        root
    }

    fn list(root: &Path) -> Changed {
        Changed::list(&[root.to_path_buf()], None).unwrap()
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
    fn listed_entries_keep_spaces_and_skip_empty_ones() {
        let output = b" M src/a b.js\0?? new.js\0\0 D gone.js\0";
        let found: Vec<&[u8]> = entries(output).collect();
        assert_eq!(
            found,
            [&b" M src/a b.js"[..], &b"?? new.js"[..], &b" D gone.js"[..]]
        );
        assert_eq!(entries(b"").count(), 0);
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

        let changed = Changed::list(&[root.join("src")], None).unwrap();
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
        assert_eq!(names(&list(&root), &root), ["src/new.js"]);
    }

    #[test]
    fn only_changes_under_the_scan_paths_count() {
        let root = repo(&[
            ("src/a.js", "export const a = 1;\n"),
            ("docs/b.js", "export const b = 2;\n"),
        ]);
        std::fs::write(root.join("docs/b.js"), "export const b = 20;\n").unwrap();
        let changed = Changed::list(&[root.join("src")], None).unwrap();
        assert!(names(&changed, &root).is_empty());
        assert!(changed.clean);
    }

    #[test]
    fn the_baseline_file_is_no_change() {
        let root = repo(&[("a.js", "export const a = 1;\n")]);
        std::fs::write(root.join(DEFAULT_BASELINE), "{}").unwrap();
        std::fs::write(root.join("mine.json"), "{}").unwrap();
        assert_eq!(names(&list(&root), &root), ["mine.json"]);
        let named =
            Changed::list(std::slice::from_ref(&root), Some(&root.join("mine.json"))).unwrap();
        assert_eq!(names(&named, &root), [DEFAULT_BASELINE]);
        std::fs::remove_file(root.join("mine.json")).unwrap();
        assert!(list(&root).clean, "the default baseline alone");
    }

    #[test]
    fn a_path_outside_a_repository_is_an_error() {
        let root = project(&[("a.js", "export const a = 1;\n")]);
        match Changed::list(&[root], None) {
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
        let changed = list(&root);

        let mut clones = vec![
            clone_of(&root, "a.js", "b.js"),
            clone_of(&root, "b.js", "c.js"),
            clone_of(&root, "a.js", "c.js"),
        ];
        assert_eq!(changed.touching(&clones), [true, true, false]);
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
        let changed = list(&root);
        let mut clones = vec![clone_of(&root, "README.md:javascript", "a.js")];
        changed.retain(&mut clones);
        assert_eq!(clones.len(), 1);
    }

    #[test]
    fn the_scan_key_follows_the_paths_and_the_options() {
        let root = repo(&[("src/a.js", "export const a = 1;\n")]);
        let whole = list(&root);
        let src = Changed::list(&[root.join("src")], None).unwrap();
        let options = serde_json::json!({"min_tokens": 50, "formats_exts": {"b": [1], "a": [2]}});
        let reordered = serde_json::json!({"formats_exts": {"a": [2], "b": [1]}, "min_tokens": 50});
        let other = serde_json::json!({"min_tokens": 30, "formats_exts": {"b": [1], "a": [2]}});
        assert_eq!(whole.scan_key(&options), whole.scan_key(&reordered));
        assert_ne!(whole.scan_key(&options), whole.scan_key(&other));
        assert_ne!(whole.scan_key(&options), src.scan_key(&options));
    }

    #[test]
    fn a_repository_without_commits_has_an_empty_baseline_and_saves_none() {
        let root = project(&[("a.js", "export const a = 1;\n")]);
        git_ok(&root, &["init", "-q"]);
        let changed = list(&root);
        assert!(!changed.clean, "an untracked file is a change");
        assert!(changed.head.is_none());
        let config = RunConfig {
            paths: vec![root.clone()],
            ..RunConfig::default()
        };
        let baseline = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
        assert!(baseline.fingerprints.is_empty());
        assert!(!root.join(DEFAULT_BASELINE).exists());
    }

    /// A clean repository, the scan config of its root and the path of its
    /// baseline file.
    fn clean_repo() -> (Changed, RunConfig, PathBuf) {
        let root = repo(&[("a.js", "export const a = 1;\n")]);
        let changed = list(&root);
        assert!(changed.clean);
        let config = RunConfig {
            paths: vec![root.clone()],
            ..RunConfig::default()
        };
        (changed, config, root.join(DEFAULT_BASELINE))
    }

    fn stored(path: &Path, head: Option<&str>, scan: Option<&str>) -> BaselineFile {
        let mut file = build(&["cafe".to_string()]);
        file.head = head.map(str::to_string);
        file.scan = scan.map(str::to_string);
        baseline::save(path, &file).unwrap();
        file
    }

    #[test]
    fn a_missing_baseline_is_saved_with_the_commit_and_the_scan() {
        let (changed, config, path) = clean_repo();
        let built = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
        assert_eq!(built.head, changed.head);
        assert_eq!(built.scan.as_deref(), Some("key"));
        assert_eq!(baseline::load(&path).unwrap(), built);
    }

    #[test]
    fn a_baseline_of_the_same_commit_and_scan_is_read_as_it_is() {
        let (changed, config, path) = clean_repo();
        let file = stored(&path, changed.head.as_deref(), Some("key"));
        let read = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
        assert_eq!(read, file);
    }

    #[test]
    fn a_baseline_of_another_commit_or_scan_is_built_anew() {
        let (changed, config, path) = clean_repo();
        let head = changed.head.clone();
        for (commit, scan) in [
            (
                Some("0123456789abcdef0123456789abcdef01234567"),
                Some("key"),
            ),
            (head.as_deref(), Some("other")),
            (head.as_deref(), None),
        ] {
            stored(&path, commit, scan);
            let built = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
            assert_eq!(built.head, head);
            assert!(built.fingerprints.is_empty(), "the clones of HEAD");
            assert_eq!(baseline::load(&path).unwrap(), built);
        }
    }

    #[test]
    fn a_baseline_that_names_no_commit_is_used_as_it_is() {
        let (changed, config, path) = clean_repo();
        let file = stored(&path, None, None);
        let read = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
        assert_eq!(read, file);
        assert_eq!(baseline::load(&path).unwrap(), file);
    }

    #[test]
    fn a_broken_baseline_is_built_again() {
        let (changed, config, path) = clean_repo();
        std::fs::write(&path, "{not json").unwrap();
        let built = changed.baseline(&config, "key", Some((&[], &[]))).unwrap();
        assert_eq!(baseline::load(&path).unwrap(), built);
    }

    #[test]
    fn a_clean_tree_without_clones_of_its_own_builds_no_baseline() {
        let (changed, config, path) = clean_repo();
        let built = changed.baseline(&config, "key", None).unwrap();
        assert!(built.fingerprints.is_empty());
        assert!(!path.exists(), "--changed-only waits for a change");
    }

    #[test]
    fn a_clean_tree_keeps_the_clones_of_tracked_files_only() {
        let root = repo(&[
            (".gitignore", "gen/\n"),
            ("a.js", "export const a = 1;\n"),
            ("b.js", "export const b = 2;\n"),
        ]);
        std::fs::create_dir_all(root.join("gen")).unwrap();
        std::fs::write(root.join("gen/c.js"), "export const c = 3;\n").unwrap();
        let changed = list(&root);
        assert!(changed.clean, "an ignored file is no change");
        let config = RunConfig {
            paths: vec![root.clone()],
            ..RunConfig::default()
        };
        let clones = [
            clone_of(&root, "a.js", "b.js"),
            clone_of(&root, "a.js", "gen/c.js"),
        ];
        let fingerprints = ["tracked".to_string(), "ignored".to_string()];
        let built = changed
            .baseline(&config, "key", Some((&clones, &fingerprints)))
            .unwrap();
        let kept: Vec<&String> = built.fingerprints.keys().collect();
        assert_eq!(kept, ["tracked"]);
    }
}
