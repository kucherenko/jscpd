// detect.rs

use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use crate::{
    hash::{HASH_BASE, base_pow, roll, token_hash},
    models::{
        CloneKind, CpdClone, DetectionToken, Fragment, Location, SimilarityMethod, SourceFile,
        TokenKind, covered_lines,
    },
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
    detect_with_options(files, min_tokens, 0, &PathFilters::default())
}

/// Detect clones with extended options.
///
/// - `min_lines`: reject clones whose fragment line span is shorter than this.
///   The line span is `end.line - start.line`; a clone is kept only if this value
///   is >= `min_lines`. This mirrors jscpd's `LinesLengthCloneValidator`.
/// - `filters`: path-based clone pair filters ([`PathFilters`]).
pub fn detect_with_options(
    files: &[SourceFile],
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
) -> Vec<CpdClone> {
    if files.is_empty() || min_tokens == 0 {
        return vec![];
    }

    // Group files by format. Sort for deterministic order.
    let mut by_format: FxHashMap<&str, Vec<&SourceFile>> = FxHashMap::default();
    for file in files {
        by_format
            .entry(file.format.as_str())
            .or_default()
            .push(file);
    }
    let mut format_groups: Vec<(&str, Vec<&SourceFile>)> = by_format.into_iter().collect();
    format_groups.sort_unstable_by_key(|(fmt, _)| *fmt);
    for (_, group) in &mut format_groups {
        group.sort_unstable_by_key(|&file| file.id.as_str());
    }

    let mut clones: Vec<CpdClone> = format_groups
        .into_par_iter()
        .flat_map(|(_format, files)| {
            // Build per-group prepared data from SourceFile.tokens.
            // This is the backward-compat path; orchestrate.rs uses
            // detect_prepared() directly to avoid re-hashing.
            let prepared: Vec<PreparedSource> = files
                .into_iter()
                .map(|file| {
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
                        raw_hashes: Vec::new(),
                        renames: Renames::default(),
                        real_path: String::new(),
                        embedded: false,
                    }
                })
                .collect();
            detect_in_group(&prepared, min_tokens, min_lines, filters)
        })
        .collect();

    finalize_clones(&mut clones);
    clones
}

fn finalize_clones(clones: &mut Vec<CpdClone>) {
    dedup_exact_clones(clones);
    clones.sort_by(|a, b| a.position_key().cmp(&b.position_key()));
}

// ---------------------------------------------------------------------------
// Direct DetectionToken path (called by orchestrate.rs)
// ---------------------------------------------------------------------------

/// A file ready for detection: pre-hashed, pre-filtered.
///
/// Produced either from `SourceFile.tokens` (backward compat) or directly from
/// `tokenize_to_detection` output (fast path used by orchestrate.rs).
#[derive(Debug, Clone)]
pub struct PreparedSource {
    pub id: String,
    pub format: String,
    pub hashes: Vec<u64>,
    pub spans: Vec<(Location, Location)>,
    /// Un-normalized token hashes, parallel to `hashes`. Empty unless a
    /// normalization option rewrote at least one token of this source; then
    /// it is used to classify clones as exact or renamed (issue #998).
    pub raw_hashes: Vec<u64>,
    /// Where the renamed identifiers repeat, for
    /// `--ignore-identifiers=consistent` (see [`Self::track_renames`]).
    /// Empty otherwise.
    pub renames: Renames,
    /// Canonical on-disk path of the file; empty when it equals `id`. The two
    /// differ behind a symlink: `id` keeps the path the walker found the file
    /// at, which is what reports, `--ignore` and the path filters use (issue
    /// #1059). `--skip-isolated` falls back to this path so a group folder
    /// that is itself a symlink still matches the files found through it.
    pub real_path: String,
    /// True when this source is one language embedded in another — a fenced
    /// block in markdown, a `<script>` in a single-file component. Its tokens
    /// keep the host file's line numbers, so a clone's line span may run
    /// across host text that belongs to no block (issue #1090); statistics
    /// discount those lines.
    pub embedded: bool,
}

impl PreparedSource {
    /// The canonical path when it differs from `id`, otherwise `id` itself.
    pub fn filter_path(&self) -> &str {
        if self.real_path.is_empty() {
            &self.id
        } else {
            &self.real_path
        }
    }

    /// Build from a `DetectionToken` slice — the fast path.
    pub fn from_detection_tokens(id: String, format: String, tokens: &[DetectionToken]) -> Self {
        let mut hashes = Vec::with_capacity(tokens.len());
        let mut spans = Vec::with_capacity(tokens.len());
        // Only materialize raw hashes once a token proves normalization was
        // applied; the default path allocates nothing extra. The tokens
        // before it hash as they are.
        let mut raw_hashes: Vec<u64> = Vec::new();
        let mut normalized = false;
        for (i, t) in tokens.iter().enumerate() {
            hashes.push(t.hash);
            spans.push((t.start.clone(), t.end.clone()));
            if !normalized && t.raw_hash != t.hash {
                normalized = true;
                raw_hashes.reserve(tokens.len());
                raw_hashes.extend(hashes[..i].iter().copied());
            }
            if normalized {
                raw_hashes.push(t.raw_hash);
            }
        }
        Self {
            id,
            format,
            hashes,
            spans,
            raw_hashes,
            renames: Renames::default(),
            real_path: String::new(),
            embedded: false,
        }
    }

    /// Records where each renamed identifier occurs again, so that detection
    /// matches names consistently (`--ignore-identifiers`, issue #1217): two
    /// fragments match only when every name stands for one name on the other
    /// side and no two names for the same one, so `d = a + b` matches
    /// `s = p + q` but not `x = x + x`. `tokens` are the tokens this source
    /// was made from; their `name` tells the renamed identifiers. Leaves a
    /// source without renamed identifiers as it is: its windows hash the
    /// same either way.
    pub fn track_renames(&mut self, tokens: &[DetectionToken]) {
        debug_assert_eq!(tokens.len(), self.hashes.len());
        if tokens.iter().all(|t| t.name == 0) {
            return;
        }
        let mut prev = vec![0u32; tokens.len()];
        let mut last: FxHashMap<u64, usize> = FxHashMap::default();
        for (j, token) in tokens.iter().enumerate() {
            if token.name == 0 {
                continue;
            }
            if let Some(p) = last.insert(token.name, j) {
                // A name too far back for any window counts as new.
                prev[j] = u32::try_from(j - p).unwrap_or(0);
            }
        }
        self.renames = Renames { prev };
    }
}

/// For every token of a source, how many tokens back the same renamed
/// identifier occurred last (0: never), when detection matches names
/// consistently. This is Baker's parameterized matching: a window hashes a
/// renamed identifier with that distance when the earlier occurrence lies
/// in the window ([`within`]), so two windows hash alike only when their
/// names repeat alike. Sliding the window changes the distance of one token
/// at most, the next occurrence of the name that leaves it.
#[derive(Debug, Clone, Default)]
pub struct Renames {
    prev: Vec<u32>,
}

impl Renames {
    pub fn is_empty(&self) -> bool {
        self.prev.is_empty()
    }

    /// How far back the name at `j` occurred last within the window or part
    /// that starts at `start`; 0 when it did not, and for any other token.
    #[inline]
    fn back(&self, j: usize, start: usize) -> u32 {
        within(self.prev.get(j).copied().unwrap_or(0), j - start)
    }

    /// For every token, how many tokens ahead its name occurs next (0:
    /// never), into `next`: what [`roll_consistent`] needs.
    fn next_into(&self, next: &mut Vec<u32>) {
        next.clear();
        next.resize(self.prev.len(), 0);
        for (j, &distance) in self.prev.iter().enumerate() {
            if distance != 0 {
                next[j - distance as usize] = distance;
            }
        }
    }

    /// Whether the windows of `len` tokens from `a_start` in `a` and from
    /// `b_start` in `b` repeat their names alike: each token is as far back
    /// from the previous occurrence of its name within the window on both
    /// sides. Every source repeats nothing when its names are not tracked.
    fn repeat_alike(a: &Renames, a_start: usize, b: &Renames, b_start: usize, len: usize) -> bool {
        match (a.is_empty(), b.is_empty()) {
            (true, true) => true,
            (false, false) => {
                let (a, b) = (
                    &a.prev[a_start..a_start + len],
                    &b.prev[b_start..b_start + len],
                );
                (0..len)
                    .zip(a.iter().zip(b))
                    .fold(true, |same, (k, (&x, &y))| {
                        same & (within(x, k) == within(y, k))
                    })
            }
            _ => (0..len).all(|k| a.back(a_start + k, a_start) == b.back(b_start + k, b_start)),
        }
    }
}

/// A distance back to the previous occurrence of a name counts only when
/// that occurrence lies in the window or part, here `k` tokens into it.
#[inline]
fn within(distance: u32, k: usize) -> u32 {
    if distance as usize <= k { distance } else { 0 }
}

/// A renamed identifier that occurred `distance` tokens earlier in the
/// window; the token's own hash for 0. Distinct distances give distinct
/// hashes: `HASH_BASE` is odd.
#[inline]
fn rename_hash(hash: u64, distance: u32) -> u64 {
    hash ^ (distance as u64).wrapping_mul(HASH_BASE)
}

/// What token `j` of `p` adds to the hash of the window that starts at
/// `start`.
#[inline]
fn window_token(p: &PreparedSource, j: usize, start: usize) -> u64 {
    rename_hash(p.hashes[j], p.renames.back(j, start))
}

/// The hash of the `len` tokens of `p` from `start`, computed from scratch;
/// [`crate::hash::hash_window`] for a source whose names are not tracked.
fn consistent_window_hash(p: &PreparedSource, start: usize, len: usize) -> u64 {
    (start..start + len).fold(0u64, |hash, j| {
        hash.wrapping_mul(HASH_BASE)
            .wrapping_add(window_token(p, j, start))
    })
}

