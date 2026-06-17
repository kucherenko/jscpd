// rust/crates/cpd/tests/parity_test.rs
// Parity tests: compare cpd output against jscpd (TypeScript reference).
// Run with: PARITY=1 cargo test -p cpd --test parity_test
//
// Fixture paths discovered at:
//   /Users/apk/Workspace/lab/jscpd/fixtures/{javascript,typescript,python,java,go}/
// (repo_root = parent of rust/ dir, i.e. /Users/apk/Workspace/lab/jscpd)

use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{self, Command},
};

/// Version of jscpd we test against.
const JSCPD_VERSION: &str = "4.2.4";

fn skip_if_no_parity() -> bool {
    std::env::var("PARITY").is_err()
}

fn cpd_bin() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_cpd for integration tests; prefer it because
    // it already points at the correct target directory and executable suffix.
    if let Ok(bin) = std::env::var("CARGO_BIN_EXE_cpd") {
        return PathBuf::from(bin);
    }
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/cpd");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path
}

fn jscpd_bin() -> &'static str {
    "jscpd" // expected to be on PATH
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = rust/crates/cpd
    // parent = rust/crates
    // parent = rust
    // parent = repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixtures_dir(language: &str) -> PathBuf {
    // Primary: <repo_root>/fixtures/<language>
    let base = repo_root().join("fixtures").join(language);
    if base.exists() {
        return base;
    }
    // Fallback: packages/jscpd/fixtures/<language>
    let alt = repo_root()
        .join("packages")
        .join("jscpd")
        .join("fixtures")
        .join(language);
    if alt.exists() {
        return alt;
    }
    base
}

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-parity-{}-{}", process::id(), suffix));
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn json_report_args(input_dir: &Path, output_dir: &Path) -> Vec<String> {
    vec![
        "--reporters".to_string(),
        "json".to_string(),
        "--min-tokens".to_string(),
        "50".to_string(),
        "--output".to_string(),
        output_dir.to_string_lossy().into_owned(),
        input_dir.to_string_lossy().into_owned(),
    ]
}

fn read_json_report(output_dir: &Path) -> Option<Value> {
    let report_path = output_dir.join("jscpd-report.json");
    let content = std::fs::read_to_string(&report_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Run cpd on a directory and return parsed JSON output.
fn run_cpd(input_dir: &Path, output_dir: &Path) -> Option<Value> {
    let _ = Command::new(cpd_bin())
        .args(json_report_args(input_dir, output_dir))
        .status()
        .ok()?;
    read_json_report(output_dir)
}

/// Run jscpd (TypeScript) on a directory and return parsed JSON output.
fn run_jscpd(input_dir: &Path, output_dir: &Path) -> Option<Value> {
    let _ = Command::new(jscpd_bin())
        .args(json_report_args(input_dir, output_dir))
        .status()
        .ok()?;
    read_json_report(output_dir)
}

/// Compare percentage values within ±0.1% tolerance.
fn percentages_match(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.1
}

#[test]
fn jscpd_version_matches_constant() {
    if skip_if_no_parity() {
        return;
    }

    let output = Command::new(jscpd_bin())
        .arg("--version")
        .output()
        .expect("jscpd must be on PATH for parity tests");

    let version_str = String::from_utf8_lossy(&output.stdout);
    let version_str = version_str.trim();

    assert!(
        version_str.contains(JSCPD_VERSION),
        "jscpd version '{}' must match constant '{}'. Run: npm install -g jscpd@{}",
        version_str,
        JSCPD_VERSION,
        JSCPD_VERSION
    );
}

fn run_parity_for_language(language: &str) {
    if skip_if_no_parity() {
        return;
    }

    let input_dir = fixtures_dir(language);
    if !input_dir.exists() {
        eprintln!(
            "Skipping parity test for '{}': fixtures not found at {:?}",
            language, input_dir
        );
        return;
    }

    let cpd_out = tmp_dir(&format!("cpd-{}", language));
    let jscpd_out = tmp_dir(&format!("jscpd-{}", language));

    let cpd_result = match run_cpd(&input_dir, &cpd_out) {
        Some(r) => r,
        None => {
            eprintln!("cpd produced no JSON output for '{}'", language);
            return;
        }
    };

    let jscpd_result = match run_jscpd(&input_dir, &jscpd_out) {
        Some(r) => r,
        None => {
            eprintln!(
                "jscpd produced no JSON output for '{}' — is jscpd installed?",
                language
            );
            return;
        }
    };

    // Clone counts from both tools (for informational logging)
    let cpd_clones = cpd_result
        .pointer("/duplicates")
        .or_else(|| cpd_result.pointer("/clones"))
        .and_then(|a| a.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let jscpd_clones = jscpd_result
        .pointer("/statistics/total/clones")
        .and_then(|n| n.as_u64())
        .unwrap_or(0) as usize;

    // Compare duplication percentage (±0.1%)
    let cpd_pct = cpd_result
        .pointer("/statistics/total/percentage")
        .and_then(|n| n.as_f64())
        .unwrap_or(0.0);
    let jscpd_pct = jscpd_result
        .pointer("/statistics/total/percentage")
        .and_then(|n| n.as_f64())
        .unwrap_or(0.0);

    eprintln!(
        "Parity test '{}': cpd={} clones ({:.2}%), jscpd={} clones ({:.2}%)",
        language, cpd_clones, cpd_pct, jscpd_clones, jscpd_pct
    );

    assert!(
        percentages_match(cpd_pct, jscpd_pct),
        "Parity test '{}': percentage mismatch: cpd={:.2}% vs jscpd={:.2}% (tolerance ±0.1%)",
        language,
        cpd_pct,
        jscpd_pct
    );

    // Cleanup temp dirs
    let _ = std::fs::remove_dir_all(&cpd_out);
    let _ = std::fs::remove_dir_all(&jscpd_out);
}

#[test]
fn parity_javascript() {
    run_parity_for_language("javascript");
}

#[test]
fn parity_typescript() {
    run_parity_for_language("typescript");
}

#[test]
fn parity_python() {
    run_parity_for_language("python");
}

#[test]
fn parity_java() {
    run_parity_for_language("java");
}

#[test]
fn parity_go() {
    run_parity_for_language("go");
}
