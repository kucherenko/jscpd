// rust/crates/cpd/tests/changed.rs — `--changed` end to end: the clones of
// the files git lists as changed, against a baseline of HEAD that the first
// run saves.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

/// Run git in `dir` with an identity that works on any machine; panics
/// unless it succeeds.
fn git(dir: &Path, args: &[&str]) {
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

/// One of two functions of about 60 tokens that share no code: `known`,
/// copied in `a.js` and `b.js`, and `helper`.
fn function(name: &str) -> String {
    match name {
        "known" => r#"export function known(rows, limit, label) {
  const picked = rows.filter((row) => row.size > limit);
  const total = picked.reduce((sum, row) => sum + row.size, 0);
  const average = picked.length ? total / picked.length : 0;
  const report = `${label}: ${picked.length} rows, ${average} on average`;
  console.log(report);
  return { picked, total, average, report };
}
"#
        .to_string(),
        _ => r#"export class Queue {
  constructor(capacity) {
    this.items = new Array(capacity);
    this.head = 0;
    this.tail = 0;
  }
  push(item) {
    if (this.tail - this.head === this.items.length) throw new Error("full");
    this.items[this.tail++ % this.items.length] = item;
  }
  shift() {
    if (this.tail === this.head) return undefined;
    return this.items[this.head++ % this.items.length];
  }
}
"#
        .to_string(),
    }
}

/// A repository whose commit holds a clone between `src/a.js` and
/// `src/b.js` and a function of its own in `src/lib.js`.
fn repo() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "cpd-it-changed-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::write(root.join("src/a.js"), function("known")).unwrap();
    std::fs::write(root.join("src/b.js"), function("known")).unwrap();
    std::fs::write(root.join("src/lib.js"), function("helper")).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    root
}

/// `jscpd --changed` on `src/` from the repository root, with a JSON report
/// in `out/`.
fn run_changed(root: &Path, extra: &[&str]) -> Output {
    run_flag(root, "--changed", extra)
}

fn run_flag(root: &Path, flag: &str, extra: &[&str]) -> Output {
    Command::new(cpd_bin())
        .current_dir(root)
        .args([flag, "--min-tokens", "20", "-r", "json", "-o", "out"])
        .args(extra)
        .arg("src")
        .output()
        .expect("failed to run cpd")
}