/// The hash of the window of `len` tokens from `start` of a source whose
/// names are tracked, rolled from `hash`, the window one token earlier: the
/// token that left goes out, the one that came in goes in, and the next
/// occurrence of the name that left (`next`, see [`Renames::next_into`])
/// becomes the first one of the window. `powers[k]` is `HASH_BASE^k`.
#[inline]
fn roll_consistent(
    p: &PreparedSource,
    hash: u64,
    start: usize,
    len: usize,
    window_power: u64,
    next: &[u32],
    powers: &[u64],
) -> u64 {
    let last = start + len - 1;
    let rolled = roll(
        hash,
        p.hashes[start - 1],
        window_token(p, last, start),
        window_power,
    );
    let distance = next[start - 1] as usize;
    let q = start - 1 + distance;
    // The token that came in was hashed for this window already.
    if distance == 0 || q >= last {
        return rolled;
    }
    let was = rename_hash(p.hashes[q], distance as u32);
    rolled.wrapping_add(p.hashes[q].wrapping_sub(was).wrapping_mul(powers[last - q]))
}

/// Detect clones from pre-prepared sources grouped by format.
///
/// Called by orchestrate.rs after `tokenize_to_detection` — skips re-hashing.
/// - `filters`: path-based clone pair filters ([`PathFilters`]).
pub fn detect_prepared(
    format_groups: Vec<Vec<PreparedSource>>,
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
) -> Vec<CpdClone> {
    if format_groups.is_empty() || min_tokens == 0 {
        return vec![];
    }

    let mut clones: Vec<CpdClone> = format_groups
        .into_par_iter()
        .flat_map(|group| detect_in_group(&group, min_tokens, min_lines, filters))
        .collect();

    finalize_clones(&mut clones);
    clones
}

// ---------------------------------------------------------------------------
// Core detection — per format group
// ---------------------------------------------------------------------------

fn detect_in_group(
    prepared: &[PreparedSource],
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
) -> Vec<CpdClone> {
    // Precompute window_power once for this format group.
    // If per-language min_tokens is introduced, recompute per group (it is already scoped here).
    let window_power = base_pow(min_tokens.saturating_sub(1));
    // The weight of every position in a window, and where each name occurs
    // next, for the sources whose names are matched consistently (see
    // `Renames`).
    let powers: Vec<u64> = match prepared.iter().any(|p| !p.renames.is_empty()) {
        true => std::iter::successors(Some(1u64), |p| Some(p.wrapping_mul(HASH_BASE)))
            .take(min_tokens)
            .collect(),
        false => Vec::new(),
    };
    let mut next: Vec<u32> = Vec::new();

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

        let consistent = !source.renames.is_empty();
        if consistent {
            source.renames.next_into(&mut next);
        }
        let mut window_hash = consistent_window_hash(source, 0, min_tokens);

        for token_start in 0..windows_len {
            if token_start > 0 {
                window_hash = match consistent {
                    true => roll_consistent(
                        source,
                        window_hash,
                        token_start,
                        min_tokens,
                        window_power,
                        &next,
                        &powers,
                    ),
                    false => roll(
                        window_hash,
                        hashes[token_start - 1],
                        hashes[token_start + min_tokens - 1],
                        window_power,
                    ),
                };
            }

            let current = Occurrence {
                source_id: file_idx,
                token_start,
            };

            // An open clone grows only while its own anchor keeps matching
            // (issue #1033). The store may hold a *different* occurrence for
            // this window — one stored by a third file whose text continues
            // the same way — and extending on that would stretch the anchored
            // fragment past what the anchor file contains: the clone is then
            // dropped by `flush_clone` when the anchor runs out of tokens, or
            // reported longer than the real common region when it does not.
            let anchor = open_clone.as_ref().map(|oc| Occurrence {
                source_id: oc.stored_occurrence.source_id,
                token_start: oc.stored_occurrence.token_start + (token_start - oc.current_start),
            });
            let anchor_continues = anchor
                .is_some_and(|anchor| window_continues(anchor, current, prepared, min_tokens));

            // While the clone grows, the stored window only goes to the
            // secondary pass: a full bucket needs no check, and the anchor
            // itself matches, exactly so where names are not tracked.
            let stored = match store.get(&window_hash).copied() {
                Some(_)
                    if anchor_continues
                        && repeated_windows
                            .get(&window_hash)
                            .is_some_and(|bucket| bucket.len() >= SECONDARY_OCCURRENCE_CAP) =>
                {
                    None
                }
                Some(stored) if anchor_continues && !consistent && Some(stored) == anchor => {
                    Some(stored)
                }
                stored => {
                    stored.filter(|stored| windows_match(*stored, current, prepared, min_tokens))
                }
            };

            if anchor_continues {
                if let Some(oc) = open_clone.as_mut() {
                    oc.match_len += 1;
                }
            } else {
                // The anchor stopped (or nothing was open): flush, then start a
                // new clone on whatever the store matched, if anything.
                flush_clone(
                    open_clone.take(),
                    file_idx,
                    prepared,
                    min_tokens,
                    min_lines,
                    filters,
                    &mut clones,
                );
                match stored {
                    Some(stored) => {
                        open_clone = Some(OpenClone {
                            stored_occurrence: stored,
                            current_start: token_start,
                            match_len: min_tokens,
                        });
                    }
                    None => {
                        store.insert(window_hash, current);
                    }
                }
            }
            if let Some(stored) = stored {
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
                // The store keeps the first occurrence so the enlargement stays
                // consistent across the contiguous match region.
            }
        }

        // Flush any open clone at the end of the file scan.
        flush_clone(
            open_clone.take(),
            file_idx,
            prepared,
            min_tokens,
            min_lines,
            filters,
            &mut clones,
        );
    }

    add_secondary_clones(
        repeated_windows,
        prepared,
        min_tokens,
        min_lines,
        filters,
        &mut clones,
    );

    clones
}

// ---------------------------------------------------------------------------
// Open clone state machine helpers
// ---------------------------------------------------------------------------

/// Path-based clone pair filters, applied when a clone is flushed.
///
/// Bundles the options that decide whether a clone pair is dropped based on
/// where its two fragments live on disk.
#[derive(Debug, Default, Clone, Copy)]
pub struct PathFilters<'a> {
    /// Skip clone pairs where both fragments are under the same scan root.
    /// Mirrors jscpd's `SkipLocalValidator`.
    pub skip_local: bool,
    /// Scan roots used by `skip_local` to determine same-directory pairs.
    pub scan_roots: &'a [PathBuf],
    /// Isolation groups (`--skip-isolated`): skip clone pairs whose fragments
    /// are under two *different* folders of the same group. Mirrors the
    /// `SkipIsolatedValidator` proposed in jscpd PR #628.
    pub isolated_groups: &'a [Vec<PathBuf>],
}

impl PathFilters<'_> {
    /// Returns true if the clone pair (`file_a`, `file_b`) must be dropped.
    pub fn should_skip(&self, file_a: &str, file_b: &str) -> bool {
        (self.skip_local && should_skip_local(file_a, file_b, self.scan_roots))
            || should_skip_isolated(file_a, file_b, self.isolated_groups)
    }

    /// `should_skip` for two prepared sources. Filters see the path a file
    /// was found at, like jscpd v4: `--skip-local` drops a pair found under
    /// the same scan root even when one side is a symlink into it. Isolation
    /// groups are canonicalized, so a group folder that is itself a symlink
    /// only matches the canonical path of the files found through it; that
    /// case is covered by a second check on the real paths.
    fn should_skip_pair(&self, a: &PreparedSource, b: &PreparedSource) -> bool {
        self.should_skip_sources((&a.id, &a.real_path), (&b.id, &b.real_path))
    }

    /// The decision [`Self::should_skip_pair`] makes, for two sources given
    /// by id and canonical path (empty when it is the id). The function pairs
    /// of `--similarity` go through it as token clones do.
    pub fn should_skip_sources(&self, a: (&str, &str), b: (&str, &str)) -> bool {
        fn filter_path<'p>((id, real): (&'p str, &'p str)) -> &'p str {
            match real.is_empty() {
                true => id,
                false => real,
            }
        }
        self.should_skip(a.0, b.0)
            || ((!a.1.is_empty() || !b.1.is_empty())
                && should_skip_isolated(filter_path(a), filter_path(b), self.isolated_groups))
    }
}

/// Where one file stands for the path filters, computed once so that
/// deciding a pair costs a few integer comparisons: `a.skips(&b)` agrees
/// with [`PathFilters::should_skip`] on the two paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathLabel {
    /// Indexes of the scan roots holding the file (only with `skip_local`).
    roots: Vec<u16>,
    /// `(group, folder)` for every isolation group with a folder holding
    /// the file, the first such folder as in `should_skip_isolated`.
    folders: Vec<(u16, u16)>,
}

impl PathLabel {
    /// True when a clone between files labelled `self` and `other` is
    /// dropped by the path filters the labels were made with.
    pub fn skips(&self, other: &PathLabel) -> bool {
        self.roots.iter().any(|r| other.roots.contains(r))
            || self
                .folders
                .iter()
                .any(|(g, f)| other.folders.iter().any(|(g2, f2)| g == g2 && f != f2))
    }
}

impl PathFilters<'_> {
    /// Whether any path filter is on.
    pub fn is_active(&self) -> bool {
        self.skip_local || !self.isolated_groups.is_empty()
    }

    /// The [`PathLabel`] of `file`.
    pub fn label(&self, file: &str) -> PathLabel {
        let roots = match self.skip_local {
            true => self
                .scan_roots
                .iter()
                .enumerate()
                .filter(|(_, root)| is_relative_to(file, root))
                .map(|(k, _)| k as u16)
                .collect(),
            false => Vec::new(),
        };
        let folders = self
            .isolated_groups
            .iter()
            .enumerate()
            .filter_map(|(g, group)| {
                let f = group.iter().position(|dir| is_relative_to(file, dir))?;
                Some((g as u16, f as u16))
            })
            .collect();
        PathLabel { roots, folders }
    }
}

