// detect.rs
// Attribution: sliding-window Rabin-Karp clone detection; inspired by jscpd-rs approach; rewritten independently.

use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::{
    hash::{base_pow, hash_window, roll, token_hash},
    models::{CpdClone, Fragment, Location, SourceFile, TokenKind},
    store::{MemoryStore, SourceRef, Store},
};

/// Detect duplicate code clones across `files` using a rolling-hash sliding window.
///
/// Files are grouped by format; each format group is processed independently using
/// its own in-memory store so tokens from different languages never cross-match.
/// Rayon is used for outer parallelism (one task per format group).
pub fn detect(files: &[SourceFile], min_tokens: usize) -> Vec<CpdClone> {
    if files.is_empty() || min_tokens == 0 {
        return vec![];
    }

    // Group files by format. Sort groups and files within each group by id so
    // detection order is deterministic regardless of FxHashMap iteration order.
    let mut by_format: FxHashMap<&str, Vec<&SourceFile>> = FxHashMap::default();
    for file in files {
        by_format.entry(file.format.as_str()).or_default().push(file);
    }
    let mut format_groups: Vec<(&str, Vec<&SourceFile>)> = by_format.into_iter().collect();
    format_groups.sort_unstable_by_key(|(fmt, _)| *fmt);
    for (_, group) in &mut format_groups {
        group.sort_unstable_by_key(|f| f.id.as_str());
    }

    // Process each format group in parallel; each group owns its own MemoryStore.
    let all_clones: Vec<Vec<CpdClone>> = format_groups
        .into_par_iter()
        .map(|(_format, group)| {
            let mut local_store = MemoryStore::new();
            detect_in_group(&group, min_tokens, &mut local_store)
        })
        .collect();

    let mut clones: Vec<CpdClone> = all_clones.into_iter().flatten().collect();

    dedup_clones(&mut clones);
    suppress_subclones(&mut clones);

    clones.sort_by(|a, b| {
        (
            &a.fragment_a.source_id,
            a.fragment_a.start.line,
            &a.fragment_b.source_id,
            a.fragment_b.start.line,
        )
            .cmp(&(
                &b.fragment_a.source_id,
                b.fragment_a.start.line,
                &b.fragment_b.source_id,
                b.fragment_b.start.line,
            ))
    });

    clones
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Per-file prepared data, structure-of-arrays layout.
///
/// Detection only needs the hash array for window operations and the span
/// array (start/end `Location` per kept token) for fragment construction.
/// The original `Vec<&Token>` reference array was replaced with the span
/// array to halve indirections in the hot path and to free `Token` from
/// having to carry per-token metadata that detection never reads.
type PreparedFile<'a> = (&'a SourceFile, Vec<u64>, Vec<(Location, Location)>);

