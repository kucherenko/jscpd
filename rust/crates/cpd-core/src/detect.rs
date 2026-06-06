// detect.rs

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
    // Cap repeated-window occurrences per hash. Higher values find more clone pairs
    // among 3+ similar files (e.g., file_1.js, file_1.mjs, file_1.cjs) but use more memory.
    // The TypeScript jscpd compares all file pairs, so we raise this to match its coverage.
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
                Some(stored) if windows_match(stored, current, prepared, min_tokens) => {
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
fn windows_match(
    stored: Occurrence,
    current: Occurrence,
    prepared: &[PreparedSource],
    min_tokens: usize,
) -> bool {
    if stored.source_id == current.source_id && stored.token_start == current.token_start {
        return false;
    }
    let stored_hashes = &prepared[stored.source_id].hashes;
    let current_hashes = &prepared[current.source_id].hashes;
    if stored.token_start + min_tokens > stored_hashes.len()
        || current.token_start + min_tokens > current_hashes.len()
    {
        return false;
    }
    stored_hashes[stored.token_start..stored.token_start + min_tokens]
        == current_hashes[current.token_start..current.token_start + min_tokens]
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

    // min_lines filter: reject clones whose fragment A line span is shorter than min_lines.
    // Mirrors jscpd's LinesLengthCloneValidator which checks only duplicationA:
    //   duplicationA.end.line - duplicationA.start.line >= minLines
    if min_lines > 0 {
        let lines = fragment_a.end.line as usize - fragment_a.start.line as usize;
        if lines < min_lines {
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

struct SecondaryOpen {
    clone: CpdClone,
    source_a: usize,
    source_b: usize,
    last_token_start_a: usize,
    last_token_start_b: usize,
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
    let mut coverage = LineCoverage::from_clones(prepared, clones);
    let mut open: Option<SecondaryOpen> = None;

    for candidate in candidates {
        if let Some(current) = open.as_mut()
            && current.source_a == candidate.source_a
            && current.source_b == candidate.source_b
            && current.last_token_start_a + 1 == candidate.token_a
            && current.last_token_start_b + 1 == candidate.token_b
        {
            // Enlarge: extend the open secondary clone by one token on each side.
            let new_match_len = current.clone.token_count as usize + 1;
            let end_idx_a = candidate.token_a + min_tokens;
            let end_idx_b = candidate.token_b + min_tokens;
            if let Some(frag_a_end) = prepared[current.source_a].spans.get(end_idx_a) {
                current.clone.fragment_a.end = frag_a_end.1.clone();
                current.clone.fragment_a.range[1] = end_idx_a as u32;
            }
            if let Some(frag_b_end) = prepared[current.source_b].spans.get(end_idx_b) {
                current.clone.fragment_b.end = frag_b_end.1.clone();
                current.clone.fragment_b.range[1] = end_idx_b as u32;
            }
            current.clone.token_count = new_match_len as u32;
            current.last_token_start_a = candidate.token_a;
            current.last_token_start_b = candidate.token_b;
            continue;
        }

        flush_secondary_clone(open.take(), prepared, skip_local, min_lines, clones, &mut coverage);

        // Create a new secondary clone candidate.
        let start_a = candidate.token_a;
        let end_a = start_a + min_tokens - 1;
        let start_b = candidate.token_b;
        let end_b = start_b + min_tokens - 1;

        let frag_a = match make_fragment(
            &prepared[candidate.source_a].id,
            &prepared[candidate.source_a].spans,
            start_a,
            end_a,
        ) {
            Some(f) => f,
            None => continue,
        };
        let frag_b = match make_fragment(
            &prepared[candidate.source_b].id,
            &prepared[candidate.source_b].spans,
            start_b,
            end_b,
        ) {
            Some(f) => f,
            None => continue,
        };

        open = Some(SecondaryOpen {
            clone: CpdClone {
                format: prepared[candidate.source_a].format.clone(),
                fragment_a: frag_a,
                fragment_b: frag_b,
                token_count: min_tokens as u32,
            },
            source_a: candidate.source_a,
            source_b: candidate.source_b,
            last_token_start_a: candidate.token_a,
            last_token_start_b: candidate.token_b,
        });
    }

    flush_secondary_clone(open.take(), prepared, skip_local, min_lines, clones, &mut coverage);
}

fn flush_secondary_clone(
    open: Option<SecondaryOpen>,
    prepared: &[PreparedSource],
    skip_local: bool,
    min_lines: usize,
    clones: &mut Vec<CpdClone>,
    coverage: &mut LineCoverage,
) {
    let Some(oc) = open else {
        return;
    };

    let range_a = fragment_line_range(&oc.clone.fragment_a);
    let range_b = fragment_line_range(&oc.clone.fragment_b);

    // skip_local check
    if skip_local {
        let dir_a = Path::new(&prepared[oc.source_a].id).parent();
        let dir_b = Path::new(&prepared[oc.source_b].id).parent();
        if dir_a == dir_b {
            return;
        }
    }

    // min_lines filter: only check fragment A, mirroring jscpd's LinesLengthCloneValidator.
    if min_lines > 0 {
        let lines = oc.clone.fragment_a.end.line as usize - oc.clone.fragment_a.start.line as usize;
        if lines < min_lines {
            return;
        }
    }

    // Line-coverage filter: skip secondary clones that don't extend existing coverage
    // on either side.  This prevents the report from filling up with dozens of
    // overlapping sub-clones of the same region.
    if !coverage.extends(oc.source_a, range_a) || !coverage.extends(oc.source_b, range_b) {
        return;
    }

    let before = clones.len();
    clones.push(oc.clone);

    // Insert coverage for newly added clone.
    if clones.len() > before {
        coverage.insert(oc.source_a, range_a);
        coverage.insert(oc.source_b, range_b);
    }
}

fn fragment_line_range(fragment: &Fragment) -> (usize, usize) {
    let start = fragment.start.line as usize;
    let end = fragment.end.line as usize;
    (start.min(end), start.max(end))
}

// ---------------------------------------------------------------------------
// Line coverage tracking for secondary clones
// ---------------------------------------------------------------------------

struct LineCoverage {
    ranges_by_source: Vec<Vec<(usize, usize)>>,
}

impl LineCoverage {
    fn from_clones(prepared: &[PreparedSource], clones: &[CpdClone]) -> Self {
        let mut source_lookup: FxHashMap<&str, usize> = FxHashMap::default();
        for (idx, source) in prepared.iter().enumerate() {
            source_lookup.insert(source.id.as_str(), idx);
        }
        let mut coverage = Self {
            ranges_by_source: vec![Vec::new(); prepared.len()],
        };
        for clone in clones {
            if let Some(idx) = source_lookup.get(clone.fragment_a.source_id.as_str()) {
                coverage.insert(*idx, fragment_line_range(&clone.fragment_a));
            }
            if let Some(idx) = source_lookup.get(clone.fragment_b.source_id.as_str()) {
                coverage.insert(*idx, fragment_line_range(&clone.fragment_b));
            }
        }
        coverage
    }

    fn extends(&self, source_idx: usize, range: (usize, usize)) -> bool {
        let Some(ranges) = self.ranges_by_source.get(source_idx) else {
            return true;
        };
        let mut next_line = range.0;
        for &(start, end) in ranges {
            if end < next_line {
                continue;
            }
            if start > next_line {
                return true;
            }
            next_line = next_line.max(end.saturating_add(1));
            if next_line > range.1 {
                return false;
            }
        }
        next_line <= range.1
    }

    fn insert(&mut self, source_idx: usize, range: (usize, usize)) {
        let Some(ranges) = self.ranges_by_source.get_mut(source_idx) else {
            return;
        };
        ranges.push(range);
        ranges.sort_unstable();

        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
        for &(start, end) in ranges.iter() {
            if let Some((_, previous_end)) = merged.last_mut()
                && start <= previous_end.saturating_add(1)
            {
                *previous_end = (*previous_end).max(end);
                continue;
            }
            merged.push((start, end));
        }
        *ranges = merged;
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
