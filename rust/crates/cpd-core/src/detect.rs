// detect.rs
// Attribution: sliding-window Rabin-Karp clone detection; inspired by jscpd-rs approach; rewritten independently.

use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

use crate::{
    hash::{base_pow, hash_window, roll, token_hash},
    models::{CpdClone, DetectionToken, Fragment, Location, SourceFile, TokenKind},
};

// ---------------------------------------------------------------------------
// Internal store type — replaces the Store trait + MemoryStore
// ---------------------------------------------------------------------------

/// Window store: maps a window hash to the last seen occurrence.
/// Type alias — no trait indirection, no vtable, no dyn dispatch.
type WindowStore = FxHashMap<u64, Occurrence>;

/// Lightweight reference to a window position within a format-group detection call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Occurrence {
    /// Index into the `prepared` array for this `detect_in_group` call.
    source_id: usize,
    token_start: usize,
}

// ---------------------------------------------------------------------------
// Deduplication key
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CloneDedupKey {
    a_id: String,
    a_start_line: u32,
    b_id: String,
    b_start_line: u32,
}

impl CloneDedupKey {
    fn from_clone(c: &CpdClone) -> Self {
        // Normalize: smaller (id, line) first so (A,B) and (B,A) map to the same key.
        let a_key = (&c.fragment_a.source_id, c.fragment_a.start.line);
        let b_key = (&c.fragment_b.source_id, c.fragment_b.start.line);
        if a_key <= b_key {
            Self {
                a_id: c.fragment_a.source_id.clone(),
                a_start_line: c.fragment_a.start.line,
                b_id: c.fragment_b.source_id.clone(),
                b_start_line: c.fragment_b.start.line,
            }
        } else {
            Self {
                a_id: c.fragment_b.source_id.clone(),
                a_start_line: c.fragment_b.start.line,
                b_id: c.fragment_a.source_id.clone(),
                b_start_line: c.fragment_a.start.line,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — SourceFile path (for backward compat with tests)
// ---------------------------------------------------------------------------

/// Detect duplicate code clones across `files` using a rolling-hash sliding window.
///
/// Files are grouped by format; each format group is processed independently.
/// Rayon is used for outer parallelism (one task per format group).
pub fn detect(files: &[SourceFile], min_tokens: usize) -> Vec<CpdClone> {
    detect_with_options(files, min_tokens, false, 0)
}

/// Detect clones with extended options.
///
/// - `skip_local`: skip clone pairs where both fragments share the same parent directory.
/// - `min_lines`: reject clones whose fragment line span is shorter than this.
///   The line span is `end.line - start.line`; a clone is kept only if this value
///   is >= `min_lines`. This mirrors jscpd's `LinesLengthCloneValidator`.
pub fn detect_with_options(
    files: &[SourceFile],
    min_tokens: usize,
    skip_local: bool,
    min_lines: usize,
) -> Vec<CpdClone> {
    if files.is_empty() || min_tokens == 0 {
        return vec![];
    }

    // Group files by format. Sort for deterministic order.
    let mut by_format: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    for (idx, file) in files.iter().enumerate() {
        by_format.entry(file.format.as_str()).or_default().push(idx);
    }
    let mut format_groups: Vec<(&str, Vec<usize>)> = by_format.into_iter().collect();
    format_groups.sort_unstable_by_key(|(fmt, _)| *fmt);
    for (_, group) in &mut format_groups {
        group.sort_unstable_by_key(|&idx| files[idx].id.as_str());
    }

    let all_clones: Vec<Vec<CpdClone>> = format_groups
        .into_par_iter()
        .map(|(_format, indices)| {
            // Build per-group prepared data from SourceFile.tokens.
            // This is the backward-compat path; orchestrate.rs uses
            // detect_prepared() directly to avoid re-hashing.
            let prepared: Vec<PreparedSource> = indices
                .iter()
                .map(|&idx| {
                    let file = &files[idx];
                    let mut hashes = Vec::with_capacity(file.tokens.len());
                    let mut spans: Vec<(Location, Location)> =
                        Vec::with_capacity(file.tokens.len());
                    for t in &file.tokens {
                        if t.kind == TokenKind::Ignore {
                            continue;
                        }
                        hashes.push(token_hash(t.kind.discriminant(), &t.value));
                        spans.push((t.start.clone(), t.end.clone()));
                    }
                    PreparedSource {
                        id: file.id.clone(),
                        format: file.format.clone(),
                        hashes,
                        spans,
                    }
                })
                .collect();
            detect_in_group(&prepared, min_tokens, skip_local, min_lines)
        })
        .collect();

    let mut clones: Vec<CpdClone> = all_clones.into_iter().flatten().collect();
    dedup_exact_clones(&mut clones);
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
// Direct DetectionToken path (called by orchestrate.rs)
// ---------------------------------------------------------------------------

/// A file ready for detection: pre-hashed, pre-filtered.
///
/// Produced either from `SourceFile.tokens` (backward compat) or directly from
/// `tokenize_to_detection` output (fast path used by orchestrate.rs).
pub struct PreparedSource {
    pub id: String,
    pub format: String,
    pub hashes: Vec<u64>,
    pub spans: Vec<(Location, Location)>,
}

impl PreparedSource {
    /// Build from a `DetectionToken` slice — the fast path.
    pub fn from_detection_tokens(id: String, format: String, tokens: &[DetectionToken]) -> Self {
        let mut hashes = Vec::with_capacity(tokens.len());
        let mut spans = Vec::with_capacity(tokens.len());
        for t in tokens {
            hashes.push(t.hash);
            spans.push((t.start.clone(), t.end.clone()));
        }
        Self {
            id,
            format,
            hashes,
            spans,
        }
    }
}

/// Detect clones from pre-prepared sources grouped by format.
///
/// Called by orchestrate.rs after `tokenize_to_detection` — skips re-hashing.
pub fn detect_prepared(
    format_groups: Vec<Vec<PreparedSource>>,
    min_tokens: usize,
    skip_local: bool,
    min_lines: usize,
) -> Vec<CpdClone> {
    if format_groups.is_empty() || min_tokens == 0 {
        return vec![];
    }

    let all_clones: Vec<Vec<CpdClone>> = format_groups
        .into_par_iter()
        .map(|group| detect_in_group(&group, min_tokens, skip_local, min_lines))
        .collect();

    let mut clones: Vec<CpdClone> = all_clones.into_iter().flatten().collect();
    dedup_exact_clones(&mut clones);
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
// Core detection — per format group
// ---------------------------------------------------------------------------

fn detect_in_group(
    prepared: &[PreparedSource],
    min_tokens: usize,
    skip_local: bool,
    min_lines: usize,
) -> Vec<CpdClone> {
    // Precompute window_power once for this format group.
    // If per-language min_tokens is introduced, recompute per group (it is already scoped here).
    let window_power = base_pow(min_tokens.saturating_sub(1));

    // Pre-allocate store capacity to avoid FxHashMap rehashing.
    let total_windows: usize = prepared
        .iter()
        .map(|p| p.hashes.len().saturating_sub(min_tokens))
        .sum();
    let mut store: WindowStore =
        FxHashMap::with_capacity_and_hasher(total_windows, Default::default());

    let mut clones: Vec<CpdClone> = Vec::new();
    // Cap at 2 — matching jscpd-rs SECONDARY_OCCURRENCE_CAP.
    // Values > 2 cause unbounded memory growth on boilerplate-heavy codebases.
    const SECONDARY_OCCURRENCE_CAP: usize = 2;
    let mut repeated_windows: FxHashMap<u64, Vec<Occurrence>> = FxHashMap::default();

    for (file_idx, source) in prepared.iter().enumerate() {
        let hashes = &source.hashes;
        if hashes.len() < min_tokens {
            continue;
        }
        let windows_len = hashes.len() - min_tokens + 1;

        // open_clone state machine: replaces emit-every-window + suppress_subclones.
        // A clone is opened when a matching window is found and enlarged as long as
        // subsequent windows also match. flush_clone is called when the match breaks
        // or the file scan ends — only one clone per contiguous matching region.
        let mut open_clone: Option<OpenClone> = None;

        let mut window_hash = hash_window(&hashes[..min_tokens]);

        for token_start in 0..windows_len {
            if token_start > 0 {
                window_hash = roll(
                    window_hash,
                    hashes[token_start - 1],
                    hashes[token_start + min_tokens - 1],
                    window_power,
                );
            }

            let current = Occurrence {
                source_id: file_idx,
                token_start,
            };

            match store.get(&window_hash).copied() {
                Some(stored) if windows_match(stored, current, prepared) => {
                    if open_clone.is_none() {
                        open_clone = Some(OpenClone {
                            stored_occurrence: stored,
                            current_start: token_start,
                            match_len: min_tokens,
                        });
                    } else if let Some(ref mut oc) = open_clone {
                        // Enlarge: the next window also matches — extend by one token.
                        oc.match_len += 1;
                    }
                    remember_repeated_window(
                        &mut repeated_windows,
                        window_hash,
                        stored,
                        SECONDARY_OCCURRENCE_CAP,
                    );
                    remember_repeated_window(
                        &mut repeated_windows,
                        window_hash,
                        current,
                        SECONDARY_OCCURRENCE_CAP,
                    );
                    // Do NOT update store — keep the first occurrence so the enlargement
                    // stays consistent across the contiguous match region.
                }
                _ => {
                    // Match broke (or no entry). Flush whatever was open.
                    flush_clone(
                        open_clone.take(),
                        file_idx,
                        prepared,
                        skip_local,
                        min_lines,
                        &mut clones,
                    );
                    store.insert(window_hash, current);
                }
            }
        }

        // Flush any open clone at the end of the file scan.
        flush_clone(
            open_clone.take(),
            file_idx,
            prepared,
            skip_local,
            min_lines,
            &mut clones,
        );
    }

    add_secondary_clones(
        repeated_windows,
        prepared,
        min_tokens,
        skip_local,
        min_lines,
        &mut clones,
    );

    clones
}

// ---------------------------------------------------------------------------
// Open clone state machine helpers
// ---------------------------------------------------------------------------

struct OpenClone {
    stored_occurrence: Occurrence,
    current_start: usize,
    match_len: usize,
}

/// Returns true if the window at `current` actually matches the window at `stored`
/// (hash match is necessary but not sufficient — verify token equality).
fn windows_match(stored: Occurrence, current: Occurrence, _prepared: &[PreparedSource]) -> bool {
    if stored.source_id == current.source_id && stored.token_start == current.token_start {
        return false; // same position — not a duplicate
    }
    // Hash match is sufficient for detection (Rabin-Karp assumption).
    // We trust the hash; no secondary token-by-token verification here.
    // The chance of false positives with xxh3_64 is negligible.
    true
}

/// Flush an open clone to the clones list.
///
/// A clone is rejected if its line span is shorter than `min_lines`.
/// The line span is measured as `end.line - start.line` (which equals
/// `number_of_lines - 1`). Mirrors jscpd's `LinesLengthCloneValidator`.
fn flush_clone(
    open: Option<OpenClone>,
    current_file_idx: usize,
    prepared: &[PreparedSource],
    skip_local: bool,
    min_lines: usize,
    clones: &mut Vec<CpdClone>,
) {
    let oc = match open {
        Some(o) => o,
        None => return,
    };

    let existing = &oc.stored_occurrence;
    let cur_start = oc.current_start;
    let match_len = oc.match_len;

    let existing_file = &prepared[existing.source_id];
    let current_file = &prepared[current_file_idx];

    let ex_start = existing.token_start;
    let ex_end = ex_start + match_len - 1;
    let cur_end = cur_start + match_len - 1;

    // Guard: don't emit a self-clone for windows that reference overlapping token
    // ranges in the same file. Adjacent ranges (sharing exactly one boundary token)
    // are allowed — they represent consecutive duplicate blocks.
    //
    // Overlap condition: the two regions [ex_start, ex_end] and [cur_start, cur_end]
    // overlap if ex_start < cur_end AND cur_start < ex_end (strict less-than).
    // Using strict less-than excludes the adjacent/boundary case.
    if existing.source_id == current_file_idx {
        let overlap = if ex_start < cur_start {
            // ex region starts first; overlaps if ex_end > cur_start
            ex_end > cur_start
        } else {
            // cur region starts first (or equal); overlaps if cur_end > ex_start
            cur_end > ex_start
        };
        if overlap {
            return;
        }
    }

    // skip_local: drop clone pairs where both fragments share the same parent directory.
    if skip_local {
        let dir_a = Path::new(&existing_file.id).parent();
        let dir_b = Path::new(&current_file.id).parent();
        if dir_a == dir_b {
            return;
        }
    }

    let fragment_a = match make_fragment(&existing_file.id, &existing_file.spans, ex_start, ex_end)
    {
        Some(f) => f,
        None => return,
    };
    let fragment_b = match make_fragment(&current_file.id, &current_file.spans, cur_start, cur_end)
    {
        Some(f) => f,
        None => return,
    };

    // min_lines filter: reject clones whose line span is shorter than min_lines.
    // Mirrors jscpd's LinesLengthCloneValidator which checks
    //   duplicationA.end.line - duplicationA.start.line >= minLines
    if min_lines > 0 {
        let span_a = fragment_a.end.line as i64 - fragment_a.start.line as i64;
        let span_b = fragment_b.end.line as i64 - fragment_b.start.line as i64;
        if span_a < min_lines as i64 && span_b < min_lines as i64 {
            return;
        }
    }

    clones.push(CpdClone {
        format: current_file.format.clone(),
        fragment_a,
        fragment_b,
        token_count: match_len as u32,
    });
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
// Deduplication — O(n) FxHashSet + sub-clone suppression
// ---------------------------------------------------------------------------

fn dedup_exact_clones(clones: &mut Vec<CpdClone>) {
    // Normalize each clone so fragment_a <= fragment_b (by id then start line).
    for clone in clones.iter_mut() {
        let a_key = (&clone.fragment_a.source_id, clone.fragment_a.start.line);
        let b_key = (&clone.fragment_b.source_id, clone.fragment_b.start.line);
        if a_key > b_key {
            std::mem::swap(&mut clone.fragment_a, &mut clone.fragment_b);
        }
    }

    let mut seen: FxHashSet<CloneDedupKey> = FxHashSet::default();
    clones.retain(|c| seen.insert(CloneDedupKey::from_clone(c)));
}

/// Remove clones that are fully contained (by token range) within a larger clone
/// of the same file pair. Largest clones first so containers are processed before
/// their contained sub-clones.
///
/// This handles sub-clone outputs from the secondary pass and any edge cases
/// where the primary emits adjacent-region duplicates.
fn suppress_subclones(clones: &mut Vec<CpdClone>) {
    use std::cmp::Reverse;
    clones.sort_by_key(|c| Reverse(c.token_count));

    let n = clones.len();
    let mut keep = vec![true; n];

    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let big_a_id = clones[i].fragment_a.source_id.clone();
        let big_b_id = clones[i].fragment_b.source_id.clone();
        let big_a_range = clones[i].fragment_a.range;
        let big_b_range = clones[i].fragment_b.range;

        for j in (i + 1)..n {
            if !keep[j] {
                continue;
            }
            let small = &clones[j];
            if small.fragment_a.source_id != big_a_id || small.fragment_b.source_id != big_b_id {
                continue;
            }
            // Sub-clone: small's both fragments are contained within big's ranges.
            if big_a_range[0] <= small.fragment_a.range[0]
                && big_a_range[1] >= small.fragment_a.range[1]
                && big_b_range[0] <= small.fragment_b.range[0]
                && big_b_range[1] >= small.fragment_b.range[1]
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

// ---------------------------------------------------------------------------
// Secondary clone pass
// ---------------------------------------------------------------------------

fn remember_repeated_window(
    repeated_windows: &mut FxHashMap<u64, Vec<Occurrence>>,
    hash: u64,
    occurrence: Occurrence,
    cap: usize,
) {
    let bucket = repeated_windows.entry(hash).or_default();
    if bucket
        .iter()
        .any(|s| s.source_id == occurrence.source_id && s.token_start == occurrence.token_start)
    {
        return;
    }
    if bucket.len() < cap {
        bucket.push(occurrence);
    }
}

fn add_secondary_clones(
    repeated_windows: FxHashMap<u64, Vec<Occurrence>>,
    prepared: &[PreparedSource],
    min_tokens: usize,
    skip_local: bool,
    min_lines: usize,
    clones: &mut Vec<CpdClone>,
) {
    if repeated_windows.is_empty() {
        return;
    }

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Candidate {
        source_a: usize,
        source_b: usize,
        token_a: usize,
        token_b: usize,
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    for occurrences in repeated_windows.values() {
        if occurrences.len() < 2 {
            continue;
        }
        for li in 0..occurrences.len() {
            for ri in li + 1..occurrences.len() {
                let left = &occurrences[li];
                let right = &occurrences[ri];
                if left.source_id == right.source_id && left.token_start == right.token_start {
                    continue;
                }
                let lh = &prepared[left.source_id].hashes;
                let rh = &prepared[right.source_id].hashes;
                let la = left.token_start;
                let ra = right.token_start;
                if la + min_tokens > lh.len() || ra + min_tokens > rh.len() {
                    continue;
                }
                if lh[la..la + min_tokens] != rh[ra..ra + min_tokens] {
                    continue;
                }
                let (sa, ta, sb, tb) =
                    if (left.source_id, left.token_start) <= (right.source_id, right.token_start) {
                        (
                            left.source_id,
                            left.token_start,
                            right.source_id,
                            right.token_start,
                        )
                    } else {
                        (
                            right.source_id,
                            right.token_start,
                            left.source_id,
                            left.token_start,
                        )
                    };
                candidates.push(Candidate {
                    source_a: sa,
                    source_b: sb,
                    token_a: ta,
                    token_b: tb,
                });
            }
        }
    }
    if candidates.is_empty() {
        return;
    }
    candidates.sort_unstable();
    candidates.dedup();

    // Build line-coverage from already-found primary clones.
    let n_files = prepared.len();
    let mut covered: Vec<Vec<(u32, u32)>> = vec![Vec::new(); n_files];
    for c in clones.iter() {
        let fa = prepared.iter().position(|p| p.id == c.fragment_a.source_id);
        let fb = prepared.iter().position(|p| p.id == c.fragment_b.source_id);
        if let Some(idx) = fa {
            covered[idx].push((c.fragment_a.start.line, c.fragment_a.end.line));
        }
        if let Some(idx) = fb {
            covered[idx].push((c.fragment_b.start.line, c.fragment_b.end.line));
        }
    }
    for ranges in &mut covered {
        ranges.sort_unstable();
    }

    let line_extends_coverage = |file_idx: usize, start: u32, end: u32| -> bool {
        let ranges = &covered[file_idx];
        let mut next = start;
        for &(rs, re) in ranges {
            if re < next {
                continue;
            }
            if rs > next {
                return true;
            }
            next = next.max(re.saturating_add(1));
            if next > end {
                return false;
            }
        }
        next <= end
    };

    for cand in candidates {
        // Build the clone using flush_clone's logic (direct fragment construction).
        let existing = Occurrence {
            source_id: cand.source_a,
            token_start: cand.token_a,
        };
        let current = Occurrence {
            source_id: cand.source_b,
            token_start: cand.token_b,
        };

        // skip_local check
        if skip_local {
            let dir_a = Path::new(&prepared[existing.source_id].id).parent();
            let dir_b = Path::new(&prepared[current.source_id].id).parent();
            if dir_a == dir_b {
                continue;
            }
        }

        // Greedy extend.
        let ex_hashes = &prepared[existing.source_id].hashes;
        let cur_hashes = &prepared[current.source_id].hashes;
        let max_extend = (ex_hashes
            .len()
            .saturating_sub(existing.token_start + min_tokens))
        .min(
            cur_hashes
                .len()
                .saturating_sub(current.token_start + min_tokens),
        );
        let mut extra = 0usize;
        while extra < max_extend
            && ex_hashes[existing.token_start + min_tokens + extra]
                == cur_hashes[current.token_start + min_tokens + extra]
        {
            extra += 1;
        }
        let match_len = min_tokens + extra;

        let ex_start = existing.token_start;
        let ex_end = ex_start + match_len - 1;
        let cur_start = current.token_start;
        let cur_end = cur_start + match_len - 1;

        let frag_a = match make_fragment(
            &prepared[existing.source_id].id,
            &prepared[existing.source_id].spans,
            ex_start,
            ex_end,
        ) {
            Some(f) => f,
            None => continue,
        };
        let frag_b = match make_fragment(
            &prepared[current.source_id].id,
            &prepared[current.source_id].spans,
            cur_start,
            cur_end,
        ) {
            Some(f) => f,
            None => continue,
        };

        let nc = CpdClone {
            format: prepared[current.source_id].format.clone(),
            fragment_a: frag_a,
            fragment_b: frag_b,
            token_count: match_len as u32,
        };

        // min_lines filter for secondary clones too.
        if min_lines > 0 {
            let span_a = nc.fragment_a.end.line as i64 - nc.fragment_a.start.line as i64;
            let span_b = nc.fragment_b.end.line as i64 - nc.fragment_b.start.line as i64;
            if span_a < min_lines as i64 && span_b < min_lines as i64 {
                continue;
            }
        }

        let extends_a = line_extends_coverage(
            cand.source_a,
            nc.fragment_a.start.line,
            nc.fragment_a.end.line,
        );
        let extends_b = line_extends_coverage(
            cand.source_b,
            nc.fragment_b.start.line,
            nc.fragment_b.end.line,
        );
        if extends_a || extends_b {
            clones.push(nc);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Location, Token, TokenKind};

    fn loc(line: u32, col: u32, offset: u32) -> Location {
        Location {
            line,
            column: col,
            offset,
        }
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
        SourceFile {
            id: id.to_string(),
            format: format.to_string(),
            tokens,
        }
    }

    fn js_tokens_ab() -> Vec<Token> {
        vec![
            make_token(TokenKind::Keyword, "function", 1, 0, 0),
            make_token(TokenKind::Other, "hello", 1, 9, 9),
            make_token(TokenKind::Operator, "(", 1, 14, 14),
            make_token(TokenKind::Operator, ")", 1, 15, 15),
            make_token(TokenKind::Operator, "{", 1, 16, 16),
            make_token(TokenKind::Keyword, "return", 2, 0, 18),
            make_token(TokenKind::Literal, "42", 2, 7, 25),
            make_token(TokenKind::Operator, ";", 2, 9, 27),
            make_token(TokenKind::Operator, "}", 3, 0, 29),
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
        assert!(
            !clones.is_empty(),
            "identical files must produce at least one clone"
        );
    }

    #[test]
    fn min_tokens_threshold_respected() {
        let tokens = js_tokens_ab(); // 9 tokens
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 100);
        assert!(
            clones.is_empty(),
            "no clones when min_tokens exceeds file length"
        );
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
        let file_py = make_file("a.py", "python", tokens);
        let clones = detect(&[file_js, file_py], 5);
        assert!(
            clones.is_empty(),
            "tokens from different formats must not match"
        );
    }

    #[test]
    fn identical_files_maximal_clone() {
        // With the open_clone state machine, a single maximal clone is emitted
        // instead of multiple sliding-window sub-clones.
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 5);
        assert_eq!(
            clones.len(),
            1,
            "open_clone SM must produce one maximal clone"
        );
        assert_eq!(
            clones[0].token_count, 9,
            "maximal clone must cover all 9 tokens"
        );
    }

    #[test]
    fn three_identical_files_secondary_pass_adds_missing_pair() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens.clone());
        let file_c = make_file("c.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b, file_c], 5);
        assert!(
            clones.len() >= 2,
            "three identical files must yield at least 2 clone pairs, got {}",
            clones.len()
        );
    }

    #[test]
    fn clones_sorted_by_source_and_line() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b], 5);
        for i in 1..clones.len() {
            let prev = &clones[i - 1];
            let curr = &clones[i];
            assert!(
                (
                    &prev.fragment_a.source_id,
                    prev.fragment_a.start.line,
                    &prev.fragment_b.source_id,
                    prev.fragment_b.start.line,
                ) <= (
                    &curr.fragment_a.source_id,
                    curr.fragment_a.start.line,
                    &curr.fragment_b.source_id,
                    curr.fragment_b.start.line,
                ),
                "clones must be sorted"
            );
        }
    }
}