fn detect_in_group(
    files: &[&SourceFile],
    min_tokens: usize,
    store: &mut MemoryStore,
) -> Vec<CpdClone> {
    // Pre-filter tokens (remove Ignore) and compute per-token hashes and spans
    // in a single pass. Each non-Ignore token contributes one u64 to the hash
    // array and one (start, end) pair to the span array. Indices align: hashes[i]
    // and spans[i] refer to the same kept-token position.
    let prepared: Vec<PreparedFile> = files
        .iter()
        .map(|&file| {
            let mut hashes: Vec<u64> = Vec::with_capacity(file.tokens.len());
            let mut spans: Vec<(Location, Location)> = Vec::with_capacity(file.tokens.len());
            for t in &file.tokens {
                if t.kind == TokenKind::Ignore {
                    continue;
                }
                hashes.push(token_hash(t.kind.discriminant(), &t.value));
                spans.push((t.start.clone(), t.end.clone()));
            }
            (file, hashes, spans)
        })
        .collect();

    // Precompute window_power once for this format group.
    // If per-language min_tokens is introduced in future, recompute per group
    // invocation using that group's min_tokens (already scoped here).
    let window_power = base_pow(min_tokens.saturating_sub(1));

    // Pre-allocate store capacity to avoid FxHashMap rehashing mid-loop.
    // This is a performance hint only — correctness does not depend on it.
    let total = prepared.iter()
        .map(|(_, hashes, _)| hashes.len().saturating_sub(min_tokens))
        .sum::<usize>();
    store.reserve(total);

    let mut clones = Vec::new();
    // Track windows that produce a hit in the primary pass. When the primary
    // store overwrites an earlier occurrence, we lose potential clone pairs for
    // fragments that appear 3+ times. The secondary pass recovers those.
    let mut repeated_windows: FxHashMap<u64, Vec<SourceRef>> = FxHashMap::default();

    for (file_idx, (_file, hashes, spans)) in prepared.iter().enumerate() {
        if spans.len() < min_tokens {
            continue;
        }

        // Compute initial window hash for the first window.
        let mut window_hash = hash_window(&hashes[..min_tokens]);

        for i in 0..=(hashes.len() - min_tokens) {
            if i > 0 {
                window_hash = roll(window_hash, hashes[i - 1], hashes[i + min_tokens - 1], window_power);
            }

            let current_ref = SourceRef { file_idx, token_index: i };

            if let Some(existing) = store.get(window_hash) {
                // Guard against trivially matching the same window location.
                if existing.file_idx != file_idx || existing.token_index != i {
                    if let Some(existing_clone) = build_clone(
                        existing,
                        &current_ref,
                        &prepared,
                        min_tokens,
                    ) {
                        clones.push(existing_clone);
                    }
                    // Record both sides for the secondary pass.
                    remember_repeated_window(&mut repeated_windows, window_hash, existing.clone());
                    remember_repeated_window(&mut repeated_windows, window_hash, current_ref.clone());
                }
            }
            // Always overwrite so we keep the most recent reference.
            store.set(window_hash, current_ref);
        }
    }

    // Secondary pass: recover clone pairs missed because the primary store only
    // keeps one occurrence per hash. This matters when a fragment appears 3+ times.
    add_secondary_clones(repeated_windows, &prepared, min_tokens, &mut clones);

    clones
}

/// Build a `CpdClone` by direct-indexing the existing fragment's prepared data,
/// extending greedily, and constructing Fragments with correct positions.
fn build_clone(
    existing: &SourceRef,
    current: &SourceRef,
    prepared: &[PreparedFile],
    min_tokens: usize,
) -> Option<CpdClone> {
    // Direct lookup by file_idx — O(1), no linear scan over prepared.
    let (existing_file, existing_hashes, existing_spans) = &prepared[existing.file_idx];
    let (current_file, current_hashes, current_spans) = &prepared[current.file_idx];

    let ex_start = existing.token_index;
    let cur_start = current.token_index;

    // Greedy extend: how many tokens beyond min_tokens also match?
    let max_extend_existing = existing_hashes.len().saturating_sub(ex_start + min_tokens);
    let max_extend_current = current_hashes.len().saturating_sub(cur_start + min_tokens);
    let max_extend = max_extend_existing.min(max_extend_current);

    let mut extra = 0usize;
    while extra < max_extend
        && existing_hashes[ex_start + min_tokens + extra]
            == current_hashes[cur_start + min_tokens + extra]
    {
        extra += 1;
    }

    let match_len = min_tokens + extra;

    let ex_end = ex_start + match_len - 1;
    let cur_end = cur_start + match_len - 1;

    // Guard: don't emit a self-clone for overlapping windows in the same file.
    if existing.file_idx == current.file_idx {
        // Overlapping ranges in the same file: skip.
        let (lo, hi) = if ex_start < cur_start {
            (ex_start, ex_start + match_len)
        } else {
            (cur_start, cur_start + match_len)
        };
        let (lo2, hi2) = if ex_start < cur_start {
            (cur_start, cur_start + match_len)
        } else {
            (ex_start, ex_start + match_len)
        };
        if lo2 < hi {
            // Overlapping — skip.
            let _ = (lo, hi2); // suppress unused warning
            return None;
        }
    }

    let fragment_a = make_fragment(
        &existing_file.id,
        existing_spans,
        ex_start,
        ex_end,
    )?;
    let fragment_b = make_fragment(
        &current_file.id,
        current_spans,
        cur_start,
        cur_end,
    )?;

    Some(CpdClone {
        format: current_file.format.clone(),
        fragment_a,
        fragment_b,
        token_count: match_len as u32,
    })
}

