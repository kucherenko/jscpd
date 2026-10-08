// `--blame` over a real repository: commits by two authors, then the blame
// of each clone line must name the author who wrote it.

use cpd_core::models::{CpdClone, Fragment, Location};
use cpd_finder::blame::enrich;
use std::path::Path;
use std::process::Command;

mod common;
use common::temp_dir;

fn git(repo: &Path, author: &str, args: &[&str]) {
    let email = format!("{}@example.com", author.to_lowercase());
    let status = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", author)
        .env("GIT_AUTHOR_EMAIL", &email)
        .env("GIT_COMMITTER_NAME", author)
        .env("GIT_COMMITTER_EMAIL", &email)
        .env("GIT_AUTHOR_DATE", "2024-01-02T03:04:05Z")
        .env("GIT_COMMITTER_DATE", "2024-01-02T03:04:05Z")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", repo)
        .status()
        .expect("the git binary is needed for blame");
    assert!(status.success(), "git {args:?}");
}

fn fragment(path: &Path, start: u32, end: u32) -> Fragment {
    Fragment::new(
        path.to_string_lossy().into_owned(),
        Location::new(start, 0, 0),
        Location::new(end, 0, 0),
        [0, 10],
    )
}

/// A repository where a.js is all Alice's and b.js line 2 is Bob's, between
/// two lines of Alice's.
fn two_author_repo(name: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let repo = temp_dir("blame-git", name);
    git(&repo, "Alice", &["init", "--quiet"]);
    let a = repo.join("a.js");
    let b = repo.join("b.js");
    std::fs::write(&a, "line one\nline two\nline three\n").unwrap();
    std::fs::write(&b, "line one\nline two\nline three\n").unwrap();
    git(&repo, "Alice", &["add", "."]);
    git(&repo, "Alice", &["commit", "--quiet", "-m", "first"]);
    std::fs::write(&b, "line one\nline TWO by bob\nline three\n").unwrap();
    git(&repo, "Bob", &["commit", "--quiet", "-am", "second"]);
    (repo, a, b)
}

#[test]
fn blame_names_the_author_of_each_line() {
    let (repo, a, b) = two_author_repo("authors");
    let mut clones = vec![CpdClone::exact(
        "javascript",
        fragment(&a, 1, 3),
        fragment(&b, 2, 3),
        10,
    )];
    let map = enrich(&mut clones, &repo);

    let author = |file: &Path, line: u32| -> String {
        map[&file.to_string_lossy().into_owned()][&line].1.clone()
    };
    assert_eq!(author(&a, 1), "Alice");
    assert_eq!(author(&a, 3), "Alice");
    assert_eq!(author(&b, 1), "Alice");
    assert_eq!(author(&b, 2), "Bob");
    assert_eq!(
        map[&a.to_string_lossy().into_owned()].len(),
        3,
        "every line"
    );

    // Each fragment carries the blame of its first line.
    let blame_a = clones[0].fragment_a.blame.as_ref().unwrap();
    let blame_b = clones[0].fragment_b.blame.as_ref().unwrap();
    assert_eq!(blame_a.author, "Alice");
    assert_eq!(blame_b.author, "Bob");
    assert_eq!(blame_b.timestamp, 1_704_164_645);
    assert_eq!(blame_b.commit_sha.len(), 40);
    assert_ne!(blame_a.commit_sha, blame_b.commit_sha);
}

#[test]
#[ignore = "known bug: blame.rs reads author lines only where git porcelain prints them (a commit's first line) and keeps the last author seen, so a line of an earlier commit after a newer one is attributed to the newer author"]
fn a_line_keeps_its_own_author_after_a_line_by_someone_else() {
    let (repo, a, b) = two_author_repo("interleaved");
    let mut clones = vec![CpdClone::exact(
        "javascript",
        fragment(&a, 1, 3),
        fragment(&b, 3, 3),
        10,
    )];
    let map = enrich(&mut clones, &repo);
    let lines = &map[&b.to_string_lossy().into_owned()];
    assert_eq!(
        lines[&3].1, "Alice",
        "line 3 of b.js is from Alice's commit"
    );
    assert_eq!(lines[&3].0, lines[&1].0, "the same commit as line 1");
    assert_eq!(clones[0].fragment_b.blame.as_ref().unwrap().author, "Alice");
}

#[test]
fn files_outside_a_repository_get_no_blame() {
    let dir = temp_dir("blame-git", "no-repo");
    let a = dir.join("a.js");
    std::fs::write(&a, "x\ny\n").unwrap();
    let mut clones = vec![CpdClone::exact(
        "javascript",
        fragment(&a, 1, 2),
        fragment(&a, 1, 2),
        10,
    )];
    let map = enrich(&mut clones, &dir);
    assert!(map.values().all(|lines| lines.is_empty()), "{map:?}");
    assert!(clones[0].fragment_a.blame.is_none());
}
