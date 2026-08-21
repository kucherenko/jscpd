// baseline.rs — clone-fingerprint baseline (issue #944, phase 1).
//
// A committed baseline file records fingerprints of accepted clones so CI can
// gate on *new* duplication only. A clone's fingerprint is the content hash of
// its two duplicated snippets — the same `snippet_pair_hash` value the SARIF
// reporter emits as `partialFingerprints["jscpdCloneHash/v1"]` — so it is
// stable under line-number shifts, file renames and unrelated edits, while any
// edit inside a duplicated fragment produces a new fingerprint.
//
// Fingerprints carry a multiplicity count: removing one instance of a clone
// and adding an identical one elsewhere keeps the count unchanged, while a
// genuinely new occurrence exceeds the recorded count and is marked new.
//
// File format (version 1): a sorted map that pretty-prints one fingerprint per
// line — merge-friendly and reviewable in PRs.

use crate::shared::fragment_text;
use cpd_core::hash::snippet_pair_hash;
use cpd_core::models::{CpdClone, Statistics};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineFile {
    pub version: u32,
    pub fingerprints: BTreeMap<String, u64>,
}

impl BaselineFile {
    pub fn empty() -> Self {
        Self {
            version: BASELINE_VERSION,
            fingerprints: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum BaselineError {
    /// The baseline file does not exist and `--update-baseline` was not given.
    Missing {
        path: String,
    },
    Io {
        path: String,
        error: std::io::Error,
    },
    Parse {
        path: String,
        error: String,
    },
    UnsupportedVersion {
        path: String,
        version: u32,
    },
}

impl std::fmt::Display for BaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { path } => write!(
                f,
                "baseline file {} not found — run with --update-baseline to create it",
                path
            ),
            Self::Io { path, error } => write!(f, "baseline file {}: {}", path, error),
            Self::Parse { path, error } => write!(f, "baseline file {}: {}", path, error),
            Self::UnsupportedVersion { path, version } => write!(
                f,
                "baseline file {}: unsupported version {} (this cpd supports version {}) — regenerate it with --update-baseline",
                path, version, BASELINE_VERSION
            ),
        }
    }
}

impl std::error::Error for BaselineError {}

/// Counts of fingerprint instances added to / removed from the baseline by
/// `--update-baseline` (with multiplicity), for non-silent regeneration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateSummary {
    pub added: u64,
    pub removed: u64,
    /// Total fingerprint instances in the updated baseline.
    pub total: u64,
}

/// Result of applying a baseline to a detection run.
#[derive(Debug, Clone, Copy)]
pub struct BaselineOutcome {
    /// Number of clones marked new (absent from the baseline).
    pub new_clones: u64,
    /// Present when the baseline file was rewritten (`--update-baseline`).
    pub update: Option<UpdateSummary>,
}

pub fn load(path: &Path) -> Result<BaselineFile, BaselineError> {
    let display = path.display().to_string();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(BaselineError::Missing { path: display });
        }
        Err(e) => {
            return Err(BaselineError::Io {
                path: display,
                error: e,
            });
        }
    };
    let file: BaselineFile = serde_json::from_str(&content).map_err(|e| BaselineError::Parse {
        path: display.clone(),
        error: e.to_string(),
    })?;
    if file.version > BASELINE_VERSION {
        return Err(BaselineError::UnsupportedVersion {
            path: display,
            version: file.version,
        });
    }
    Ok(file)
}

pub fn save(path: &Path, baseline: &BaselineFile) -> Result<(), BaselineError> {
    let display = path.display().to_string();
    // BTreeMap keys serialize sorted; pretty-printing puts one fingerprint per
    // line so baseline diffs stay reviewable and merge conflicts trivial.
    let content = serde_json::to_string_pretty(baseline).map_err(|e| BaselineError::Parse {
        path: display.clone(),
        error: e.to_string(),
    })?;
    std::fs::write(path, content + "\n").map_err(|e| BaselineError::Io {
        path: display,
        error: e,
    })
}