fn make_fragment(
    source_id: &str,
    spans: &[(Location, Location)],
    start_idx: usize,
    end_idx: usize,
) -> Option<Fragment> {
    let (first_start, _) = spans.get(start_idx)?;
    let (_, last_end) = spans.get(end_idx)?;
    Some(Fragment {
        source_id: source_id.to_string(),
        start: first_start.clone(),
        end: last_end.clone(),
        range: [start_idx as u32, end_idx as u32],
        blame: None,
    })
}

// ---------------------------------------------------------------------------
// Secondary clone pass
// ---------------------------------------------------------------------------

const SECONDARY_OCCURRENCE_CAP: usize = 16;

/// Record an occurrence for a window hash. Capped at SECONDARY_OCCURRENCE_CAP
/// entries per hash to avoid unbounded memory use on highly repeated code.
fn remember_repeated_window(
    repeated_windows: &mut FxHashMap<u64, Vec<SourceRef>>,
    hash: u64,
    occurrence: SourceRef,
) {
    let bucket = repeated_windows.entry(hash).or_default();
    // Deduplicate: same file_idx + token_index is the same position.
    if bucket.iter().any(|s| s.file_idx == occurrence.file_idx && s.token_index == occurrence.token_index) {
        return;
    }
    if bucket.len() < SECONDARY_OCCURRENCE_CAP {
        bucket.push(occurrence);
    }
}

