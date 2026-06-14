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

#[test]
fn same_directory_clones_removed_by_skip_local() {
    let dir = setup_temp_dir("same");

    fs::write(dir.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir.join("file_b.js"), duplicate_js()).unwrap();

    let config = RunConfig {
        paths: vec![dir.clone()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        skip_local: true,
        ..Default::default()
    };

    let result = run(&config).unwrap();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(
        result.clones.len(),
        0,
        "skip_local=true must remove same-directory clones, got: {}",
        result.clones.len()
    );
}

#[test]
fn same_root_subdirectories_skipped_by_skip_local() {
    let dir = setup_temp_dir("same_root_subs");

    let dir_a = dir.join("subdir_a");
    let dir_b = dir.join("subdir_b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();

    fs::write(dir_a.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir_b.join("file_b.js"), duplicate_js()).unwrap();

    // Single scan root with files in different subdirectories.
    // skip_local skips clones where both files share a scan root,
    // even if they're in different subdirectories.
    let config = RunConfig {
        paths: vec![dir.clone()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        skip_local: true,
        ..Default::default()
    };

    let result = run(&config).unwrap();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(
        result.clones.len(),
        0,
        "skip_local=true must skip clones within the same scan root, got: {}",
        result.clones.len()
    );
}

#[test]
fn cross_directory_clones_survive_skip_local() {
    let dir = setup_temp_dir("cross");

    let dir_a = dir.join("subdir_a");
    let dir_b = dir.join("subdir_b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();

    fs::write(dir_a.join("file_a.js"), duplicate_js()).unwrap();
    fs::write(dir_b.join("file_b.js"), duplicate_js()).unwrap();

    // Use BOTH subdirectories as separate scan roots.
    // skip_local skips clones where both files share a scan root.
    // Files in different roots should survive.
    let config = RunConfig {
        paths: vec![dir_a.clone(), dir_b.clone()],
        min_tokens: 5,
        min_lines: 1,
        mode: Mode::Mild,
        skip_local: true,
        ..Default::default()
    };

    let result = run(&config).unwrap();
    let _ = fs::remove_dir_all(&dir);

    assert!(
        !result.clones.is_empty(),
        "skip_local=true must keep cross-directory clones"
    );
}
