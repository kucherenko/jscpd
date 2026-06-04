// detect.rs
// Attribution: sliding-window Rabin-Karp clone detection; inspired by jscpd-rs approach; rewritten independently.

use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::{
    hash::{base_pow, hash_window, roll, token_hash},
    models::{CpdClone, Fragment, SourceFile, TokenKind},
    store::{MemoryStore, SourceRef, Store},
};

/// Detect duplicate code clones across `files` using a rolling-hash sliding window.
///
/// Files are grouped by format; each format group is processed independently using
/// its own in-memory store so tokens from different languages never cross-match.
/// Rayon is used for outer parallelism (one task per format group).
pub fn detect(files: &[SourceFile], min_tokens: usize, _store: &mut dyn Store) -> Vec<CpdClone> {
    if files.is_empty() || min_tokens == 0 {
        return vec![];
    }

    // Group files by format using FxHashMap for speed.
    let mut by_format: FxHashMap<&str, Vec<&SourceFile>> = FxHashMap::default();
    for file in files {
        by_format.entry(file.format.as_str()).or_default().push(file);
    }

    // Process each format group in parallel; each group owns its own MemoryStore.
    let all_clones: Vec<Vec<CpdClone>> = by_format
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

fn detect_in_group(
    files: &[&SourceFile],
    min_tokens: usize,
    store: &mut dyn Store,
) -> Vec<CpdClone> {
    // Pre-filter tokens (remove Ignore) and compute per-token hashes for every file.
    // We keep this as a Vec indexed by file position so make_clone can look up
    // tokens without re-scanning.
    let prepared: Vec<(&SourceFile, Vec<u64>, Vec<&crate::models::Token>)> = files
        .iter()
        .map(|&file| {
            let tokens: Vec<&crate::models::Token> =
                file.tokens.iter().filter(|t| t.kind != TokenKind::Ignore).collect();
            let hashes: Vec<u64> = tokens
                .iter()
                .map(|t| token_hash(t.kind.discriminant(), &t.value))
                .collect();
            (file, hashes, tokens)
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

    for (file_idx, (file, hashes, tokens)) in prepared.iter().enumerate() {
        if tokens.len() < min_tokens {
            continue;
        }

        // Compute initial window hash for the first window.
        let mut window_hash = hash_window(&hashes[..min_tokens]);

        for i in 0..=(hashes.len() - min_tokens) {
            if i > 0 {
                window_hash = roll(window_hash, hashes[i - 1], hashes[i + min_tokens - 1], window_power);
            }

            let current_ref = SourceRef {
                source_id: file.id.clone(),
                token_index: i,
            };

            if let Some(existing) = store.get(window_hash) {
                // Guard against trivially matching the same window location.
                if existing.source_id != file.id || existing.token_index != i {
                    // Find the existing file's prepared data.
                    if let Some(existing_clone) = build_clone(
                        existing,
                        &current_ref,
                        &prepared,
                        file_idx,
                        tokens,
                        hashes,
                        min_tokens,
                        &file.format,
                    ) {
                        clones.push(existing_clone);
                    }
                }
            }
            // Always overwrite so we keep the most recent reference.
            store.set(window_hash, current_ref);
        }
    }

    clones
}

/// Build a `CpdClone` by locating the existing fragment's file data, extending
/// greedily, and constructing Fragments with correct positions.
#[allow(clippy::too_many_arguments)]
fn build_clone(
    existing: &SourceRef,
    current: &SourceRef,
    prepared: &[(&SourceFile, Vec<u64>, Vec<&crate::models::Token>)],
    current_file_idx: usize,
    current_tokens: &[&crate::models::Token],
    current_hashes: &[u64],
    min_tokens: usize,
    format: &str,
) -> Option<CpdClone> {
    // Find the existing file in prepared by source_id.
    let existing_file_idx = prepared.iter().position(|(f, _, _)| f.id == existing.source_id)?;
    let (_, existing_hashes, existing_tokens) = &prepared[existing_file_idx];

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
    if existing_file_idx == current_file_idx {
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
        &existing.source_id,
        existing_tokens,
        ex_start,
        ex_end,
    )?;
    let fragment_b = make_fragment(
        &current.source_id,
        current_tokens,
        cur_start,
        cur_end,
    )?;

    Some(CpdClone {
        format: format.to_string(),
        fragment_a,
        fragment_b,
        token_count: match_len as u32,
    })
}

fn make_fragment(
    source_id: &str,
    tokens: &[&crate::models::Token],
    start_idx: usize,
    end_idx: usize,
) -> Option<Fragment> {
    let first = tokens.get(start_idx)?;
    let last = tokens.get(end_idx)?;
    Some(Fragment {
        source_id: source_id.to_string(),
        start: first.start.clone(),
        end: last.end.clone(),
        range: [start_idx as u32, end_idx as u32],
        blame: None,
    })
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
    use crate::store::MemoryStore;

    fn loc(line: u32, col: u32, offset: u32) -> Location {
        Location { line, column: col, offset }
    }

    fn make_token(kind: TokenKind, value: &str, line: u32, col: u32, offset: u32) -> Token {
        let end_col = col + value.len() as u32;
        let end_off = offset + value.len() as u32;
        Token {
            kind,
            value: value.to_string(),
            format: "javascript".to_string(),
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
        let mut store = MemoryStore::new();
        let result = detect(&[], 10, &mut store);
        assert!(result.is_empty());
    }

    #[test]
    fn identical_files_detected_as_clone() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let mut store = MemoryStore::new();
        let clones = detect(&[file_a, file_b], 5, &mut store);
        assert!(!clones.is_empty(), "identical files must produce at least one clone");
    }

    #[test]
    fn min_tokens_threshold_respected() {
        let tokens = js_tokens_ab(); // 9 tokens
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let mut store = MemoryStore::new();
        let clones = detect(&[file_a, file_b], 100, &mut store);
        assert!(clones.is_empty(), "no clones when min_tokens exceeds file length");
    }

    #[test]
    fn deduplication_ab_ba_collapse() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let mut store = MemoryStore::new();
        let clones = detect(&[file_a, file_b], 5, &mut store);
        // Should get exactly 1 clone pair, not 2 symmetric ones.
        assert_eq!(clones.len(), 1, "symmetric pairs must collapse to 1");
    }

    #[test]
    fn different_formats_not_cross_detected() {
        let tokens = js_tokens_ab();
        let file_js = make_file("a.js", "javascript", tokens.clone());
        let mut py_tokens = tokens;
        for t in &mut py_tokens {
            t.format = "python".to_string();
        }
        let file_py = make_file("a.py", "python", py_tokens);
        let mut store = MemoryStore::new();
        let clones = detect(&[file_js, file_py], 5, &mut store);
        assert!(clones.is_empty(), "tokens from different formats must not match");
    }

    #[test]
    fn subclones_suppressed_keeping_only_maximal() {
        // Two identical files with 9 tokens each; min_tokens=5 would produce sliding-window
        // sub-clones without suppression. After suppression only the maximal clone survives.
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let mut store = MemoryStore::new();
        let clones = detect(&[file_a, file_b], 5, &mut store);
        // Must be exactly 1 clone (the maximal), not multiple sliding-window sub-clones.
        assert_eq!(clones.len(), 1, "only maximal non-contained clone must survive");
        assert_eq!(clones[0].token_count, 9, "maximal clone must cover all 9 tokens");
    }
}