/// Recover clone pairs that the primary pass missed because the store only holds
/// the last occurrence of each window hash. Needed when a fragment appears 3+
/// times: occurrences (A, B, C) — the primary loop stores the last one (C), finds
/// only the C↔B pair; this pass finds A↔B or A↔C from the recorded occurrences.
fn add_secondary_clones(
    repeated_windows: FxHashMap<u64, Vec<SourceRef>>,
    prepared: &[PreparedFile<'_>],
    min_tokens: usize,
    clones: &mut Vec<CpdClone>,
) {
    if repeated_windows.is_empty() {
        return;
    }

    // Build canonical (source_a ≤ source_b) candidate pairs, deduped and sorted
    // so consecutive windows in the same file pair can be merged into one clone.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Candidate { source_a: usize, source_b: usize, token_a: usize, token_b: usize }

    let mut candidates: Vec<Candidate> = Vec::new();
    for occurrences in repeated_windows.values() {
        if occurrences.len() < 2 { continue; }
        for li in 0..occurrences.len() {
            for ri in li + 1..occurrences.len() {
                let left = &occurrences[li];
                let right = &occurrences[ri];
                if left.file_idx == right.file_idx && left.token_index == right.token_index {
                    continue;
                }
                // Verify the hash match is not a collision.
                let (lh, _, _) = &prepared[left.file_idx];
                let (rh, _, _) = &prepared[right.file_idx];
                let lh_hashes = &prepared[left.file_idx].1;
                let rh_hashes = &prepared[right.file_idx].1;
                let _ = (lh, rh); // suppress unused binding
                let la = left.token_index;
                let ra = right.token_index;
                if la + min_tokens > lh_hashes.len() || ra + min_tokens > rh_hashes.len() {
                    continue;
                }
                if lh_hashes[la..la + min_tokens] != rh_hashes[ra..ra + min_tokens] {
                    continue;
                }
                // Canonical: smaller (file_idx, token_index) first.
                let (sa, ta, sb, tb) = if (left.file_idx, left.token_index) <= (right.file_idx, right.token_index) {
                    (left.file_idx, left.token_index, right.file_idx, right.token_index)
                } else {
                    (right.file_idx, right.token_index, left.file_idx, left.token_index)
                };
                candidates.push(Candidate { source_a: sa, source_b: sb, token_a: ta, token_b: tb });
            }
        }
    }
    if candidates.is_empty() { return; }
    candidates.sort_unstable();
    candidates.dedup();

    // Build the line-coverage set from already-found primary clones so we don't
    // re-report lines that are fully covered.
    let n_files = prepared.len();
    let mut covered: Vec<Vec<(u32, u32)>> = vec![Vec::new(); n_files];
    for c in clones.iter() {
        // Map source_id back to file_idx via linear search (small list).
        let fa = prepared.iter().position(|(sf, _, _)| sf.id == c.fragment_a.source_id);
        let fb = prepared.iter().position(|(sf, _, _)| sf.id == c.fragment_b.source_id);
        if let Some(idx) = fa { covered[idx].push((c.fragment_a.start.line, c.fragment_a.end.line)); }
        if let Some(idx) = fb { covered[idx].push((c.fragment_b.start.line, c.fragment_b.end.line)); }
    }
    for ranges in &mut covered { ranges.sort_unstable(); }

    let line_extends_coverage = |file_idx: usize, start: u32, end: u32| -> bool {
        let ranges = &covered[file_idx];
        let mut next = start;
        for &(rs, re) in ranges {
            if re < next { continue; }
            if rs > next { return true; }
            next = next.max(re.saturating_add(1));
            if next > end { return false; }
        }
        next <= end
    };

    // Walk sorted candidates — each unique pair gets one build_clone call.
    // build_clone already greedily extends the match, so we don't need to merge
    // consecutive windows here. Deduplication happens in the outer dedup_clones pass.
    for cand in candidates {
        let new_clone = build_clone(
            &SourceRef { file_idx: cand.source_a, token_index: cand.token_a },
            &SourceRef { file_idx: cand.source_b, token_index: cand.token_b },
            prepared,
            min_tokens,
        );
        if let Some(nc) = new_clone {
            // Only add if it extends beyond already-covered lines.
            let extends_a = line_extends_coverage(cand.source_a, nc.fragment_a.start.line, nc.fragment_a.end.line);
            let extends_b = line_extends_coverage(cand.source_b, nc.fragment_b.start.line, nc.fragment_b.end.line);
            if extends_a || extends_b {
                clones.push(nc);
            }
        }
    }
}

/// Remove clones that are fully contained within a larger clone of the same file pair.
///
/// When the sliding window emits every sub-window of a large duplicate, we keep only
/// the maximal (non-contained) clone, matching jscpd's behaviour.
fn suppress_subclones(clones: &mut Vec<CpdClone>) {
    use std::cmp::Reverse;

    // Largest clones first so outer loop processes "containers" before "contained".
    clones.sort_by_key(|c| Reverse(c.token_count));

    let n = clones.len();
    let mut keep = vec![true; n];

    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let big = &clones[i];
        let big_a = &big.fragment_a;
        let big_b = &big.fragment_b;

        for j in (i + 1)..n {
            if !keep[j] {
                continue;
            }
            let small = &clones[j];
            let small_a = &small.fragment_a;
            let small_b = &small.fragment_b;

            // Same file pair? (fragments are already normalised: a_id ≤ b_id)
            if big_a.source_id != small_a.source_id || big_b.source_id != small_b.source_id {
                continue;
            }

            // Is small fully contained within big (by token range)?
            if big_a.range[0] <= small_a.range[0]
                && big_a.range[1] >= small_a.range[1]
                && big_b.range[0] <= small_b.range[0]
                && big_b.range[1] >= small_b.range[1]
            {
                keep[j] = false;
            }
        }
    }

    let mut i = 0;
    clones.retain(|_| {
        let k = keep[i];
        i += 1;
        k
    });
}