/// Compute one fingerprint per clone, in clone order. Reads fragment snippets
/// from disk with a shared per-call file cache.
pub fn compute_fingerprints(clones: &[CpdClone]) -> Vec<String> {
    let mut file_cache: HashMap<String, String> = HashMap::new();
    clones
        .iter()
        .map(|clone| {
            let snippet_a = fragment_text(&mut file_cache, &clone.fragment_a);
            let snippet_b = fragment_text(&mut file_cache, &clone.fragment_b);
            format!("{:016x}", snippet_pair_hash(&snippet_a, &snippet_b))
        })
        .collect()
}

/// Build a baseline from the fingerprints of the current run.
pub fn build(fingerprints: &[String]) -> BaselineFile {
    let mut map: BTreeMap<String, u64> = BTreeMap::new();
    for fp in fingerprints {
        *map.entry(fp.clone()).or_insert(0) += 1;
    }
    BaselineFile {
        version: BASELINE_VERSION,
        fingerprints: map,
    }
}

/// Mark clones absent from the baseline as new, honoring multiplicity: with N
/// recorded instances of a fingerprint, the first N occurrences (in clone
/// order) stay known and any further occurrence is new. Returns the number of
/// clones marked new.
pub fn mark_new(clones: &mut [CpdClone], fingerprints: &[String], baseline: &BaselineFile) -> u64 {
    debug_assert_eq!(clones.len(), fingerprints.len());
    let mut allowance: HashMap<&str, u64> = baseline
        .fingerprints
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    let mut new_count = 0u64;
    for (clone, fp) in clones.iter_mut().zip(fingerprints) {
        match allowance.get_mut(fp.as_str()) {
            Some(remaining) if *remaining > 0 => *remaining -= 1,
            _ => {
                clone.is_new = true;
                new_count += 1;
            }
        }
    }
    new_count
}

/// Fill `newClones` / `newDuplicatedLines` on the total and per-format stat
/// rows from the `is_new` markers. Line counting mirrors `duplicated_lines`
/// in cpd-finder statistics (fragment A end line minus start line).
pub fn apply_to_stats(clones: &[CpdClone], stats: &mut Statistics) {
    stats.total.new_clones = 0;
    stats.total.new_duplicated_lines = 0;
    for row in stats.formats.values_mut() {
        row.new_clones = 0;
        row.new_duplicated_lines = 0;
    }
    for clone in clones.iter().filter(|c| c.is_new) {
        let lines = clone
            .fragment_a
            .end
            .line
            .saturating_sub(clone.fragment_a.start.line) as u64;
        stats.total.new_clones += 1;
        stats.total.new_duplicated_lines += lines;
        if let Some(row) = stats.formats.get_mut(&clone.format) {
            row.new_clones += 1;
            row.new_duplicated_lines += lines;
        }
    }
}

/// Instance-level added/removed counts between two baselines (with multiplicity).
pub fn diff(old: &BaselineFile, new: &BaselineFile) -> UpdateSummary {
    let mut added = 0u64;
    let mut removed = 0u64;
    for (fp, &new_count) in &new.fingerprints {
        let old_count = old.fingerprints.get(fp).copied().unwrap_or(0);
        added += new_count.saturating_sub(old_count);
    }
    for (fp, &old_count) in &old.fingerprints {
        let new_count = new.fingerprints.get(fp).copied().unwrap_or(0);
        removed += old_count.saturating_sub(new_count);
    }
    UpdateSummary {
        added,
        removed,
        total: new.fingerprints.values().sum(),
    }
}

/// Apply an in-memory baseline to a detection run: mark new clones and fill
/// the new-clone statistics. Used by `--baseline-from-ref`, where the baseline
/// is built from the base ref's tree instead of loaded from a file. Returns
/// the number of clones marked new.
pub fn apply_in_memory(
    clones: &mut [CpdClone],
    stats: &mut Statistics,
    baseline: &BaselineFile,
) -> u64 {
    let fingerprints = compute_fingerprints(clones);
    let new_clones = mark_new(clones, &fingerprints, baseline);
    apply_to_stats(clones, stats);
    new_clones
}