/// Returns true if both files share a common scan root directory.
/// Mirrors jscpd's `SkipLocalValidator.shouldSkipClone`:
///   `path.some(dir => isRelative(fileA, dir) && isRelative(fileB, dir))`
fn should_skip_local(file_a: &str, file_b: &str, scan_roots: &[PathBuf]) -> bool {
    scan_roots
        .iter()
        .any(|root| is_relative_to(file_a, root) && is_relative_to(file_b, root))
}

/// Returns true if the two files fall under two different folders of the same
/// isolation group. Mirrors `SkipIsolatedValidator.shouldSkipClone` from jscpd
/// PR #628: for each group, take the first folder containing each file; the
/// clone is skipped when both files match and their folders differ.
fn should_skip_isolated(file_a: &str, file_b: &str, isolated_groups: &[Vec<PathBuf>]) -> bool {
    isolated_groups.iter().any(|group| {
        let Some(dir_a) = group.iter().find(|dir| is_relative_to(file_a, dir)) else {
            return false;
        };
        group
            .iter()
            .find(|dir| is_relative_to(file_b, dir))
            .is_some_and(|dir_b| dir_a != dir_b)
    })
}

/// Returns true if `file_path` is contained within `dir`.
/// Mirrors the TypeScript `SkipLocalValidator.isRelative`:
///   `const rel = relative(dir, file); return rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);`
fn is_relative_to(file_path: &str, dir: &PathBuf) -> bool {
    let file = Path::new(file_path);
    // Fast path: file path starts with the dir prefix
    if let Ok(rel) = file.strip_prefix(dir) {
        return !rel.as_os_str().is_empty();
    }
    // Mixed absolute/relative can never match via simple prefix
    if file.is_absolute() != dir.is_absolute() {
        return false;
    }
    // Walk up from the file path checking if any ancestor starts with dir
    let mut ancestor = file;
    loop {
        if ancestor == dir.as_path() {
            return false;
        }
        if ancestor.starts_with(dir) {
            let rel = ancestor.strip_prefix(dir).unwrap_or(ancestor);
            return !rel.as_os_str().is_empty();
        }
        ancestor = match ancestor.parent() {
            Some(p) => p,
            None => return false,
        };
    }
}

struct OpenClone {
    stored_occurrence: Occurrence,
    current_start: usize,
    match_len: usize,
}

/// Returns true if the window at `current` actually matches the window at `stored`
/// (hash match is necessary but not sufficient — verify token equality).
/// When names are matched consistently, their repeats must line up too.
fn windows_match(
    stored: Occurrence,
    current: Occurrence,
    prepared: &[PreparedSource],
    min_tokens: usize,
) -> bool {
    if stored.source_id == current.source_id && stored.token_start == current.token_start {
        return false;
    }
    let stored_source = &prepared[stored.source_id];
    let current_source = &prepared[current.source_id];
    let stored_hashes = &stored_source.hashes;
    let current_hashes = &current_source.hashes;
    if stored.token_start + min_tokens > stored_hashes.len()
        || current.token_start + min_tokens > current_hashes.len()
    {
        return false;
    }
    if stored_hashes[stored.token_start..stored.token_start + min_tokens]
        != current_hashes[current.token_start..current.token_start + min_tokens]
    {
        return false;
    }
    Renames::repeat_alike(
        &stored_source.renames,
        stored.token_start,
        &current_source.renames,
        current.token_start,
        min_tokens,
    )
}

/// Whether an open clone grows by the window at `current` against the one at
/// `anchor`: the windows one token earlier matched, so only the token each
/// window took in has to match. With names matched consistently, the clone
/// grows past a name that would stand for two, and [`consistent_runs`] cuts
/// it there when it closes, into parts that do not overlap.
fn window_continues(
    anchor: Occurrence,
    current: Occurrence,
    prepared: &[PreparedSource],
    min_tokens: usize,
) -> bool {
    let (a, b) = (&prepared[anchor.source_id], &prepared[current.source_id]);
    let last_a = anchor.token_start + min_tokens - 1;
    let last_b = current.token_start + min_tokens - 1;
    last_a < a.hashes.len() && last_b < b.hashes.len() && a.hashes[last_a] == b.hashes[last_b]
}

/// Flush an open clone to the clones list.
///
/// A clone is rejected if its line span is shorter than `min_lines`.
/// The line span is measured as `end.line - start.line` (which equals
/// `number_of_lines - 1`). Mirrors jscpd's `LinesLengthCloneValidator`.
/// A clone whose names match consistently only in parts is reported as
/// those parts, each of `min_tokens` tokens at least (see
/// [`consistent_runs`]).
fn flush_clone(
    open: Option<OpenClone>,
    current_file_idx: usize,
    prepared: &[PreparedSource],
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
    clones: &mut Vec<CpdClone>,
) {
    let oc = match open {
        Some(o) => o,
        None => return,
    };

    let existing = &oc.stored_occurrence;
    let existing_file = &prepared[existing.source_id];
    let current_file = &prepared[current_file_idx];

    // Path filters: drop clone pairs by fragment location (skip_local,
    // skip_isolated). Mirrors jscpd's SkipLocalValidator / SkipIsolatedValidator.
    if filters.should_skip_pair(existing_file, current_file) {
        return;
    }

    let runs = consistent_runs(
        existing_file,
        existing.token_start,
        current_file,
        oc.current_start,
        oc.match_len,
        min_tokens,
        min_lines,
    );
    for (offset, len) in runs {
        if len >= min_tokens {
            push_clone(
                existing_file,
                existing.token_start + offset,
                current_file,
                oc.current_start + offset,
                len,
                min_lines,
                offset > 0,
                clones,
            );
        }
    }
}

/// Report `len` matching tokens from `ex_start` in `existing_file` and
/// `cur_start` in `current_file` as one clone, unless it spans fewer than
/// `min_lines` lines. `after_cut` when a cut ends the part before it (see
/// [`CpdClone::after_cut`]).
#[allow(clippy::too_many_arguments)]
fn push_clone(
    existing_file: &PreparedSource,
    ex_start: usize,
    current_file: &PreparedSource,
    cur_start: usize,
    match_len: usize,
    min_lines: usize,
    after_cut: bool,
    clones: &mut Vec<CpdClone>,
) {
    let ex_end = ex_start + match_len - 1;
    let cur_end = cur_start + match_len - 1;

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
    let kind = clone_kind(
        existing_file,
        fragment_a.range,
        current_file,
        fragment_b.range,
    );

    // min_lines filter: reject clones whose fragment A line span is shorter than min_lines.
    // Mirrors jscpd's LinesLengthCloneValidator which checks only duplicationA:
    //   duplicationA.end.line - duplicationA.start.line >= minLines
    if min_lines > 0 {
        let lines = fragment_a.end.line as usize - fragment_a.start.line as usize;
        if lines < min_lines {
            return;
        }
    }

    let unmatched_lines = [
        host_lines_in(existing_file, &fragment_a),
        host_lines_in(current_file, &fragment_b),
    ];

    clones.push(CpdClone {
        format: current_file.format.clone(),
        fragment_a,
        fragment_b,
        token_count: match_len as u32,
        is_new: false,
        kind,
        similarity: None,
        similarity_method: None,
        structure: None,
        unmatched_lines,
        after_cut,
    });
}

/// The parts, as `(offset, len)`, into which the `len` matching tokens from
/// `a_start` in `a` and `b_start` in `b` fall when every renamed identifier
/// must stand for one name on the other side and no two names for the same
/// one: the whole match when names are not tracked or keep to one mapping.
/// A clone grows past a name that would stand for two ([`window_continues`]),
/// and is cut where the two sides disagree how far back a name occurred
/// last within the part. When the part before that token can be reported,
/// the cut goes right before it, or at the last line start after the
/// earlier occurrence where both parts stay big enough; otherwise right
/// after the earlier occurrence, so the rest is as long as it can be. Parts
/// do not overlap, and share a line only when the cut is not at a line
/// start.
fn consistent_runs(
    a: &PreparedSource,
    a_start: usize,
    b: &PreparedSource,
    b_start: usize,
    len: usize,
    min_tokens: usize,
    min_lines: usize,
) -> Vec<(usize, usize)> {
    if a.renames.is_empty() || b.renames.is_empty() {
        return vec![(0, len)];
    }
    // Whether tokens `from..to` would make a clone big enough to report.
    let reported = |from: usize, to: usize| {
        let (first, last) = (&a.spans[a_start + from].0, &a.spans[a_start + to - 1].1);
        to - from >= min_tokens && last.line.saturating_sub(first.line) as usize >= min_lines
    };
    let mut runs = Vec::new();
    let mut from = 0;
    for k in 0..len {
        let back_a = a.renames.back(a_start + k, a_start + from);
        let back_b = b.renames.back(b_start + k, b_start + from);
        if back_a == back_b {
            continue;
        }
        let nearest = [back_a, back_b]
            .into_iter()
            .filter(|&d| d != 0)
            .min()
            .unwrap_or(1) as usize;
        // Any cut after the earlier occurrence and up to `k` keeps both
        // parts to one mapping.
        let earliest = k + 1 - nearest;
        let cut = match reported(from, k) {
            // Statistics count a line two parts share twice, so the part ends
            // with the line before when it and the rest can still be reported.
            true => (earliest..=k)
                .rev()
                .find(|&c| starts_line(a, a_start + c) && starts_line(b, b_start + c))
                .filter(|&c| reported(from, c) && len - c >= min_tokens)
                .unwrap_or(k),
            false => earliest,
        };
        runs.push((from, cut - from));
        from = cut;
    }
    runs.push((from, len - from));
    runs
}

/// Whether token `i` of `source` is the first on its line.
fn starts_line(source: &PreparedSource, i: usize) -> bool {
    i == 0 || source.spans[i - 1].1.line < source.spans[i].0.line
}

