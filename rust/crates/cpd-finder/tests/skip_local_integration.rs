use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;
use std::{fs, path::PathBuf};

fn setup_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-skip-local-{}", suffix));
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

fn skip_local_config(paths: Vec<PathBuf>) -> RunConfig {
    RunConfig {
        paths,
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        skip_local: true,
        ..Default::default()
    }
}

fn write_pair(dir_a: &PathBuf, dir_b: &PathBuf) {
    fs::write(dir_a.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir_b.join("file_b.js"), duplicate_js()).unwrap();
}

fn assert_skip_local(paths: Vec<PathBuf>, expected: usize, message: &str) {
    let result = run(&skip_local_config(paths)).unwrap();
    assert_eq!(result.clones.len(), expected, "{}", message);
}

fn setup_subdirs(root_suffix: &str, sub_a: &str, sub_b: &str) -> PathBuf {
    let dir = setup_temp_dir(root_suffix);
    let dir_a = dir.join(sub_a);
    let dir_b = dir.join(sub_b);
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    write_pair(&dir_a, &dir_b);
    dir
}

#[test]
fn same_directory_clones_removed_by_skip_local() {
    let dir = setup_temp_dir("same");
    fs::write(dir.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir.join("file_b.js"), duplicate_js()).unwrap();
    assert_skip_local(
        vec![dir.clone()],
        0,
        "skip_local=true must remove same-directory clones",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn same_root_subdirectories_skipped_by_skip_local() {
    let dir = setup_subdirs("same_root_subs", "subdir_a", "subdir_b");
    assert_skip_local(
        vec![dir.clone()],
        0,
        "skip_local=true must skip clones within the same scan root",
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cross_directory_clones_survive_skip_local() {
    let dir = setup_temp_dir("cross");
    let dir_a = dir.join("subdir_a");
    let dir_b = dir.join("subdir_b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    write_pair(&dir_a, &dir_b);
    let result = run(&skip_local_config(vec![dir_a.clone(), dir_b.clone()])).unwrap();
    assert!(
        !result.clones.is_empty(),
        "skip_local=true must keep cross-directory clones"
    );
    let _ = fs::remove_dir_all(&dir);
}
