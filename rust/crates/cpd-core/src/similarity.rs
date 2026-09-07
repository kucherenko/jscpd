//! Function-level similarity (issue #999, stage 2).
//!
//! Each function is summarized by the bag of `k`-grams over the pre-order
//! sequence of its AST node types ("shingles"). Two functions are similar
//! when the weighted Jaccard index of their shingle bags reaches the
//! configured threshold. Candidate pairs come from MinHash + LSH banding so
//! the search stays close to linear in the number of functions; the exact
//! bag Jaccard is only computed for candidates.
//!
//! Node *types* only: identifier names and literal values do not take part,
//! so a renamed copy scores 1.0 and an edited copy scores by how much of
//! its structure survived. Positions always reference the original source.
//!
//! The scoring is grammar-agnostic: node-type ids are opaque `u16`s from
//! whichever extractor produced them (`cpd_tokenizer::functions`), and a
//! signature records its `grammar` so functions are only compared within
//! one grammar. Adding a language means adding an extractor, not touching
//! this module.

use crate::detect::PreparedSource;
use crate::models::{CloneKind, CpdClone, Fragment, Location, SimilarityMethod};
use rustc_hash::{FxHashMap, FxHashSet};

/// Shingle length over the node-type sequence.
pub const SHINGLE_K: usize = 4;
/// MinHash signature size; `BANDS * ROWS` must equal it.
pub const MINHASH_SIZE: usize = 64;
const BANDS: usize = 16;
const ROWS: usize = 4;
const _: () = assert!(BANDS * ROWS == MINHASH_SIZE);
/// Buckets larger than this are truncated before pairing: a bucket that
/// size means hundreds of structurally identical functions, and the first
/// members already carry the signal.
const MAX_BUCKET: usize = 256;

/// Structural summary of one function, method or arrow function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    /// Grammar that produced the node-type sequence; pairs are only formed
    /// within one grammar.
    pub grammar: &'static str,
    /// Declared or inferred name (`<arrow>` / `<anonymous>` when none).
    pub name: String,
    pub start: Location,
    pub end: Location,
    /// Inclusive detection-token index range inside the owning source.
    pub range: [u32; 2],
    /// Detection tokens covered by the function.
    pub token_count: u32,
    /// Sorted bag of shingle hashes.
    pub shingles: Vec<u64>,
    pub minhash: [u64; MINHASH_SIZE],
}

impl FunctionSig {
    /// Build a signature from a node-type sequence and the owning source's
    /// token spans. Returns `None` when no detection token lies inside the
    /// function's byte range (comment-only or type-only bodies).
    pub fn build(
        grammar: &'static str,
        name: String,
        start: Location,
        end: Location,
        kinds: &[u16],
        spans: &[(Location, Location)],
    ) -> Option<Self> {
        let first = spans.partition_point(|(s, _)| s.offset < start.offset);
        let last = spans.partition_point(|(_, e)| e.offset <= end.offset);
        if first >= last {
            return None;
        }
        let shingles = shingles_from_kinds(kinds, SHINGLE_K);
        if shingles.is_empty() {
            return None;
        }
        let minhash = minhash(&shingles);
        Some(Self {
            grammar,
            name,
            start,
            end,
            range: [first as u32, (last - 1) as u32],
            token_count: (last - first) as u32,
            shingles,
            minhash,
        })
    }

    /// Lines spanned, in jscpd's `end - start` convention.
    pub fn line_span(&self) -> u32 {
        self.end.line.saturating_sub(self.start.line)
    }
}