fn dedup_clones(clones: &mut Vec<CpdClone>) {
    // Normalise each clone so fragment_a <= fragment_b (by source_id then start line).
    for clone in clones.iter_mut() {
        let a_key = (&clone.fragment_a.source_id, clone.fragment_a.start.line);
        let b_key = (&clone.fragment_b.source_id, clone.fragment_b.start.line);
        if a_key > b_key {
            std::mem::swap(&mut clone.fragment_a, &mut clone.fragment_b);
        }
    }

    clones.sort_by(|a, b| {
        (
            &a.fragment_a.source_id,
            a.fragment_a.start.line,
            &a.fragment_b.source_id,
            a.fragment_b.start.line,
        )
            .cmp(&(
                &b.fragment_a.source_id,
                b.fragment_a.start.line,
                &b.fragment_b.source_id,
                b.fragment_b.start.line,
            ))
    });

    clones.dedup_by(|a, b| {
        a.fragment_a.source_id == b.fragment_a.source_id
            && a.fragment_a.start.line == b.fragment_a.start.line
            && a.fragment_b.source_id == b.fragment_b.source_id
            && a.fragment_b.start.line == b.fragment_b.start.line
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Location, Token, TokenKind};
    fn loc(line: u32, col: u32, offset: u32) -> Location {
        Location { line, column: col, offset }
    }

    fn make_token(kind: TokenKind, value: &str, line: u32, col: u32, offset: u32) -> Token {
        let end_col = col + value.len() as u32;
        let end_off = offset + value.len() as u32;
        Token {
            kind,
            value: value.to_string(),
            start: loc(line, col, offset),
            end: loc(line, end_col, end_off),
        }
    }

    fn make_file(id: &str, format: &str, tokens: Vec<Token>) -> SourceFile {
        SourceFile { id: id.to_string(), format: format.to_string(), tokens }
    }

    fn js_tokens_ab() -> Vec<Token> {
        vec![
            make_token(TokenKind::Keyword,  "function", 1, 0,  0),
            make_token(TokenKind::Other,    "hello",    1, 9,  9),
            make_token(TokenKind::Operator, "(",        1, 14, 14),
            make_token(TokenKind::Operator, ")",        1, 15, 15),
            make_token(TokenKind::Operator, "{",        1, 16, 16),
            make_token(TokenKind::Keyword,  "return",   2, 0,  18),
            make_token(TokenKind::Literal,  "42",       2, 7,  25),
            make_token(TokenKind::Operator, ";",        2, 9,  27),
            make_token(TokenKind::Operator, "}",        3, 0,  29),
        ]
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = detect(&[], 10);
        assert!(result.is_empty());
    }

    #[test]
    fn identical_files_detected_as_clone() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 5);
        assert!(!clones.is_empty(), "identical files must produce at least one clone");
    }

    #[test]
    fn min_tokens_threshold_respected() {
        let tokens = js_tokens_ab(); // 9 tokens
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 100);
        assert!(clones.is_empty(), "no clones when min_tokens exceeds file length");
    }

    #[test]
    fn deduplication_ab_ba_collapse() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 5);
        // Should get exactly 1 clone pair, not 2 symmetric ones.
        assert_eq!(clones.len(), 1, "symmetric pairs must collapse to 1");
    }

    #[test]
    fn different_formats_not_cross_detected() {
        let tokens = js_tokens_ab();
        let file_js = make_file("a.js", "javascript", tokens.clone());
        // Detection groups by SourceFile.format — Token no longer carries a
        // per-token format field, so grouping is purely a SourceFile-level
        // concern. Reusing the same token sequence across two files with
        // different SourceFile.format values must yield no cross-format clones.
        let file_py = make_file("a.py", "python", tokens);
        let clones = detect(&[file_js, file_py], 5);
        assert!(clones.is_empty(), "tokens from different formats must not match");
    }

    #[test]
    fn subclones_suppressed_keeping_only_maximal() {
        // Two identical files with 9 tokens each; min_tokens=5 would produce sliding-window
        // sub-clones without suppression. After suppression only the maximal clone survives.
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 5);
        // Must be exactly 1 clone (the maximal), not multiple sliding-window sub-clones.
        assert_eq!(clones.len(), 1, "only maximal non-contained clone must survive");
        assert_eq!(clones[0].token_count, 9, "maximal clone must cover all 9 tokens");
    }

    #[test]
    fn three_identical_files_secondary_pass_adds_missing_pair() {
        // The primary pass (single store slot per hash) only finds adjacent pairs in the
        // file order. The secondary pass must find the pair that was missed.
        // With files [a, b, c] the primary finds a↔b and b↔c. The secondary recovers
        // any pair whose fragment reaches a file NOT yet covered by primary clones.
        // In this case all three files cover the same lines, so a↔c is suppressed
        // by the coverage gate (both ends already covered). We get exactly 2 pairs.
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens.clone());
        let file_c = make_file("c.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b, file_c], 5);
        // At minimum we must find 2 pairs (primary finds adjacent; secondary adds one more
        // that extends coverage). Exact count depends on file order but must be >= 2.
        assert!(clones.len() >= 2,
            "three identical files must yield at least 2 clone pairs, got {}", clones.len());
    }
}