/// Lines of `fragment` that hold no token of `source`.
///
/// Zero for an ordinary file, where the fragment is a contiguous run of code.
/// For an embedded block it is the host language's text sitting between two
/// blocks the fragment happens to span — markdown prose, the template half of
/// a single-file component. That text is not code of this format and must not
/// count as duplicated (issue #1090).
fn host_lines_in(source: &PreparedSource, fragment: &Fragment) -> u32 {
    if !source.embedded {
        return 0;
    }
    let (first, last) = (fragment.range[0] as usize, fragment.range[1] as usize);
    let Some(spans) = source.spans.get(first..=last) else {
        return 0;
    };
    let span = fragment.end.line.saturating_sub(fragment.start.line) + 1;
    let covered = covered_lines(spans.iter().map(|(s, e)| (s.line, e.line)));
    span.saturating_sub(covered)
}

/// Classify a clone pair as exact or renamed (issue #998).
///
/// Sources scanned without a normalization option carry no raw hashes and
/// always yield `Exact`. Otherwise the raw (un-normalized) hashes of both
/// fragments are compared over the run of tokens whose normalized hashes
/// match — the fragment ranges may overshoot the matched window by one
/// token on the secondary path, so the comparison is bounded by the
/// normalized match rather than by the stored range.
fn clone_kind(
    a: &PreparedSource,
    a_range: [u32; 2],
    b: &PreparedSource,
    b_range: [u32; 2],
) -> CloneKind {
    if a.raw_hashes.is_empty() && b.raw_hashes.is_empty() {
        return CloneKind::Exact;
    }
    let (na, ra) = range_slices(a, a_range);
    let (nb, rb) = range_slices(b, b_range);
    let matched = na.iter().zip(nb).take_while(|(x, y)| x == y).count();
    if ra[..matched.min(ra.len())] == rb[..matched.min(rb.len())] {
        CloneKind::Exact
    } else {
        CloneKind::Renamed
    }
}

/// `(normalized, raw)` hash slices for an inclusive token index range.
fn range_slices(p: &PreparedSource, range: [u32; 2]) -> (&[u64], &[u64]) {
    let start = (range[0] as usize).min(p.hashes.len());
    let end = (range[1] as usize + 1).min(p.hashes.len());
    let raw = if p.raw_hashes.is_empty() {
        &p.hashes
    } else {
        &p.raw_hashes
    };
    (&p.hashes[start..end], &raw[start..end])
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
        source_root: None,
        start: first_start.clone(),
        end: last_end.clone(),
        range: [start_idx as u32, end_idx as u32],
        blame: None,
    })
}

// ---------------------------------------------------------------------------
// Deduplication — O(n) FxHashSet + sub-clone suppression
// ---------------------------------------------------------------------------

/// Lowest similarity a gap merge may produce. Below it the merged span
/// would hold more unmatched than matched tokens (one very long inserted
/// line, say), which is not a near-miss copy: the halves stay separate.
pub const MIN_GAP_SIMILARITY: f32 = 0.5;

/// Gap-tolerant merging (issue #999, stage 1).
///
/// Two clones of the same file pair whose fragments follow each other in
/// *both* files with at most `max_gap_lines` unmatched lines in between are
/// merged into one `similar` clone spanning both. `token_count` becomes the
/// number of matched tokens and `similarity` the matched tokens divided by
/// the tokens of the longer merged span; a merge whose similarity would fall
/// below [`MIN_GAP_SIMILARITY`] is refused. Chains merge transitively. The
/// merged clone is `similar` even when its halves were `renamed`, and its
/// `unmatched_lines` hold the gap lines of each fragment so statistics can
/// leave them out. With `max_gap_lines == 0` the input is returned as-is, so
/// default runs never enter this pass.
pub fn merge_gapped_clones(mut clones: Vec<CpdClone>, max_gap_lines: usize) -> Vec<CpdClone> {
    if max_gap_lines == 0 || clones.len() < 2 {
        return clones;
    }
    clones.sort_by(|x, y| {
        pair_key(x)
            .cmp(&pair_key(y))
            .then(x.fragment_a.range[0].cmp(&y.fragment_a.range[0]))
            .then(x.fragment_b.range[0].cmp(&y.fragment_b.range[0]))
    });
    let mut merged: Vec<CpdClone> = Vec::with_capacity(clones.len());
    for clone in clones {
        let extended = match merged.last_mut() {
            Some(last) if pair_key(last) == pair_key(&clone) => {
                merge_into(last, &clone, max_gap_lines)
            }
            _ => false,
        };
        if !extended {
            merged.push(clone);
        }
    }
    merged
}

/// Extend `last` with `next` when both fragments continue within the gap
/// limit and the result clears [`MIN_GAP_SIMILARITY`]. `last.token_count`
/// is the matched-token total of its chain, which is what the exact pass
/// stores for an unmerged clone as well.
fn merge_into(last: &mut CpdClone, next: &CpdClone, max_gap_lines: usize) -> bool {
    // Joined, the parts of a cut clone would pair a name with two names.
    if next.after_cut
        && next.fragment_a.range[0] == last.fragment_a.range[1] + 1
        && next.fragment_b.range[0] == last.fragment_b.range[1] + 1
    {
        return false;
    }
    let Some(step_a) = continuation(&last.fragment_a, &next.fragment_a, max_gap_lines) else {
        return false;
    };
    let Some(step_b) = continuation(&last.fragment_b, &next.fragment_b, max_gap_lines) else {
        return false;
    };
    // Adjacent exact windows may share their boundary tokens; that overlap is
    // matched once.
    let matched = last.token_count
        + next
            .token_count
            .saturating_sub(step_a.overlap.max(step_b.overlap));
    let span_a = next.fragment_a.range[1] - last.fragment_a.range[0] + 1;
    let span_b = next.fragment_b.range[1] - last.fragment_b.range[0] + 1;
    let similarity = matched as f32 / span_a.max(span_b) as f32;
    if similarity < MIN_GAP_SIMILARITY {
        return false;
    }
    last.fragment_a.end = next.fragment_a.end.clone();
    last.fragment_a.range[1] = next.fragment_a.range[1];
    last.fragment_b.end = next.fragment_b.end.clone();
    last.fragment_b.range[1] = next.fragment_b.range[1];
    last.token_count = matched;
    last.similarity = Some(similarity);
    last.similarity_method = Some(SimilarityMethod::Gap);
    last.kind = CloneKind::Similar;
    // The merged span covers both halves and the gap between them, so it
    // inherits what neither half matched as well as the gap itself.
    last.unmatched_lines[0] += next.unmatched_lines[0] + step_a.gap_lines;
    last.unmatched_lines[1] += next.unmatched_lines[1] + step_b.gap_lines;
    true
}

fn pair_key(c: &CpdClone) -> (&str, &str, &str) {
    (
        c.format.as_str(),
        c.fragment_a.source_id.as_str(),
        c.fragment_b.source_id.as_str(),
    )
}

/// How one fragment continues another: the tokens the two windows share at
/// the boundary and the whole lines between them that neither covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Continuation {
    overlap: u32,
    gap_lines: u32,
}

/// `next` extends `prev` when it starts after `prev` starts, ends after
/// `prev` ends, and at most `max_gap_lines` lines lie between them.
fn continuation(prev: &Fragment, next: &Fragment, max_gap_lines: usize) -> Option<Continuation> {
    if next.range[0] <= prev.range[0] || next.range[1] <= prev.range[1] {
        return None;
    }
    let gap_lines = next.start.line.saturating_sub(prev.end.line + 1);
    if gap_lines as usize > max_gap_lines {
        return None;
    }
    Some(Continuation {
        overlap: (prev.range[1] + 1).saturating_sub(next.range[0]),
        gap_lines,
    })
}

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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
    source_a: usize,
    source_b: usize,
    token_a: usize,
    token_b: usize,
}

impl SecondaryOpen {
    /// True when `candidate` extends this open clone by exactly one token on
    /// both sides.
    fn is_continuation(&self, candidate: &Candidate) -> bool {
        self.source_a == candidate.source_a
            && self.source_b == candidate.source_b
            && self.last_token_start_a + 1 == candidate.token_a
            && self.last_token_start_b + 1 == candidate.token_b
    }

    /// Grow the open clone by one token on each side, using `prepared` to
    /// resolve the new endpoint spans.
    fn grow(&mut self, candidate: &Candidate, prepared: &[PreparedSource], min_tokens: usize) {
        self.clone.token_count += 1;
        let end_a = candidate.token_a + min_tokens;
        let end_b = candidate.token_b + min_tokens;
        if let Some(span) = prepared[self.source_a].spans.get(end_a) {
            self.clone.fragment_a.end = span.1.clone();
            self.clone.fragment_a.range[1] = end_a as u32;
        }
        if let Some(span) = prepared[self.source_b].spans.get(end_b) {
            self.clone.fragment_b.end = span.1.clone();
            self.clone.fragment_b.range[1] = end_b as u32;
        }
        self.last_token_start_a = candidate.token_a;
        self.last_token_start_b = candidate.token_b;
    }
}

fn add_secondary_clones(
    repeated_windows: FxHashMap<u64, Vec<Occurrence>>,
    prepared: &[PreparedSource],
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
    clones: &mut Vec<CpdClone>,
) {
    if repeated_windows.is_empty() {
        return;
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
                // Both entered the bucket as a pair that matched.
                if left.source_id == right.source_id && left.token_start == right.token_start {
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
            && current.is_continuation(&candidate)
        {
            current.grow(&candidate, prepared, min_tokens);
            continue;
        }

        flush_secondary_clone(
            open.take(),
            prepared,
            min_tokens,
            min_lines,
            filters,
            clones,
            &mut coverage,
        );

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
            clone: CpdClone::exact(
                prepared[candidate.source_a].format.clone(),
                frag_a,
                frag_b,
                min_tokens as u32,
            ),
            source_a: candidate.source_a,
            source_b: candidate.source_b,
            last_token_start_a: candidate.token_a,
            last_token_start_b: candidate.token_b,
        });
    }

    flush_secondary_clone(
        open.take(),
        prepared,
        min_tokens,
        min_lines,
        filters,
        clones,
        &mut coverage,
    );
}

