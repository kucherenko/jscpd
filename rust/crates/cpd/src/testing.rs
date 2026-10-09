//! What the unit tests of this crate share.

use std::path::{Path, PathBuf};

/// A fresh folder holding `files` (path and content), by its canonical path,
/// so the paths a scan gives are the folder's; unique across the tests that
/// run in parallel.
pub fn project(files: &[(&str, &str)]) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "jscpd-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::canonicalize(dir).unwrap()
}

/// Run git in `dir` with an identity that works on any machine; panics
/// unless it succeeds, and gives its output.
pub fn git_ok(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}
