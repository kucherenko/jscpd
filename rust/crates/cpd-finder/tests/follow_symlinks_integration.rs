//! `--follow-symlinks` path semantics (issue #1059): a file reached through a
//! link keeps the path it was found at, `--ignore` sees that same path, and a
//! file reachable through several paths is scanned once.
#![cfg(unix)]

use cpd_finder::orchestrate::{RunConfig, RunResult, run};
use cpd_tokenizer::tokenizer::Mode;
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

mod common;
use common::duplicate_js;

/// The layout from issue #1059: `root/candidate/app.js`, a copy outside the
/// scan root at `outside/S1/app.js`, the directory link `root/corpus ->
/// ../outside` and the file link `root/linked.js -> candidate/app.js`.
/// Returns the scan root.
fn layout(suffix: &str) -> PathBuf {
    let dir = common::temp_dir("follow-symlinks", suffix);
    fs::create_dir_all(dir.join("root/candidate")).unwrap();
    fs::create_dir_all(dir.join("outside/S1")).unwrap();
    fs::write(dir.join("root/candidate/app.js"), duplicate_js()).unwrap();
    fs::write(dir.join("outside/S1/app.js"), duplicate_js()).unwrap();
    symlink("../outside", dir.join("root/corpus")).unwrap();
    symlink("candidate/app.js", dir.join("root/linked.js")).unwrap();
    dir.join("root")
}

fn scan(root: &Path, follow_symlinks: bool, ignore: &[&str]) -> RunResult {
    run(&RunConfig {
        paths: vec![root.to_path_buf()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        follow_symlinks,
        ignore: ignore.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    })
    .unwrap()
}

/// Source ids relative to the canonical scan root, sorted.
fn relative_ids(result: &RunResult, root: &Path) -> Vec<String> {
    let root = fs::canonicalize(root).unwrap();
    let mut ids: Vec<String> = result
        .sources
        .iter()
        .map(|s| {
            Path::new(&s.id)
                .strip_prefix(&root)
                .unwrap_or_else(|_| panic!("{} is not under the scan root", s.id))
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    ids.sort();
    ids
}

#[test]
fn symlinks_are_not_followed_by_default() {
    let root = layout("default");
    let result = scan(&root, false, &[]);
    assert_eq!(relative_ids(&result, &root), ["candidate/app.js"]);
    assert!(result.clones.is_empty());
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn followed_files_keep_the_path_they_were_found_at() {
    let root = layout("walked-path");
    let result = scan(&root, true, &[]);
    assert_eq!(
        relative_ids(&result, &root),
        ["candidate/app.js", "corpus/S1/app.js"],
        "the walked name, not the link target, and no second entry for linked.js"
    );
    assert_eq!(result.clones.len(), 1);
    let clone = &result.clones[0];
    assert_ne!(
        clone.fragment_a.source_id, clone.fragment_b.source_id,
        "a file must not be a clone of itself"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn a_file_symlink_next_to_its_target_is_one_source() {
    let root = layout("file-link");
    fs::remove_file(root.join("corpus")).unwrap();
    let result = scan(&root, true, &[]);
    assert_eq!(relative_ids(&result, &root), ["candidate/app.js"]);
    assert!(result.clones.is_empty(), "{:?}", result.clones);
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

#[test]
fn ignore_matches_the_reported_path() {
    let root = layout("ignore");
    let by_walked_name = scan(&root, true, &["corpus/**"]);
    assert_eq!(relative_ids(&by_walked_name, &root), ["candidate/app.js"]);

    let by_link_target = scan(&root, true, &["**/outside/**"]);
    assert_eq!(
        relative_ids(&by_link_target, &root),
        ["candidate/app.js", "corpus/S1/app.js"],
        "the link target is not a name the scan ever uses"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// `jscpd --skip-local root other` shows cross-root clones only. The corpus
/// copy was found inside `root`, so its clone with `candidate/app.js` is
/// local and dropped, as in jscpd v4; the clone with `other/` survives.
#[test]
fn skip_local_treats_a_linked_corpus_as_part_of_its_root() {
    let root = layout("skip-local");
    let other = root.parent().unwrap().join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("copy.js"), duplicate_js()).unwrap();
    let result = run(&RunConfig {
        paths: vec![root.clone(), other.clone()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        follow_symlinks: true,
        skip_local: true,
        ..Default::default()
    })
    .unwrap();
    let base = root.parent().unwrap().canonicalize().unwrap();
    let rel = |id: &str| {
        Path::new(id)
            .strip_prefix(&base)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    let pairs: Vec<(String, String)> = result
        .clones
        .iter()
        .map(|c| (rel(&c.fragment_a.source_id), rel(&c.fragment_b.source_id)))
        .collect();
    assert!(!pairs.is_empty(), "the cross-root clone must survive");
    for (a, b) in &pairs {
        assert!(
            a.starts_with("other/") != b.starts_with("other/"),
            "a pair inside one root survived: {a} <-> {b}"
        );
    }
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// A `--skip-isolated` group folder that is itself a symlink: the files found
/// through it are named by the link, the folder is canonicalized to its
/// target, and the pair across the two group folders is still dropped.
#[test]
fn skip_isolated_matches_a_symlinked_group_folder() {
    let dir = common::temp_dir("follow-symlinks", "skip-isolated");
    fs::create_dir_all(dir.join("elsewhere/a")).unwrap();
    fs::create_dir_all(dir.join("root/packages/b")).unwrap();
    fs::write(dir.join("elsewhere/a/x.js"), duplicate_js()).unwrap();
    fs::write(dir.join("root/packages/b/y.js"), duplicate_js()).unwrap();
    symlink("../../elsewhere/a", dir.join("root/packages/a")).unwrap();
    let root = dir.join("root");
    let group = vec![root.join("packages/a"), root.join("packages/b")];
    let result = run(&RunConfig {
        paths: vec![root.clone()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        follow_symlinks: true,
        skip_isolated: vec![group],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        relative_ids(&result, &root),
        ["packages/a/x.js", "packages/b/y.js"]
    );
    assert!(result.clones.is_empty(), "{:?}", result.clones);
    let _ = fs::remove_dir_all(&dir);
}