fn flush_secondary_clone(
    open: Option<SecondaryOpen>,
    prepared: &[PreparedSource],
    min_tokens: usize,
    min_lines: usize,
    filters: &PathFilters,
    clones: &mut Vec<CpdClone>,
    coverage: &mut LineCoverage,
) {
    let Some(oc) = open else {
        return;
    };

    // Path filters: drop clone pairs by fragment location (skip_local, skip_isolated).
    if filters.should_skip_pair(&prepared[oc.source_a], &prepared[oc.source_b]) {
        return;
    }

    let (a, b) = (&prepared[oc.source_a], &prepared[oc.source_b]);
    let start_a = oc.clone.fragment_a.range[0] as usize;
    let start_b = oc.clone.fragment_b.range[0] as usize;
    let matched = oc.last_token_start_a + min_tokens - start_a;
    // A grown clone ends one token past the windows that matched, as in 5.4.
    // That token is checked as well when it matches: it may be a name that
    // stands for two.
    let overrun = |f: &Fragment, start: usize| f.range[1] as usize + 1 - start > matched;
    let len = match overrun(&oc.clone.fragment_a, start_a)
        && overrun(&oc.clone.fragment_b, start_b)
        && a.hashes[start_a + matched] == b.hashes[start_b + matched]
    {
        true => matched + 1,
        false => matched,
    };
    let runs = match a.renames.is_empty() || b.renames.is_empty() {
        true => Vec::new(),
        // A part spans no more lines than the clone, and covers no line it
        // does not, so a clone that falls short is not cut.
        false if fits_secondary(&oc.clone, oc.source_a, oc.source_b, min_lines, coverage) => {
            consistent_runs(a, start_a, b, start_b, len, min_tokens, min_lines)
        }
        false => return,
    };
    if runs.len() < 2 {
        // A clone that keeps to one mapping goes out as the pass grew it.
        finish_secondary_clone(
            oc.clone,
            oc.source_a,
            oc.source_b,
            prepared,
            min_lines,
            clones,
            coverage,
        );
        return;
    }
    // Parts cover only the tokens that matched.
    for (offset, len) in runs {
        if len < min_tokens {
            continue;
        }
        let (from_a, from_b) = (start_a + offset, start_b + offset);
        let (Some(frag_a), Some(frag_b)) = (
            make_fragment(&a.id, &a.spans, from_a, from_a + len - 1),
            make_fragment(&b.id, &b.spans, from_b, from_b + len - 1),
        ) else {
            continue;
        };
        let clone = CpdClone {
            after_cut: offset > 0,
            ..CpdClone::exact(oc.clone.format.clone(), frag_a, frag_b, len as u32)
        };
        finish_secondary_clone(
            clone,
            oc.source_a,
            oc.source_b,
            prepared,
            min_lines,
            clones,
            coverage,
        );
    }
}

/// Whether a secondary clone between `source_a` and `source_b` spans
/// `min_lines` lines and adds a line to what the clones found so far cover.
fn fits_secondary(
    clone: &CpdClone,
    source_a: usize,
    source_b: usize,
    min_lines: usize,
    coverage: &LineCoverage,
) -> bool {
    // min_lines filter: only check fragment A, mirroring jscpd's LinesLengthCloneValidator.
    if min_lines > 0 {
        let lines = clone.fragment_a.end.line as usize - clone.fragment_a.start.line as usize;
        if lines < min_lines {
            return false;
        }
    }
    // Line-coverage filter: skip secondary clones that don't extend existing coverage
    // on either side.  This prevents the report from filling up with dozens of
    // overlapping sub-clones of the same region.
    coverage.extends(source_a, fragment_line_range(&clone.fragment_a))
        && coverage.extends(source_b, fragment_line_range(&clone.fragment_b))
}