/// Apply the baseline at `path` to a detection run: mark new clones, fill the
/// new-clone statistics, and — when `update` is set — rewrite the baseline
/// from the current run (creating it if missing) and report what changed.
pub fn apply(
    clones: &mut [CpdClone],
    stats: &mut Statistics,
    path: &Path,
    update: bool,
) -> Result<BaselineOutcome, BaselineError> {
    let old = match load(path) {
        Ok(file) => file,
        Err(BaselineError::Missing { .. }) if update => BaselineFile::empty(),
        Err(e) => return Err(e),
    };

    let fingerprints = compute_fingerprints(clones);
    let new_clones = mark_new(clones, &fingerprints, &old);
    apply_to_stats(clones, stats);

    let update_summary = if update {
        let rebuilt = build(&fingerprints);
        save(path, &rebuilt)?;
        Some(diff(&old, &rebuilt))
    } else {
        None
    };

    Ok(BaselineOutcome {
        new_clones,
        update: update_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::fixtures::{make_clone_with_lines, one_clone_stats, tmp_dir};
    use cpd_core::models::CpdClone;

    /// Two files whose lines 1-10 are identical, so a clone spanning them has
    /// real on-disk snippets to fingerprint.
    fn make_real_clone(dir: &std::path::Path, name_a: &str, name_b: &str) -> CpdClone {
        let body: String = (1..=10)
            .map(|i| format!("const value{} = compute({});\n", i, i))
            .collect();
        let path_a = dir.join(name_a);
        let path_b = dir.join(name_b);
        std::fs::write(&path_a, &body).unwrap();
        std::fs::write(&path_b, &body).unwrap();
        make_clone_with_lines(
            path_a.to_str().unwrap(),
            path_b.to_str().unwrap(),
            1,
            10,
            50,
        )
    }

    #[test]
    fn fingerprint_is_stable_across_fragment_order() {
        let dir = tmp_dir("baseline-fp");
        let clone = make_real_clone(&dir, "a.js", "b.js");
        let mut swapped = clone.clone();
        std::mem::swap(&mut swapped.fragment_a, &mut swapped.fragment_b);
        let fps = compute_fingerprints(&[clone, swapped]);
        assert_eq!(fps[0], fps[1], "fragment order must not change fingerprint");
        assert_eq!(fps[0].len(), 16, "fingerprint is 16 hex chars");
    }

    #[test]
    fn mark_new_flags_clones_absent_from_baseline() {
        let dir = tmp_dir("baseline-mark");
        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        let fps = compute_fingerprints(&clones);
        let known = build(&fps);

        let count = mark_new(&mut clones, &fps, &known);
        assert_eq!(count, 0);
        assert!(!clones[0].is_new);

        let count = mark_new(&mut clones, &fps, &BaselineFile::empty());
        assert_eq!(count, 1);
        assert!(clones[0].is_new);
    }

    #[test]
    fn mark_new_honors_multiplicity() {
        let dir = tmp_dir("baseline-mult");
        let clone = make_real_clone(&dir, "a.js", "b.js");
        // Baseline knows ONE instance; the current run has two identical ones.
        let one = compute_fingerprints(std::slice::from_ref(&clone));
        let known = build(&one);

        let mut clones = vec![clone.clone(), clone];
        let fps = compute_fingerprints(&clones);
        let count = mark_new(&mut clones, &fps, &known);
        assert_eq!(count, 1, "second identical instance must be new");
        assert!(!clones[0].is_new);
        assert!(clones[1].is_new);
    }

    #[test]
    fn apply_to_stats_fills_total_and_format_rows() {
        let dir = tmp_dir("baseline-stats");
        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        clones[0].is_new = true;
        let mut stats = one_clone_stats();
        apply_to_stats(&clones, &mut stats);
        assert_eq!(stats.total.new_clones, 1);
        assert_eq!(stats.total.new_duplicated_lines, 9);
        assert_eq!(stats.formats["javascript"].new_clones, 1);
        assert_eq!(stats.formats["javascript"].new_duplicated_lines, 9);
    }

    #[test]
    fn diff_counts_added_and_removed_with_multiplicity() {
        let old = build(&["aa".into(), "aa".into(), "bb".into()]);
        let new = build(&["aa".into(), "cc".into()]);
        let summary = diff(&old, &new);
        assert_eq!(summary.added, 1, "cc is added");
        assert_eq!(summary.removed, 2, "one aa instance and bb are removed");
        assert_eq!(summary.total, 2);
    }

    #[test]
    fn save_load_roundtrip_and_line_per_fingerprint() {
        let dir = tmp_dir("baseline-io");
        let path = dir.join("baseline.json");
        let baseline = build(&["cafe".into(), "beef".into(), "cafe".into()]);
        save(&path, &baseline).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, baseline);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"beef\": 1"));
        assert!(content.contains("\"cafe\": 2"));
        let beef = content.find("\"beef\"").unwrap();
        let cafe = content.find("\"cafe\"").unwrap();
        assert!(beef < cafe, "fingerprints serialize sorted");
        assert!(content.ends_with("}\n"));
    }

    #[test]
    fn load_missing_file_is_missing_error() {
        let dir = tmp_dir("baseline-missing");
        let err = load(&dir.join("nope.json")).unwrap_err();
        assert!(matches!(err, BaselineError::Missing { .. }));
        assert!(err.to_string().contains("--update-baseline"));
    }

    #[test]
    fn load_rejects_unsupported_version() {
        let dir = tmp_dir("baseline-version");
        let path = dir.join("baseline.json");
        std::fs::write(&path, r#"{"version": 99, "fingerprints": {}}"#).unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(
            err,
            BaselineError::UnsupportedVersion { version: 99, .. }
        ));
    }

    #[test]
    fn load_rejects_malformed_json() {
        let dir = tmp_dir("baseline-parse");
        let path = dir.join("baseline.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(matches!(
            load(&path).unwrap_err(),
            BaselineError::Parse { .. }
        ));
    }

    #[test]
    fn apply_update_creates_baseline_and_reports_counts() {
        let dir = tmp_dir("baseline-apply");
        let path = dir.join("baseline.json");
        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        let mut stats = one_clone_stats();

        // First run with --update-baseline: file created, clone is new
        // relative to the (empty) previous state.
        let outcome = apply(&mut clones, &mut stats, &path, true).unwrap();
        assert_eq!(outcome.new_clones, 1);
        let summary = outcome.update.unwrap();
        assert_eq!(summary.added, 1);
        assert_eq!(summary.removed, 0);
        assert_eq!(summary.total, 1);

        // Second run against the recorded baseline: nothing new.
        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        let mut stats = one_clone_stats();
        let outcome = apply(&mut clones, &mut stats, &path, false).unwrap();
        assert_eq!(outcome.new_clones, 0);
        assert!(outcome.update.is_none());
        assert!(!clones[0].is_new);
        assert_eq!(stats.total.new_clones, 0);
    }

    #[test]
    fn apply_in_memory_marks_and_fills_stats() {
        let dir = tmp_dir("baseline-mem");
        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        let mut stats = one_clone_stats();
        let new_count = apply_in_memory(&mut clones, &mut stats, &BaselineFile::empty());
        assert_eq!(new_count, 1);
        assert!(clones[0].is_new);
        assert_eq!(stats.total.new_clones, 1);

        let mut clones = vec![make_real_clone(&dir, "a.js", "b.js")];
        let known = build(&compute_fingerprints(&clones));
        let mut stats = one_clone_stats();
        let new_count = apply_in_memory(&mut clones, &mut stats, &known);
        assert_eq!(new_count, 0);
        assert!(!clones[0].is_new);
        assert_eq!(stats.total.new_clones, 0);
    }

    #[test]
    fn apply_without_update_on_missing_file_errors() {
        let dir = tmp_dir("baseline-apply-missing");
        let mut clones: Vec<CpdClone> = vec![];
        let mut stats = one_clone_stats();
        let err = apply(&mut clones, &mut stats, &dir.join("nope.json"), false).unwrap_err();
        assert!(matches!(err, BaselineError::Missing { .. }));
    }
}