/// Hash every `k`-gram of `kinds`; the result is sorted so it can be used as
/// a multiset by [`bag_jaccard`].
pub fn shingles_from_kinds(kinds: &[u16], k: usize) -> Vec<u64> {
    if kinds.len() < k {
        return Vec::new();
    }
    let mut out: Vec<u64> = kinds
        .windows(k)
        .map(|w| {
            w.iter().fold(0xcbf2_9ce4_8422_2325u64, |acc, &t| {
                (acc ^ u64::from(t)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
        .collect();
    out.sort_unstable();
    out
}

/// Weighted (multiset) Jaccard index of two sorted shingle bags.
pub fn bag_jaccard(a: &[u64], b: &[u64]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let (mut i, mut j) = (0usize, 0usize);
    let (mut inter, mut union) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
        }
        union += 1;
    }
    union += (a.len() - i) + (b.len() - j);
    inter as f32 / union as f32
}

/// MinHash signature over the *distinct* shingles of a sorted bag.
pub fn minhash(sorted_shingles: &[u64]) -> [u64; MINHASH_SIZE] {
    let mut sig = [u64::MAX; MINHASH_SIZE];
    let mut prev: Option<u64> = None;
    for &s in sorted_shingles {
        if prev == Some(s) {
            continue;
        }
        prev = Some(s);
        for (i, slot) in sig.iter_mut().enumerate() {
            let h = mix(s, i as u64);
            if h < *slot {
                *slot = h;
            }
        }
    }
    sig
}

#[inline]
fn mix(h: u64, i: u64) -> u64 {
    // splitmix64 finalizer over (shingle, hash index): cheap and well mixed.
    let mut z = h ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The functions of one source, as needed by the similarity search.
#[derive(Debug, Clone)]
pub struct FunctionSource {
    pub id: String,
    pub format: String,
    pub functions: Vec<FunctionSig>,
}

/// Pull the function signatures out of prepared sources (clones only the
/// sources that carry any, so a run without `--similarity` copies nothing).
pub fn collect_function_sources(prepared: &[PreparedSource]) -> Vec<FunctionSource> {
    prepared
        .iter()
        .filter(|p| !p.functions.is_empty())
        .map(|p| FunctionSource {
            id: p.id.clone(),
            format: p.format.clone(),
            functions: p.functions.clone(),
        })
        .collect()
}

/// LSH index over the functions of many sources. Owns its sources so a
/// long-lived holder (the MCP server) can build it once per scan and answer
/// any number of queries from it.
pub struct SimilarityIndex {
    sources: Vec<FunctionSource>,
    /// (source index, function index) per item.
    items: Vec<(usize, usize)>,
    buckets: FxHashMap<(u8, u64), Vec<usize>>,
    min_tokens: u32,
    min_lines: u32,
}

impl SimilarityIndex {
    /// Index every function with at least `min_tokens` tokens and a line
    /// span of at least `min_lines` (jscpd's usual clone thresholds).
    pub fn build(sources: Vec<FunctionSource>, min_tokens: usize, min_lines: usize) -> Self {
        let mut items = Vec::new();
        let mut buckets: FxHashMap<(u8, u64), Vec<usize>> = FxHashMap::default();
        for (si, src) in sources.iter().enumerate() {
            for (fi, f) in src.functions.iter().enumerate() {
                if !Self::eligible(f, min_tokens as u32, min_lines as u32) {
                    continue;
                }
                let item = items.len();
                items.push((si, fi));
                for (band, key) in band_keys(&f.minhash) {
                    buckets.entry((band, key)).or_default().push(item);
                }
            }
        }
        Self {
            sources,
            items,
            buckets,
            min_tokens: min_tokens as u32,
            min_lines: min_lines as u32,
        }
    }

    /// The indexed sources, in the order they were given.
    pub fn sources(&self) -> &[FunctionSource] {
        &self.sources
    }

    fn eligible(f: &FunctionSig, min_tokens: u32, min_lines: u32) -> bool {
        f.token_count >= min_tokens && f.line_span() >= min_lines
    }

    fn sig(&self, item: usize) -> &FunctionSig {
        let (si, fi) = self.items[item];
        &self.sources[si].functions[fi]
    }

    /// Indexed functions structurally similar to `query`, best first.
    /// Returns `(source index, function index, similarity)`.
    pub fn query(&self, query: &FunctionSig, threshold: f32) -> Vec<(usize, usize, f32)> {
        if !Self::eligible(query, self.min_tokens, self.min_lines) {
            return Vec::new();
        }
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        let mut hits = Vec::new();
        for (band, key) in band_keys(&query.minhash) {
            let Some(bucket) = self.buckets.get(&(band, key)) else {
                continue;
            };
            for &item in bucket.iter().take(MAX_BUCKET) {
                if !seen.insert(item) {
                    continue;
                }
                let cand = self.sig(item);
                if let Some(sim) = score(query, cand, threshold) {
                    let (si, fi) = self.items[item];
                    hits.push((si, fi, sim));
                }
            }
        }
        hits.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        hits
    }

    /// All similar pairs among the indexed functions, as `similar` clones.
    /// Pairs already covered by a clone in `existing` (an exact or renamed
    /// match spanning both functions) are left out so nothing is reported
    /// twice.
    pub fn all_pairs(&self, threshold: f32, existing: &[CpdClone]) -> Vec<CpdClone> {
        let mut pairs: FxHashSet<(usize, usize)> = FxHashSet::default();
        for bucket in self.buckets.values() {
            let members = &bucket[..bucket.len().min(MAX_BUCKET)];
            for (x, &a) in members.iter().enumerate() {
                for &b in &members[x + 1..] {
                    pairs.insert((a.min(b), a.max(b)));
                }
            }
        }
        let mut clones: Vec<CpdClone> = pairs
            .into_iter()
            .filter_map(|(a, b)| {
                let (sa, fa) = self.items[a];
                let (sb, fb) = self.items[b];
                let (fa_sig, fb_sig) = (self.sig(a), self.sig(b));
                if sa == sb && nested(fa_sig, fb_sig) {
                    return None;
                }
                let sim = score(fa_sig, fb_sig, threshold)?;
                let (src_a, src_b) = (&self.sources[sa], &self.sources[sb]);
                if covered_by_existing(src_a, fa_sig, src_b, fb_sig, existing) {
                    return None;
                }
                let _ = (fa, fb);
                Some(make_clone(src_a, fa_sig, src_b, fb_sig, sim))
            })
            .collect();
        clones.sort_by(|x, y| {
            x.fragment_a
                .source_id
                .cmp(&y.fragment_a.source_id)
                .then(x.fragment_a.start.line.cmp(&y.fragment_a.start.line))
                .then(x.fragment_b.source_id.cmp(&y.fragment_b.source_id))
                .then(x.fragment_b.start.line.cmp(&y.fragment_b.start.line))
        });
        clones
    }
}

/// Find similar function pairs across `sources` (issue #999, stage 2).
pub fn find_similar_functions(
    sources: Vec<FunctionSource>,
    threshold: f32,
    min_tokens: usize,
    min_lines: usize,
    existing: &[CpdClone],
) -> Vec<CpdClone> {
    if sources.is_empty() {
        return Vec::new();
    }
    SimilarityIndex::build(sources, min_tokens, min_lines).all_pairs(threshold, existing)
}

fn band_keys(minhash: &[u64; MINHASH_SIZE]) -> impl Iterator<Item = (u8, u64)> + '_ {
    minhash.chunks(ROWS).enumerate().map(|(band, rows)| {
        let key = rows.iter().fold(0x9E37_79B9_7F4A_7C15u64, |acc, &r| {
            mix(acc ^ r, band as u64)
        });
        (band as u8, key)
    })
}

/// Exact score for a candidate pair, `None` below `threshold`. The size
/// ratio bounds the Jaccard index from above, so it is checked first.
fn score(a: &FunctionSig, b: &FunctionSig, threshold: f32) -> Option<f32> {
    if a.grammar != b.grammar {
        return None;
    }
    let (small, large) = if a.shingles.len() <= b.shingles.len() {
        (a.shingles.len(), b.shingles.len())
    } else {
        (b.shingles.len(), a.shingles.len())
    };
    if (small as f32 / large as f32) < threshold {
        return None;
    }
    let sim = bag_jaccard(&a.shingles, &b.shingles);
    (sim >= threshold).then_some(sim)
}

fn nested(a: &FunctionSig, b: &FunctionSig) -> bool {
    (a.range[0] <= b.range[0] && b.range[1] <= a.range[1])
        || (b.range[0] <= a.range[0] && a.range[1] <= b.range[1])
}

/// True when an existing clone between the same two sources already spans
/// at least 90% of the lines of both functions.
fn covered_by_existing(
    src_a: &FunctionSource,
    a: &FunctionSig,
    src_b: &FunctionSource,
    b: &FunctionSig,
    existing: &[CpdClone],
) -> bool {
    let covers = |frag: &Fragment, f: &FunctionSig| {
        let lo = frag.start.line.max(f.start.line);
        let hi = frag.end.line.min(f.end.line);
        if hi < lo {
            return false;
        }
        let overlap = hi - lo + 1;
        let span = f.end.line - f.start.line + 1;
        overlap as f32 >= 0.9 * span as f32
    };
    existing.iter().any(|c| {
        (c.fragment_a.source_id == src_a.id
            && c.fragment_b.source_id == src_b.id
            && covers(&c.fragment_a, a)
            && covers(&c.fragment_b, b))
            || (c.fragment_a.source_id == src_b.id
                && c.fragment_b.source_id == src_a.id
                && covers(&c.fragment_a, b)
                && covers(&c.fragment_b, a))
    })
}

fn make_clone(
    src_a: &FunctionSource,
    a: &FunctionSig,
    src_b: &FunctionSource,
    b: &FunctionSig,
    similarity: f32,
) -> CpdClone {
    let frag = |src: &FunctionSource, f: &FunctionSig| Fragment {
        source_id: src.id.clone(),
        source_root: None,
        start: f.start.clone(),
        end: f.end.clone(),
        range: f.range,
        blame: None,
    };
    // Deterministic fragment order: by source id, then position.
    let a_first = (src_a.id.as_str(), a.start.line) <= (src_b.id.as_str(), b.start.line);
    let (fa, fb) = if a_first {
        (frag(src_a, a), frag(src_b, b))
    } else {
        (frag(src_b, b), frag(src_a, a))
    };
    CpdClone {
        format: src_a.format.clone(),
        fragment_a: fa,
        fragment_b: fb,
        token_count: a.token_count.min(b.token_count),
        is_new: false,
        kind: CloneKind::Similar,
        similarity: Some(similarity),
        similarity_method: Some(SimilarityMethod::Ast),
        unmatched_lines: [0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(line: u32, offset: u32) -> Location {
        Location {
            line,
            column: 0,
            offset,
        }
    }

    fn spans(n: u32) -> Vec<(Location, Location)> {
        (0..n)
            .map(|i| (loc(i + 1, i * 10), loc(i + 1, i * 10 + 5)))
            .collect()
    }

    fn sig(name: &str, kinds: &[u16], first_tok: u32, last_tok: u32) -> FunctionSig {
        let spans = spans(last_tok + 1);
        FunctionSig::build(
            "test",
            name.into(),
            loc(first_tok + 1, first_tok * 10),
            loc(last_tok + 1, last_tok * 10 + 5),
            kinds,
            &spans,
        )
        .unwrap()
    }

    #[test]
    fn shingles_are_order_sensitive_and_sorted() {
        let a = shingles_from_kinds(&[1, 2, 3, 4, 5], 4);
        let b = shingles_from_kinds(&[5, 4, 3, 2, 1], 4);
        assert_eq!(a.len(), 2);
        assert_ne!(a, b);
        assert!(a.windows(2).all(|w| w[0] <= w[1]));
        assert!(shingles_from_kinds(&[1, 2, 3], 4).is_empty());
    }

    #[test]
    fn bag_jaccard_counts_multiplicity() {
        assert_eq!(bag_jaccard(&[1, 2, 3], &[1, 2, 3]), 1.0);
        assert_eq!(bag_jaccard(&[1, 1, 2], &[1, 2, 2]), 0.5);
        assert_eq!(bag_jaccard(&[1, 2], &[3, 4]), 0.0);
        assert_eq!(bag_jaccard(&[], &[]), 0.0);
    }

    #[test]
    fn minhash_of_equal_sets_is_equal_and_of_disjoint_sets_differs() {
        let a = minhash(&[1, 2, 3, 3, 4]);
        let b = minhash(&[1, 2, 3, 4]);
        assert_eq!(a, b, "multiplicity does not affect the signature");
        let c = minhash(&[10, 20, 30, 40]);
        assert_ne!(a, c);
    }

    #[test]
    fn build_maps_bytes_to_token_range_and_rejects_empty_bodies() {
        let s = sig("f", &[1, 2, 3, 4, 5, 6], 2, 7);
        assert_eq!(s.range, [2, 7]);
        assert_eq!(s.token_count, 6);
        assert_eq!(s.line_span(), 5);
        let spans = spans(4);
        assert!(
            FunctionSig::build(
                "test",
                "g".into(),
                loc(9, 900),
                loc(9, 950),
                &[1, 2, 3, 4],
                &spans
            )
            .is_none()
        );
    }

    fn kinds(seed: u16, n: usize) -> Vec<u16> {
        (0..n).map(|i| ((i as u16 * 7 + seed) % 23) + 1).collect()
    }

    #[test]
    fn index_finds_edited_copies_and_ignores_unrelated_functions() {
        let base = kinds(1, 80);
        let mut edited = base.clone();
        edited.insert(40, 99); // one inserted node
        edited[10] = 98; // one changed node
        // A genuinely different structure, not a shifted copy of `base`.
        let other: Vec<u16> = (0..80u16).map(|i| (i * i * 3 + 11) % 29 + 1).collect();
        let sources = vec![
            FunctionSource {
                id: "a.js".into(),
                format: "javascript".into(),
                functions: vec![sig("base", &base, 0, 60)],
            },
            FunctionSource {
                id: "b.js".into(),
                format: "javascript".into(),
                functions: vec![sig("edited", &edited, 0, 62), sig("other", &other, 70, 140)],
            },
        ];
        let clones = find_similar_functions(sources.clone(), 0.75, 10, 3, &[]);
        assert_eq!(clones.len(), 1, "{clones:?}");
        let c = &clones[0];
        assert_eq!(c.kind, CloneKind::Similar);
        assert_eq!(c.fragment_a.source_id, "a.js");
        assert_eq!(c.fragment_b.source_id, "b.js");
        assert_eq!(c.fragment_b.start.line, 1);
        let sim = c.similarity.unwrap();
        // two node edits in 80 nodes break 8 of the 77 shingles
        assert!(sim > 0.75 && sim < 0.9, "got {sim}");
        assert_eq!(c.token_count, 61);
        assert!(find_similar_functions(sources.clone(), 0.99, 10, 3, &[]).is_empty());
    }

    #[test]
    fn index_skips_nested_functions_small_functions_and_covered_pairs() {
        let base = kinds(1, 80);
        let outer = sig("outer", &base, 0, 60);
        let inner = sig("inner", &base, 10, 50);
        let same_file = vec![FunctionSource {
            id: "a.js".into(),
            format: "javascript".into(),
            functions: vec![outer.clone(), inner],
        }];
        assert!(find_similar_functions(same_file.clone(), 0.5, 10, 3, &[]).is_empty());

        let two = vec![
            FunctionSource {
                id: "a.js".into(),
                format: "javascript".into(),
                functions: vec![outer.clone()],
            },
            FunctionSource {
                id: "b.js".into(),
                format: "javascript".into(),
                functions: vec![sig("copy", &base, 0, 60)],
            },
        ];
        assert_eq!(
            find_similar_functions(two.clone(), 0.5, 10, 3, &[]).len(),
            1
        );
        assert!(
            find_similar_functions(two.clone(), 0.5, 100, 3, &[]).is_empty(),
            "min_tokens"
        );
        assert!(
            find_similar_functions(two.clone(), 0.5, 10, 100, &[]).is_empty(),
            "min_lines"
        );

        let mut exact = make_clone(&two[0], &outer, &two[1], &two[1].functions[0], 1.0);
        exact.kind = CloneKind::Exact;
        exact.similarity = None;
        assert!(
            find_similar_functions(two.clone(), 0.5, 10, 3, &[exact]).is_empty(),
            "already reported"
        );
    }

    #[test]
    fn functions_of_different_grammars_are_never_paired() {
        let base = kinds(1, 80);
        let mut foreign = sig("copy", &base, 0, 60);
        foreign.grammar = "tree-sitter-python";
        let sources = vec![
            FunctionSource {
                id: "a.js".into(),
                format: "javascript".into(),
                functions: vec![sig("orig", &base, 0, 60)],
            },
            FunctionSource {
                id: "b.py".into(),
                format: "python".into(),
                functions: vec![foreign],
            },
        ];
        assert!(find_similar_functions(sources, 0.5, 10, 3, &[]).is_empty());
    }

    #[test]
    fn query_returns_best_match_first() {
        let base = kinds(1, 80);
        let mut near = base.clone();
        near[3] = 99;
        let mut far = base.clone();
        for k in far.iter_mut().take(20) {
            *k = 99;
        }
        let sources = vec![FunctionSource {
            id: "lib.js".into(),
            format: "javascript".into(),
            functions: vec![sig("far", &far, 0, 60), sig("near", &near, 70, 130)],
        }];
        let index = SimilarityIndex::build(sources, 10, 3);
        let hits = index.query(&sig("q", &base, 0, 60), 0.5);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].1, 1, "near first");
        assert!(hits[0].2 > hits[1].2);
    }
}