/// Report a secondary clone between `source_a` and `source_b`, unless
/// [`fits_secondary`] says otherwise.
fn finish_secondary_clone(
    mut clone: CpdClone,
    source_a: usize,
    source_b: usize,
    prepared: &[PreparedSource],
    min_lines: usize,
    clones: &mut Vec<CpdClone>,
    coverage: &mut LineCoverage,
) {
    if !fits_secondary(&clone, source_a, source_b, min_lines, coverage) {
        return;
    }
    let range_a = fragment_line_range(&clone.fragment_a);
    let range_b = fragment_line_range(&clone.fragment_b);

    let before = clones.len();
    clone.kind = clone_kind(
        &prepared[source_a],
        clone.fragment_a.range,
        &prepared[source_b],
        clone.fragment_b.range,
    );
    // The fragments grew while the clone stayed open, so the host lines they
    // now span are only known here.
    clone.unmatched_lines = [
        host_lines_in(&prepared[source_a], &clone.fragment_a),
        host_lines_in(&prepared[source_b], &clone.fragment_b),
    ];
    clones.push(clone);

    // Insert coverage for newly added clone.
    if clones.len() > before {
        coverage.insert(source_a, range_a);
        coverage.insert(source_b, range_b);
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
        // ponytail: walk existing intervals in order, advancing the low watermark
        // past anything already covered. We extend unless the candidate is
        // already fully covered by an existing interval chain.
        let Some(intervals) = self.ranges_by_source.get(source_idx) else {
            return true;
        };
        let mut cursor = range.0;
        for &(start, end) in intervals {
            if end < cursor {
                continue;
            }
            if start > cursor {
                return true;
            }
            cursor = cursor.max(end.saturating_add(1));
            if cursor > range.1 {
                return false;
            }
        }
        cursor <= range.1
    }

    fn insert(&mut self, source_idx: usize, range: (usize, usize)) {
        // ponytail: merge-into-sorted approach. Keep the per-source vector
        // sorted and merged so `extends` can scan it in one pass; we rebuild
        // it by folding the new range into the existing merged intervals
        // rather than re-sorting the whole list every insert.
        let Some(intervals) = self.ranges_by_source.get_mut(source_idx) else {
            return;
        };
        let mut folded = Vec::with_capacity(intervals.len() + 1);
        let mut pending = Some(range);
        for &(start, end) in intervals.iter() {
            let p = match pending.take() {
                None => {
                    folded.push((start, end));
                    continue;
                }
                Some(p) => p,
            };
            // p is fully before this interval — emit p, then this interval.
            if p.1.saturating_add(1) < start {
                folded.push(p);
                folded.push((start, end));
            }
            // this interval is fully before p — emit it, keep p pending.
            else if end.saturating_add(1) < p.0 {
                folded.push((start, end));
                pending = Some(p);
            }
            // overlapping or adjacent — merge into p, keep pending.
            else {
                pending = Some((p.0.min(start), p.1.max(end)));
            }
        }
        if let Some(p) = pending {
            folded.push(p);
        }
        *intervals = folded;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(hash: u64, raw_hash: u64, line: u32) -> DetectionToken {
        let loc = Location {
            line,
            column: 0,
            offset: line,
        };
        DetectionToken {
            hash,
            raw_hash,
            name: 0,
            start: loc.clone(),
            end: loc,
            range: [line as usize, line as usize + 1],
        }
    }

    /// A clone of `format` between `a` and `b` covering the given token index
    /// ranges (inclusive) and line ranges.
    fn gap_clone(
        a: &str,
        a_tok: [u32; 2],
        a_lines: [u32; 2],
        b: &str,
        b_tok: [u32; 2],
        b_lines: [u32; 2],
    ) -> CpdClone {
        let frag = |id: &str, tok: [u32; 2], lines: [u32; 2]| Fragment {
            source_id: id.to_string(),
            source_root: None,
            start: Location {
                line: lines[0],
                column: 1,
                offset: tok[0],
            },
            end: Location {
                line: lines[1],
                column: 1,
                offset: tok[1],
            },
            range: tok,
            blame: None,
        };
        CpdClone::exact(
            "javascript".to_string(),
            frag(a, a_tok, a_lines),
            frag(b, b_tok, b_lines),
            a_tok[1] - a_tok[0] + 1,
        )
    }

    #[test]
    fn merge_gapped_is_a_no_op_at_zero() {
        let clones = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [12, 21], [6, 9]),
        ];
        let out = merge_gapped_clones(clones.clone(), 0);
        assert_eq!(out, clones);
    }

    #[test]
    fn merge_gapped_joins_adjacent_fragments_within_gap() {
        // a: lines 1-4 then 5-8 (no gap); b: lines 1-4 then 6-9 (one inserted line)
        let clones = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [12, 21], [6, 9]),
        ];
        let out = merge_gapped_clones(clones, 1);
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.kind, CloneKind::Similar);
        assert_eq!(c.token_count, 20, "matched tokens");
        assert_eq!(c.fragment_a.range, [0, 19]);
        assert_eq!(c.fragment_b.range, [0, 21]);
        assert_eq!(c.fragment_a.end.line, 8);
        assert_eq!(c.fragment_b.end.line, 9);
        // 20 matched over the longer span of 22 tokens
        assert!((c.similarity.unwrap() - 20.0 / 22.0).abs() < 1e-6);
    }

    #[test]
    fn merge_gapped_respects_the_line_limit_and_file_pair() {
        let far = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [20, 29], [8, 11]), // 3-line gap in b
        ];
        assert_eq!(merge_gapped_clones(far.clone(), 2).len(), 2);
        assert_eq!(merge_gapped_clones(far, 3).len(), 1);

        let other_pair = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "c", [10, 19], [5, 8]),
        ];
        assert_eq!(merge_gapped_clones(other_pair, 5).len(), 2);
    }

    #[test]
    fn merge_gapped_counts_a_shared_boundary_token_once_and_chains() {
        // second window starts on the last token of the first in `a`
        let clones = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [9, 18], [4, 8], "b", [11, 20], [6, 9]),
            gap_clone("a", [19, 28], [9, 12], "b", [22, 31], [10, 13]),
        ];
        let out = merge_gapped_clones(clones, 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].token_count, 29, "10 + (10 - 1 overlap) + 10");
        assert_eq!(out[0].fragment_a.range, [0, 28]);
        assert_eq!(out[0].fragment_b.range, [0, 31]);
    }

    #[test]
    fn merge_gapped_never_merges_overlapping_or_reordered_fragments() {
        let nested = vec![
            gap_clone("a", [0, 19], [1, 8], "b", [0, 19], [1, 8]),
            gap_clone("a", [5, 9], [3, 4], "b", [5, 9], [3, 4]),
        ];
        assert_eq!(merge_gapped_clones(nested, 5).len(), 2);
        let crossed = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [20, 29], [10, 13]),
            gap_clone("a", [10, 19], [5, 8], "b", [0, 9], [1, 4]),
        ];
        assert_eq!(merge_gapped_clones(crossed, 5).len(), 2);
    }

    #[test]
    fn merge_gapped_refuses_a_gap_wider_than_the_match() {
        // b holds 30 unmatched tokens on one inserted line between two
        // 10-token halves: 20 matched over a 50-token span is 0.4.
        let wide = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [40, 49], [6, 9]),
        ];
        let out = merge_gapped_clones(wide, 1);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|c| c.kind == CloneKind::Exact));
        assert!(out.iter().all(|c| c.similarity.is_none()));
        // 20 matched over a 40-token span is exactly the floor and merges.
        let at_floor = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [30, 39], [6, 9]),
        ];
        let out = merge_gapped_clones(at_floor, 1);
        assert_eq!(out.len(), 1);
        assert!((out[0].similarity.unwrap() - MIN_GAP_SIMILARITY).abs() < 1e-6);
    }

    #[test]
    fn merge_gapped_records_unmatched_lines_per_fragment() {
        // a continues without a gap; b leaves lines 5 and 6 unmatched.
        let clones = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [14, 23], [7, 10]),
        ];
        let out = merge_gapped_clones(clones, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].unmatched_lines, [0, 2]);
        assert_eq!(out[0].fragment_b.end.line, 10);
    }

    #[test]
    fn merge_gapped_reports_renamed_halves_as_similar() {
        let mut clones = vec![
            gap_clone("a", [0, 9], [1, 4], "b", [0, 9], [1, 4]),
            gap_clone("a", [10, 19], [5, 8], "b", [12, 21], [6, 9]),
        ];
        for c in &mut clones {
            c.kind = CloneKind::Renamed;
        }
        let out = merge_gapped_clones(clones, 1);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].kind,
            CloneKind::Similar,
            "similar takes precedence over renamed"
        );
    }

    #[test]
    fn prepared_source_skips_raw_hashes_when_nothing_was_normalized() {
        let tokens = vec![tok(1, 1, 1), tok(2, 2, 2)];
        let p = PreparedSource::from_detection_tokens("a".into(), "js".into(), &tokens);
        assert!(p.raw_hashes.is_empty());
    }

    #[test]
    fn prepared_source_backfills_raw_hashes_from_first_normalized_token() {
        let tokens = vec![tok(1, 1, 1), tok(2, 2, 2), tok(3, 30, 3), tok(4, 4, 4)];
        let p = PreparedSource::from_detection_tokens("a".into(), "js".into(), &tokens);
        assert_eq!(p.raw_hashes, vec![1, 2, 30, 4]);
    }

    #[test]
    fn prepared_source_keeps_the_raw_hash_of_a_normalized_first_token() {
        // A file that starts with an identifier: its raw hash used to give
        // way to the placeholder's, so a clone that differs only there
        // was classified exact.
        let tokens = vec![tok(1, 10, 1), tok(2, 2, 2), tok(1, 30, 3)];
        let p = PreparedSource::from_detection_tokens("a".into(), "js".into(), &tokens);
        assert_eq!(p.raw_hashes, vec![10, 2, 30]);
    }

    #[test]
    fn clone_kind_is_exact_without_raw_hashes_and_renamed_when_raw_differs() {
        let a = PreparedSource::from_detection_tokens(
            "a".into(),
            "js".into(),
            &[tok(1, 1, 1), tok(2, 2, 2), tok(3, 3, 3)],
        );
        let b = PreparedSource::from_detection_tokens(
            "b".into(),
            "js".into(),
            &[tok(1, 1, 1), tok(2, 20, 2), tok(3, 3, 3)],
        );
        assert_eq!(clone_kind(&a, [0, 2], &a, [0, 2]), CloneKind::Exact);
        assert_eq!(clone_kind(&a, [0, 2], &b, [0, 2]), CloneKind::Renamed);
        // the differing token lies outside the compared range
        assert_eq!(clone_kind(&a, [2, 2], &b, [2, 2]), CloneKind::Exact);
        // an overshooting range is bounded by the normalized match
        let c = PreparedSource::from_detection_tokens(
            "c".into(),
            "js".into(),
            &[tok(1, 1, 1), tok(2, 2, 2), tok(9, 90, 3)],
        );
        assert_eq!(clone_kind(&a, [0, 2], &c, [0, 2]), CloneKind::Exact);
    }
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
            bytes: 0,
            id: id.to_string(),
            format: format.to_string(),
            tokens,
        }
    }

    /// A prepared source built straight from hashes and spans, with every
    /// other field at rest — what the detection tests need.
    fn make_prepared(
        id: &str,
        format: &str,
        hashes: Vec<u64>,
        spans: Vec<(Location, Location)>,
    ) -> PreparedSource {
        PreparedSource {
            id: id.to_string(),
            format: format.to_string(),
            hashes,
            spans,
            raw_hashes: Vec::new(),
            renames: Default::default(),
            real_path: String::new(),
            embedded: false,
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

    fn pair_with_js_tokens(min_tokens: usize) -> Vec<CpdClone> {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens);
        detect(&[file_a, file_b], min_tokens)
    }

    #[test]
    fn identical_files_detected_as_clone() {
        assert!(
            !pair_with_js_tokens(5).is_empty(),
            "identical files must produce at least one clone"
        );
    }

    #[test]
    fn min_tokens_threshold_respected() {
        assert!(
            pair_with_js_tokens(100).is_empty(),
            "no clones when min_tokens exceeds file length"
        );
    }

    #[test]
    fn deduplication_ab_ba_collapse() {
        assert_eq!(
            pair_with_js_tokens(5).len(),
            1,
            "symmetric pairs must collapse to 1"
        );
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
    fn cross_format_group_detected() {
        // Inverse of different_formats_not_cross_detected: when two formats
        // are pooled into ONE prepared group (--cross-formats), identical
        // token streams match across formats.
        let to_prepared = |id: &str, format: &str| {
            let tokens = js_tokens_ab();
            let mut hashes = Vec::new();
            let mut spans = Vec::new();
            for t in &tokens {
                hashes.push(token_hash(t.kind.discriminant(), &t.value));
                spans.push((t.start.clone(), t.end.clone()));
            }
            make_prepared(id, format, hashes, spans)
        };
        let group = vec![
            to_prepared("a.js", "javascript"),
            to_prepared("a.ts", "typescript"),
        ];
        let clones = detect_prepared(vec![group], 5, 0, &PathFilters::default());
        assert_eq!(
            clones.len(),
            1,
            "identical token streams in one pool must match across formats"
        );
    }

    /// Issue #1033: three files, scanned in this order.
    ///   a: X
    ///   b: X' T X'' where X' and X'' are X with a different first token
    ///   c: X T Z
    /// While scanning b, the windows spanning "tail of X + T" match nothing
    /// and are stored. While scanning c, the clone anchored on a covers X;
    /// the next window (tail of X + T) matches b's stored occurrence, and a
    /// blind extension would stretch the fragment past a's last token and
    /// drop the clone. The anchor check keeps a↔c at exactly |X| tokens.
    #[test]
    fn open_clone_extends_only_while_its_anchor_continues() {
        let min_tokens = 5;
        let x: Vec<u64> = (100..112).collect(); // 12 tokens
        let t: Vec<u64> = vec![900, 901, 902, 903, 904, 905];
        let z: Vec<u64> = vec![700, 701, 702, 703, 704, 705, 706];
        let renamed = |first: u64| {
            let mut v = x.clone();
            v[0] = first;
            v
        };
        let stream = |parts: &[&[u64]]| -> Vec<u64> { parts.concat() };
        let a = stream(&[&x]);
        let b = stream(&[&renamed(1), &t, &renamed(2)]);
        let c = stream(&[&x, &t, &z]);
        let streams: Vec<(&str, Vec<u64>)> =
            vec![("a", a.clone()), ("b", b.clone()), ("c", c.clone())];
        let to_prepared = |id: &str, hashes: Vec<u64>| {
            let spans = (0..hashes.len())
                .map(|i| {
                    let loc = Location {
                        line: i as u32 + 1,
                        column: 1,
                        offset: i as u32,
                    };
                    (loc.clone(), loc)
                })
                .collect();
            make_prepared(id, "javascript", hashes, spans)
        };
        let group = vec![
            to_prepared("a", a),
            to_prepared("b", b),
            to_prepared("c", c),
        ];
        let clones = detect_prepared(vec![group], min_tokens, 0, &PathFilters::default());
        let a_c: Vec<&CpdClone> = clones
            .iter()
            .filter(|cl| cl.fragment_a.source_id == "a" && cl.fragment_b.source_id == "c")
            .collect();
        assert_eq!(
            a_c.len(),
            1,
            "a↔c must be reported once, got {:?}",
            clones
                .iter()
                .map(|cl| (
                    cl.fragment_a.source_id.as_str(),
                    cl.fragment_a.range,
                    cl.fragment_b.source_id.as_str(),
                    cl.fragment_b.range,
                    cl.token_count
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            a_c[0].token_count,
            x.len() as u32,
            "exactly X, not X plus T"
        );
        assert_eq!(a_c[0].fragment_a.range, [0, x.len() as u32 - 1]);
        assert_eq!(a_c[0].fragment_b.range, [0, x.len() as u32 - 1]);
        // Every reported pair covers identical token runs on both sides.
        let run = |frag: &Fragment| -> &[u64] {
            let hashes = &streams
                .iter()
                .find(|(id, _)| *id == frag.source_id)
                .unwrap()
                .1;
            &hashes[frag.range[0] as usize..=frag.range[1] as usize]
        };
        for cl in &clones {
            assert_eq!(
                run(&cl.fragment_a),
                run(&cl.fragment_b),
                "{}{:?} and {}{:?} must hold the same tokens",
                cl.fragment_a.source_id,
                cl.fragment_a.range,
                cl.fragment_b.source_id,
                cl.fragment_b.range
            );
        }
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

    /// `(file a, file b)` of every clone, in report order.
    fn pairs(clones: &[CpdClone]) -> Vec<(&str, &str)> {
        clones
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.as_str(),
                    c.fragment_b.source_id.as_str(),
                )
            })
            .collect()
    }

    #[test]
    fn three_identical_files_pair_each_copy_with_the_first() {
        let tokens = js_tokens_ab();
        let file_a = make_file("a.js", "javascript", tokens.clone());
        let file_b = make_file("b.js", "javascript", tokens.clone());
        let file_c = make_file("c.js", "javascript", tokens);
        let clones = detect(&[file_a, file_b, file_c], 5);
        assert_eq!(
            pairs(&clones),
            [("a.js", "b.js"), ("a.js", "c.js")],
            "each later copy is reported once, against the first (as jscpd does)"
        );
        for clone in &clones {
            assert_eq!(clone.token_count, 9, "the whole file: {clone:?}");
            assert_eq!(
                (clone.fragment_a.start.line, clone.fragment_a.end.line),
                (1, 3)
            );
            assert_eq!(
                (clone.fragment_b.start.line, clone.fragment_b.end.line),
                (1, 3)
            );
        }
    }

    #[test]
    fn clones_sorted_by_source_and_line() {
        // Files given out of order: the report is ordered all the same.
        let tokens = js_tokens_ab();
        let files: Vec<SourceFile> = ["c.js", "a.js", "b.js"]
            .iter()
            .map(|id| make_file(id, "javascript", tokens.clone()))
            .collect();
        let clones = detect(&files, 5);
        assert!(clones.len() >= 2, "an order needs two clones: {clones:?}");
        let keys: Vec<_> = clones
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.clone(),
                    c.fragment_a.start.line,
                    c.fragment_b.source_id.clone(),
                    c.fragment_b.start.line,
                )
            })
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "clones must be sorted");
    }

    #[test]
    fn filter_path_is_the_real_path_when_it_differs_from_the_id() {
        let mut source = PreparedSource::from_detection_tokens(
            "/repo/corpus/x.js".into(),
            "javascript".into(),
            &[],
        );
        assert_eq!(source.filter_path(), "/repo/corpus/x.js");
        source.real_path = "/elsewhere/x.js".into();
        assert_eq!(source.filter_path(), "/elsewhere/x.js");
    }

    fn isolated(groups: &[&[&str]]) -> Vec<Vec<PathBuf>> {
        groups
            .iter()
            .map(|g| g.iter().map(PathBuf::from).collect())
            .collect()
    }

    #[test]
    fn skip_isolated_drops_pairs_across_group_folders() {
        let groups = isolated(&[&["/repo/packages/a", "/repo/packages/b"]]);
        assert!(should_skip_isolated(
            "/repo/packages/a/src/x.js",
            "/repo/packages/b/src/y.js",
            &groups
        ));
    }

    #[test]
    fn skip_isolated_keeps_pairs_inside_one_folder() {
        let groups = isolated(&[&["/repo/packages/a", "/repo/packages/b"]]);
        assert!(!should_skip_isolated(
            "/repo/packages/a/src/x.js",
            "/repo/packages/a/lib/y.js",
            &groups
        ));
    }

    #[test]
    fn skip_isolated_keeps_pairs_with_one_file_outside_group() {
        let groups = isolated(&[&["/repo/packages/a", "/repo/packages/b"]]);
        assert!(!should_skip_isolated(
            "/repo/packages/a/src/x.js",
            "/repo/globals/y.js",
            &groups
        ));
        assert!(!should_skip_isolated(
            "/repo/globals/x.js",
            "/repo/infra/y.js",
            &groups
        ));
    }

    #[test]
    fn skip_isolated_folders_in_different_groups_do_not_isolate() {
        let groups = isolated(&[
            &["/repo/packages/a", "/repo/packages/b"],
            &["/repo/libs/a", "/repo/libs/b"],
        ]);
        assert!(!should_skip_isolated(
            "/repo/packages/a/x.js",
            "/repo/libs/b/y.js",
            &groups
        ));
        assert!(should_skip_isolated(
            "/repo/libs/a/x.js",
            "/repo/libs/b/y.js",
            &groups
        ));
    }

    #[test]
    fn path_labels_agree_with_should_skip() {
        let roots = [PathBuf::from("/repo/app"), PathBuf::from("/repo/lib")];
        let groups = [vec![
            PathBuf::from("/repo/app/a"),
            PathBuf::from("/repo/app/b"),
        ]];
        for skip_local in [false, true] {
            let filters = PathFilters {
                skip_local,
                scan_roots: &roots,
                isolated_groups: &groups,
            };
            let files = [
                "/repo/app/a/x.ts",
                "/repo/app/b/y.ts",
                "/repo/app/z.ts",
                "/repo/lib/w.rs",
                "/repo/other.js",
            ];
            for a in files {
                for b in files {
                    assert_eq!(
                        filters.label(a).skips(&filters.label(b)),
                        filters.should_skip(a, b),
                        "{a} ~ {b}, skip_local {skip_local}"
                    );
                }
            }
        }
    }

    #[test]
    fn path_filters_combine_skip_local_and_skip_isolated() {
        let scan_roots = vec![PathBuf::from("/repo/shared")];
        let groups = isolated(&[&["/repo/packages/a", "/repo/packages/b"]]);
        let filters = PathFilters {
            skip_local: true,
            scan_roots: &scan_roots,
            isolated_groups: &groups,
        };
        assert!(filters.should_skip("/repo/shared/x.js", "/repo/shared/y.js"));
        assert!(filters.should_skip("/repo/packages/a/x.js", "/repo/packages/b/y.js"));
        assert!(!filters.should_skip("/repo/shared/x.js", "/repo/packages/a/y.js"));
    }

    /// The tokens of a toy language, one per line: a word of one lowercase
    /// letter is an identifier `--ignore-identifiers` renames, any other word
    /// a token of its own.
    fn toy_tokens(code: &str) -> Vec<DetectionToken> {
        let renamed = token_hash(2, "$id");
        code.split_whitespace()
            .enumerate()
            .map(|(i, word)| {
                let raw = token_hash(0, word);
                match word.len() == 1 && word.as_bytes()[0].is_ascii_lowercase() {
                    true => DetectionToken {
                        name: raw,
                        ..tok(renamed, raw, i as u32 + 1)
                    },
                    false => tok(raw, raw, i as u32 + 1),
                }
            })
            .collect()
    }

    /// A source of [`toy_tokens`]. With `consistent`, its names are matched
    /// consistently.
    fn renamed_source(id: &str, code: &str, consistent: bool) -> PreparedSource {
        let tokens = toy_tokens(code);
        let mut source = PreparedSource::from_detection_tokens(id.into(), "toy".into(), &tokens);
        if consistent {
            source.track_renames(&tokens);
        }
        source
    }

    /// The two files of every clone, in order, with the token range of the first.
    fn clone_pairs(clones: &[CpdClone]) -> Vec<(String, String, [u32; 2])> {
        let mut pairs: Vec<_> = clones
            .iter()
            .map(|c| {
                let (a, b) = (&c.fragment_a.source_id, &c.fragment_b.source_id);
                match a <= b {
                    true => (a.clone(), b.clone(), c.fragment_a.range),
                    false => (b.clone(), a.clone(), c.fragment_b.range),
                }
            })
            .collect();
        pairs.sort();
        pairs
    }

    #[test]
    fn consistent_matching_pairs_a_name_with_one_name_only() {
        // Issue #1217: `x` everywhere has another data flow than a, b and c.
        let one = "d = a + b * c ; e = d - a / b ; f = e * c + d ;";
        let two = "s = p + q * r ; t = s - p / q ; u = t * r + s ;";
        let three = "x = x + x * x ; x = x - x / x ; x = x * x + x ;";
        let detect = |consistent| {
            let group = vec![
                renamed_source("one", one, consistent),
                renamed_source("two", two, consistent),
                renamed_source("three", three, consistent),
            ];
            detect_prepared(vec![group], 10, 0, &PathFilters::default())
        };
        let blind = clone_pairs(&detect(false));
        assert!(
            blind
                .iter()
                .any(|(a, b, _)| (a.as_str(), b.as_str()) == ("one", "three")),
            "blind matching pairs any names: {blind:?}"
        );
        let consistent = detect(true);
        assert_eq!(
            clone_pairs(&consistent),
            [("one".to_string(), "two".to_string(), [0, 23])]
        );
        assert_eq!(consistent[0].kind, CloneKind::Renamed);
    }

    /// Two runs of 17 tokens: `a` comes back after the first one, where the
    /// other side has `p` and then `q`.
    const FAR: (&str, &str) = (
        "a = 0 ; + 1 + 2 + 3 + 4 + 5 + 6 ; a = 0 ; - 1 - 2 - 3 - 4 - 5 - 6 ;",
        "p = 0 ; + 1 + 2 + 3 + 4 + 5 + 6 ; q = 0 ; - 1 - 2 - 3 - 4 - 5 - 6 ;",
    );

    #[test]
    fn a_name_matched_two_ways_further_apart_than_a_window_cuts_the_clone() {
        // No window of 10 tokens holds both `a`s, so every window matches;
        // the clone as a whole pairs `a` with two names and is cut there.
        let detect = |consistent| {
            let group = vec![
                renamed_source("one", FAR.0, consistent),
                renamed_source("two", FAR.1, consistent),
            ];
            clone_pairs(&detect_prepared(
                vec![group],
                10,
                0,
                &PathFilters::default(),
            ))
        };
        assert_eq!(
            detect(false),
            [("one".to_string(), "two".to_string(), [0, 33])]
        );
        assert_eq!(
            detect(true),
            [
                ("one".to_string(), "two".to_string(), [0, 16]),
                ("one".to_string(), "two".to_string(), [17, 33]),
            ]
        );
    }

    #[test]
    fn a_clone_is_cut_where_a_name_repeats_on_one_side_only() {
        // `a` comes back 4 tokens later, where `two` has `q`. The window of 5
        // that holds both `a`s matches no window of `two`, but the clone
        // grows on and is cut there, into parts that do not overlap.
        let group = vec![
            renamed_source("one", "+ 1 ; a + 2 ; a + 3 ; + 4 ;", true),
            renamed_source("two", "+ 1 ; p + 2 ; q + 3 ; + 4 ;", true),
        ];
        assert_eq!(
            clone_pairs(&detect_prepared(vec![group], 5, 0, &PathFilters::default())),
            [
                ("one".to_string(), "two".to_string(), [0, 6]),
                ("one".to_string(), "two".to_string(), [7, 13]),
            ]
        );
    }

    #[test]
    fn names_come_from_the_name_not_the_raw_hash() {
        // `--ignore-annotations` folds a dropped decorator into the raw hash
        // of the token before it; that token keeps its name.
        let mut tokens = toy_tokens("a + a");
        tokens[2].raw_hash ^= 1;
        let mut source = PreparedSource::from_detection_tokens("one".into(), "toy".into(), &tokens);
        source.track_renames(&tokens);
        assert_eq!(source.renames.back(2, 0), 2);
    }

    #[test]
    fn consistent_window_hashes_roll_like_fresh_ones() {
        let code = "a + b a ; c b ( a ) d a e b + c a f g a h ( a b ) ; d d a";
        let source = renamed_source("one", code, true);
        let tokens = source.hashes.len();
        for len in [1, 2, 3, 5, 8] {
            let powers: Vec<u64> = (0..len).map(base_pow).collect();
            let window_power = base_pow(len - 1);
            let mut next = Vec::new();
            source.renames.next_into(&mut next);
            let mut hash = consistent_window_hash(&source, 0, len);
            for start in 1..=tokens - len {
                hash = roll_consistent(&source, hash, start, len, window_power, &next, &powers);
                assert_eq!(
                    hash,
                    consistent_window_hash(&source, start, len),
                    "window of {len} from {start}"
                );
            }
        }
    }

    #[test]
    fn windows_match_only_when_their_names_repeat_alike() {
        let at = |source_id| Occurrence {
            source_id,
            token_start: 0,
        };
        let windows = |a: &str, b: &str, consistent| {
            let prepared = vec![
                renamed_source("one", a, consistent),
                renamed_source("two", b, consistent),
            ];
            windows_match(at(0), at(1), &prepared, 5)
        };
        assert!(windows("a + a ; b", "p + p ; q", true));
        assert!(
            !windows("a + a ; b", "p + q ; r", true),
            "a repeats, p does not"
        );
        assert!(
            !windows("a + b ; a", "p + q ; q", true),
            "a comes back, q does"
        );
        assert!(windows("a + a ; b", "p + q ; r", false), "blind: any name");
    }

    #[test]
    fn consistent_runs_cut_where_a_name_would_stand_for_two() {
        let runs = |a: &str, b: &str, min_tokens| {
            let (one, two) = (
                renamed_source("one", a, true),
                renamed_source("two", b, true),
            );
            consistent_runs(&one, 0, &two, 0, one.hashes.len(), min_tokens, 0)
        };
        assert_eq!(runs(FAR.0, FAR.1, 10), [(0, 17), (17, 17)]);
        // Two names for one: `a` and `b` both stand for `p`.
        assert_eq!(runs("a + b", "p + p", 1), [(0, 2), (2, 1)]);
        // `a` stands for `p` once, then for `q`. When the part before the
        // second `a` is too small to report, the cut goes right after the
        // first one, so the rest is as long as it can be.
        let (one, two) = (
            "x ( a ) ; f ( a ) ; g ( a ) ;",
            "x ( p ) ; f ( q ) ; g ( q ) ;",
        );
        assert_eq!(runs(one, two, 5), [(0, 7), (7, 8)]);
        assert_eq!(runs(one, two, 8), [(0, 3), (3, 12)]);
        // A clone that keeps to one mapping stays whole, and so does every
        // clone of sources whose names are not tracked.
        assert_eq!(runs("a + b", "p + q", 1), [(0, 3)]);
        let (blind, two) = (
            renamed_source("blind", "a + b", false),
            renamed_source("two", "p + p", true),
        );
        assert_eq!(consistent_runs(&blind, 0, &two, 0, 3, 1, 0), [(0, 3)]);
    }

    #[test]
    fn a_cut_goes_at_the_start_of_a_line_where_it_can() {
        // `a` stands for `p` in `f` and for `q` in `g`; the clash is the
        // third token of line 4.
        let source = |id: &str, code: &str| {
            let lines = code
                .lines()
                .enumerate()
                .flat_map(|(n, words)| words.split_whitespace().map(move |_| n as u32 + 1));
            let tokens: Vec<DetectionToken> = toy_tokens(code)
                .into_iter()
                .zip(lines)
                .map(|(t, line)| DetectionToken {
                    name: t.name,
                    ..tok(t.hash, t.raw_hash, line)
                })
                .collect();
            let mut source =
                PreparedSource::from_detection_tokens(id.into(), "toy".into(), &tokens);
            source.track_renames(&tokens);
            source
        };
        let one = source(
            "one",
            "f ( a ) {\nreturn a + 1 ;\n}\ng ( a ) {\nreturn a + 2 ;\n}",
        );
        let two = source(
            "two",
            "f ( p ) {\nreturn p + 1 ;\n}\ng ( q ) {\nreturn q + 2 ;\n}",
        );
        let runs = |min_tokens| consistent_runs(&one, 0, &two, 0, 22, min_tokens, 0);
        assert_eq!(runs(5), [(0, 11), (11, 11)]);
        // Ended with line 3, the part before would be too small.
        assert_eq!(runs(12), [(0, 13), (13, 9)]);
    }

    /// The clones that flushing a secondary clone of `prepared[0]` and
    /// `prepared[1]` reports, grown as [`SecondaryOpen::grow`] grows it from
    /// the windows of 10 tokens at 0 to the windows at `last`.
    fn flush_secondary(prepared: &[PreparedSource], last: usize) -> Vec<CpdClone> {
        let fragment = |i: usize| {
            let spans = &prepared[i].spans;
            make_fragment(&prepared[i].id, spans, 0, (last + 10).min(spans.len() - 1)).unwrap()
        };
        let open = SecondaryOpen {
            clone: CpdClone::exact("toy", fragment(0), fragment(1), 10 + last as u32),
            source_a: 0,
            source_b: 1,
            last_token_start_a: last,
            last_token_start_b: last,
        };
        let mut clones = Vec::new();
        let mut coverage = LineCoverage::from_clones(prepared, &clones);
        flush_secondary_clone(
            Some(open),
            prepared,
            10,
            0,
            &PathFilters::default(),
            &mut clones,
            &mut coverage,
        );
        clones
    }

    #[test]
    fn secondary_clones_are_cut_like_primary_ones() {
        let far = |b: &str| {
            vec![
                renamed_source("one", FAR.0, true),
                renamed_source("two", b, true),
            ]
        };
        let clones = flush_secondary(&far(FAR.1), 24);
        assert_eq!(
            clone_pairs(&clones),
            [
                ("one".to_string(), "two".to_string(), [0, 16]),
                ("one".to_string(), "two".to_string(), [17, 33]),
            ]
        );
        assert!(clones.iter().all(|c| c.token_count == 17));
        // A clone that keeps to one mapping goes out as the pass grew it,
        // with the token past the last window.
        let clones = flush_secondary(&far(&FAR.1.replace('q', "p")), 20);
        assert_eq!(
            clone_pairs(&clones),
            [("one".to_string(), "two".to_string(), [0, 30])]
        );
        // Unless that token is a name that stands for two.
        let prepared = vec![
            renamed_source("one", "a = 0 ; + 1 + 2 + 3 + 4 + 5 + 6 ; a", true),
            renamed_source("two", "p = 0 ; + 1 + 2 + 3 + 4 + 5 + 6 ; q", true),
        ];
        assert_eq!(
            clone_pairs(&flush_secondary(&prepared, 7)),
            [("one".to_string(), "two".to_string(), [0, 16])]
        );
    }

    #[test]
    fn gap_merging_keeps_the_parts_of_a_cut_clone_apart() {
        let group = vec![
            renamed_source("one", FAR.0, true),
            renamed_source("two", FAR.1, true),
        ];
        let parts = detect_prepared(vec![group], 10, 0, &PathFilters::default());
        let after_cut: Vec<u32> = parts
            .iter()
            .filter(|c| c.after_cut)
            .map(|c| c.fragment_a.range[0])
            .collect();
        assert_eq!((parts.len(), after_cut), (2, vec![17]));
        let merged = merge_gapped_clones(parts.clone(), 1);
        assert_eq!(merged.len(), 2, "{merged:#?}");
        assert!(merged.iter().all(|c| c.kind != CloneKind::Similar));
        // Clones that touch for another reason are joined, as before.
        let touching = parts
            .into_iter()
            .map(|c| CpdClone {
                after_cut: false,
                ..c
            })
            .collect();
        assert_eq!(merge_gapped_clones(touching, 1).len(), 1);
    }
}