/// The commit HEAD points at, shortened.
fn head(root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The `statistics.total.sources` of the JSON report.
fn files_analyzed(root: &Path) -> u64 {
    let text = std::fs::read_to_string(root.join("out/jscpd-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    report["statistics"]["total"]["sources"].as_u64().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The clones of the JSON report: the two file names, sorted, and whether
/// the clone is new.
fn clones(root: &Path) -> Vec<(String, String, bool)> {
    let text = std::fs::read_to_string(root.join("out/jscpd-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut found: Vec<(String, String, bool)> = report["duplicates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|clone| {
            let mut names = [
                clone["firstFile"]["name"].as_str().unwrap().to_string(),
                clone["secondFile"]["name"].as_str().unwrap().to_string(),
            ];
            names.sort();
            let [a, b] = names;
            (a, b, clone["isNew"].as_bool().unwrap_or(false))
        })
        .collect();
    found.sort();
    found
}

fn fingerprints(path: &Path) -> u64 {
    baseline(path)["fingerprints"]
        .as_object()
        .unwrap()
        .values()
        .map(|count| count.as_u64().unwrap())
        .sum()
}

fn baseline(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_first_run_saves_the_baseline_of_head_and_reports_the_changed_files() {
    let root = repo();
    // A new file copies the function of an unchanged one.
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();

    let output = run_changed(&root, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let baseline = root.join(".jscpd-baseline.json");
    let saved = format!(
        "Baseline .jscpd-baseline.json saved from HEAD {}: 1 fingerprints",
        head(&root)
    );
    assert!(stderr(&output).contains(&saved), "{}", stderr(&output));
    assert_eq!(fingerprints(&baseline), 1, "the clone HEAD has");
    let commit = self::baseline(&baseline)["head"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(commit.starts_with(&head(&root)), "{commit}");
    // The clone of a.js and b.js changed nowhere and is left out.
    assert_eq!(
        clones(&root),
        [("copy.js".to_string(), "lib.js".to_string(), true)]
    );
}

#[test]
fn the_next_run_reads_the_saved_baseline() {
    let root = repo();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    assert!(run_changed(&root, &[]).status.success());
    let baseline = root.join(".jscpd-baseline.json");
    let saved = std::fs::read_to_string(&baseline).unwrap();

    // b.js changes outside its copy of a.js: the old clone is reported, and
    // it is not new.
    let edited = format!("{}\nexport const version = 2;\n", function("known"));
    std::fs::write(root.join("src/b.js"), edited).unwrap();
    let output = run_changed(&root, &["--fail-on-new-clones", "0"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("1 new clones not in the baseline"),
        "{}",
        stderr(&output)
    );
    assert!(!stderr(&output).contains("saved from HEAD"));
    assert_eq!(
        clones(&root),
        [
            ("a.js".to_string(), "b.js".to_string(), false),
            ("copy.js".to_string(), "lib.js".to_string(), true)
        ]
    );
    assert_eq!(std::fs::read_to_string(&baseline).unwrap(), saved);
}

#[test]
fn a_clean_tree_gives_the_baseline_and_no_clones() {
    let root = repo();
    let output = run_changed(&root, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(fingerprints(&root.join(".jscpd-baseline.json")), 1);
    assert!(clones(&root).is_empty());
}

#[test]
fn the_baseline_file_can_be_named() {
    let root = repo();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    let output = run_changed(&root, &["--baseline", "dup.json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(fingerprints(&root.join("dup.json")), 1);
    assert!(!root.join(".jscpd-baseline.json").exists());
}

#[test]
fn update_baseline_takes_the_working_tree() {
    let root = repo();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    let output = run_changed(&root, &["--update-baseline"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let file = root.join(".jscpd-baseline.json");
    assert_eq!(fingerprints(&file), 2);
    assert!(baseline(&file).get("head").is_none(), "used as it is");
    assert_eq!(
        clones(&root),
        [("copy.js".to_string(), "lib.js".to_string(), true)]
    );

    let output = run_changed(&root, &["--fail-on-new-clones", "0"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn a_ref_gives_the_baseline_instead_of_a_file() {
    let root = repo();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    let output = run_changed(&root, &["--baseline-from-ref", "HEAD"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!root.join(".jscpd-baseline.json").exists());
    assert_eq!(
        clones(&root),
        [("copy.js".to_string(), "lib.js".to_string(), true)]
    );
}

#[test]
fn the_config_file_turns_it_on() {
    let root = repo();
    std::fs::write(root.join(".jscpd.json"), r#"{"changed": true}"#).unwrap();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    let output = Command::new(cpd_bin())
        .current_dir(&root)
        .args(["--min-tokens", "20", "-r", "json", "-o", "out", "src"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!stderr(&output).contains("Unknown"), "{}", stderr(&output));
    assert_eq!(
        clones(&root),
        [("copy.js".to_string(), "lib.js".to_string(), true)]
    );
}

#[test]
fn a_folder_outside_git_is_an_error() {
    let root = repo();
    std::fs::remove_dir_all(root.join(".git")).unwrap();
    let output = run_changed(&root, &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("--changed:")
            && stderr(&output).contains("is not inside a git repository"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn it_does_not_go_with_the_dashboard() {
    let output = Command::new(cpd_bin())
        .args(["--changed", "--dashboard", "."])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("cannot be used with"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_commit_rebuilds_the_baseline() {
    let root = repo();
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    assert!(run_changed(&root, &[]).status.success());
    let before = head(&root);
    git(&root, &["add", "src"]);
    git(&root, &["commit", "-q", "-m", "copy"]);

    // b.js changes, the copy is committed: HEAD has both clones now.
    let edited = format!("{}\nexport const version = 2;\n", function("known"));
    std::fs::write(root.join("src/b.js"), edited).unwrap();
    let output = run_changed(&root, &["--fail-on-new-clones", "0"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let rebuilt = format!(
        "Baseline .jscpd-baseline.json rebuilt for HEAD {} (was {before}): 2 fingerprints",
        head(&root)
    );
    assert!(stderr(&output).contains(&rebuilt), "{}", stderr(&output));
    assert_eq!(
        clones(&root),
        [("a.js".to_string(), "b.js".to_string(), false)]
    );

    // The next run reads it.
    let output = run_changed(&root, &[]);
    assert!(!stderr(&output).contains("Baseline"), "{}", stderr(&output));
}

#[test]
fn changed_only_compares_the_changed_files_with_one_another() {
    let root = repo();
    // Both copy the function of the unchanged lib.js.
    std::fs::write(root.join("src/copy.js"), function("helper")).unwrap();
    std::fs::write(root.join("src/again.js"), function("helper")).unwrap();

    let output = run_flag(&root, "--changed-only", &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        clones(&root),
        [("again.js".to_string(), "copy.js".to_string(), true)]
    );
    assert_eq!(files_analyzed(&root), 2);
    // The baseline still has the clones of every file of HEAD.
    assert_eq!(fingerprints(&root.join(".jscpd-baseline.json")), 1);
}

#[test]
fn changed_only_on_a_clean_tree_scans_every_file_for_the_baseline() {
    let root = repo();
    let output = run_flag(&root, "--changed-only", &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(clones(&root).is_empty());
    assert_eq!(files_analyzed(&root), 0);
    assert_eq!(fingerprints(&root.join(".jscpd-baseline.json")), 1);
}

#[test]
fn changed_only_does_not_update_the_baseline() {
    let root = repo();
    let output = run_flag(&root, "--changed-only", &["--update-baseline"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("cannot be used with"),
        "{}",
        stderr(&output)
    );
}
