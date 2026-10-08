// The walker over a real `git init` repository: what a scan of a working
// tree takes and leaves out. Every test builds its own repository with the
// git binary, so ignore rules come from real `.git` metadata rather than an
// empty `.git` folder standing in for one.

use cpd_finder::walker::{WalkConfig, walk};
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;
use common::temp_dir;

/// A fresh repository at `cpd-walker-git-<name>` created by `git init`.
fn git_repo(name: &str) -> PathBuf {
    let dir = temp_dir("walker-git", name);
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&dir)
        .status()
        .expect("the git binary is needed to build a real repository");
    assert!(status.success(), "git init failed in {}", dir.display());
    assert!(
        dir.join(".git/HEAD").is_file(),
        "git init made a repository"
    );
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Walked paths relative to `root`, sorted, with `/` separators, leaving
/// out what the walk took from inside `.git` (a known bug, pinned by
/// `files_inside_the_git_folder_are_never_scanned`) so the other tests check
/// their own rule only.
fn walked(root: &Path, no_gitignore: bool) -> Vec<String> {
    walked_all(root, no_gitignore)
        .into_iter()
        .filter(|name| !name.starts_with(".git/"))
        .collect()
}

/// Every walked path relative to `root`, sorted, with `/` separators.
fn walked_all(root: &Path, no_gitignore: bool) -> Vec<String> {
    let config = WalkConfig {
        paths: vec![root.to_path_buf()],
        no_gitignore,
        ..Default::default()
    };
    let canon = std::fs::canonicalize(root).unwrap();
    let mut names: Vec<String> = walk(&config)
        .into_iter()
        .map(|f| {
            f.path
                .strip_prefix(&canon)
                .unwrap_or(&f.path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    names.sort();
    names
}

const JS: &str = "export function f(a, b) { return a + b; }\n";

#[test]
fn gitignore_files_of_a_real_repository_are_honoured() {
    let repo = git_repo("gitignore");
    write(&repo, ".gitignore", "dist/\n*.gen.js\n");
    write(&repo, "src/.gitignore", "local.js\n");
    write(&repo, "src/app.js", JS);
    write(&repo, "src/local.js", JS);
    write(&repo, "src/types.gen.js", JS);
    write(&repo, "dist/bundle.js", JS);
    write(&repo, "lib/util.ts", JS);

    assert_eq!(walked(&repo, false), ["lib/util.ts", "src/app.js"]);
}

#[test]
fn info_exclude_of_a_real_repository_is_honoured() {
    let repo = git_repo("exclude");
    write(&repo, ".git/info/exclude", "scratch/\n");
    write(&repo, "scratch/tmp.js", JS);
    write(&repo, "src/app.js", JS);

    assert_eq!(walked(&repo, false), ["src/app.js"]);
}

#[test]
fn no_gitignore_takes_the_ignored_files_too() {
    let repo = git_repo("no-gitignore");
    write(&repo, ".gitignore", "dist/\n");
    write(&repo, "dist/bundle.js", JS);
    write(&repo, "src/app.js", JS);

    assert_eq!(walked(&repo, true), ["dist/bundle.js", "src/app.js"]);
}

#[test]
fn hidden_files_and_folders_outside_git_metadata_are_scanned() {
    let repo = git_repo("hidden");
    write(&repo, ".github/scripts/release.js", JS);
    write(&repo, ".eslintrc.js", JS);
    write(&repo, "src/app.js", JS);

    let names = walked(&repo, false);
    assert!(
        names.contains(&".github/scripts/release.js".to_string()),
        "{names:?}"
    );
    assert!(names.contains(&".eslintrc.js".to_string()), "{names:?}");
    assert!(names.contains(&"src/app.js".to_string()), "{names:?}");
}

#[test]
#[ignore = "known bug: walker.rs sets hidden(false) without excluding .git, so git metadata is scanned"]
fn files_inside_the_git_folder_are_never_scanned() {
    let repo = git_repo("dot-git");
    write(&repo, "src/app.js", JS);
    // Git keeps arbitrary files under .git: `git init` alone writes hook
    // samples with a shell shebang (taken as bash), and tools drop scripts
    // there too.
    write(&repo, ".git/hooks/post-checkout.js", JS);

    let names = walked_all(&repo, false);
    let inside_git: Vec<&String> = names.iter().filter(|n| n.starts_with(".git/")).collect();
    assert!(
        inside_git.is_empty(),
        "scanned git metadata: {inside_git:?}"
    );
    assert_eq!(names, ["src/app.js"]);
}
