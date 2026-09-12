//! Helpers shared by the integration tests in this directory. Each test
//! binary compiles this module on its own, so unused items are expected.
#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

/// Fresh, empty temp directory named `cpd-<prefix>-<suffix>`.
pub fn temp_dir(prefix: &str, suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-{prefix}-{suffix}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A function long enough to clear the default thresholds when copied.
pub fn duplicate_js() -> &'static str {
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

/// `tests/fixtures/<dir>` of this crate.
pub fn fixtures(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{dir}"))
}
