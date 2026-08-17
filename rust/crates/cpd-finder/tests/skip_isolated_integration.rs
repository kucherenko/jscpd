use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn setup_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-skip-isolated-{}", suffix));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn duplicate_js() -> &'static str {
    r#"function isDuplicate(a, b, c, d, e) {
    const result = a + b + c;
    if (result > d) {
        return result * e;
    }
    return result;
}

function anotherFunc(x, y) {
    return x + y;
}
"#
}

fn config(paths: Vec<PathBuf>, skip_isolated: Vec<Vec<PathBuf>>) -> RunConfig {
    RunConfig {
        paths,
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        skip_isolated,
        ..Default::default()
    }
}

fn write_pair(dir_a: &Path, dir_b: &Path) {
    fs::create_dir_all(dir_a).unwrap();
    fs::create_dir_all(dir_b).unwrap();
    fs::write(dir_a.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir_b.join("file_b.js"), duplicate_js()).unwrap();
}

#[test]
fn clones_across_isolated_folders_are_skipped() {
    // Group folders are passed uncanonicalized (env::temp_dir() is symlinked
    // on macOS) while file ids are canonicalized — run() must bridge the two.
    let dir = setup_temp_dir("cross-group");
    let dir_a = dir.join("packages/business_a");
    let dir_b = dir.join("packages/business_b");
    write_pair(&dir_a, &dir_b);

    let result = run(&config(
        vec![dir.clone()],
        vec![vec![dir_a.clone(), dir_b.clone()]],
    ))
    .unwrap();
    assert!(
        result.clones.is_empty(),
        "clones across two folders of one isolation group must be skipped"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clones_inside_one_isolated_folder_survive() {
    let dir = setup_temp_dir("same-folder");
    let dir_a = dir.join("packages/business_a");
    let dir_b = dir.join("packages/business_b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    fs::write(dir_a.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir_a.join("file_b.js"), duplicate_js()).unwrap();

    let result = run(&config(
        vec![dir.clone()],
        vec![vec![dir_a.clone(), dir_b.clone()]],
    ))
    .unwrap();
    assert!(
        !result.clones.is_empty(),
        "clones inside a single isolated folder must be kept"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clones_outside_isolation_groups_survive() {
    let dir = setup_temp_dir("outside-group");
    let dir_a = dir.join("packages/business_a");
    let globals = dir.join("globals");
    write_pair(&dir_a, &globals);

    let result = run(&config(
        vec![dir.clone()],
        vec![vec![dir_a.clone(), dir.join("packages/business_b")]],
    ))
    .unwrap();
    assert!(
        !result.clones.is_empty(),
        "clones between an isolated folder and an outside folder must be kept"
    );
    let _ = fs::remove_dir_all(&dir);
}
