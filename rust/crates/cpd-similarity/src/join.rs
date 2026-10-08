//! Every pair of fingerprint sets whose Jaccard index reaches a threshold,
//! found exactly: the result is the one comparing every set with every other
//! would give, without doing that.
//!
//! The search filters by prefix. Fingerprints are ranked rarest first, and
//! a set of `n` fingerprints keeps its first `n - ⌈t·n⌉ + 1` as its prefix.
//! Two sets whose index is at least `t` share at least `⌈t·n⌉` fingerprints
//! of each, so their prefixes share one: a pair that shares no fingerprint
//! in its prefixes cannot reach `t`. A set also cannot reach `t` with one
//! smaller than `t` times its size. Sets are visited smallest first, each
//! one compared with the earlier sets that share a prefix fingerprint and
//! are large enough, and then indexed by its own prefix.

use rustc_hash::FxHashMap;

/// The Jaccard index of two sorted sets: shared over all.
pub fn jaccard(a: &[u64], b: &[u64]) -> f64 {
    let (shared, total) = overlap(a, b);
    match total {
        0 => 0.0,
        _ => shared as f64 / total as f64,
    }
}

/// The size of the intersection and of the union of two sorted sets.
pub(crate) fn overlap<T: Ord>(a: &[T], b: &[T]) -> (u32, u32) {
    let (mut i, mut j, mut shared) = (0, 0, 0u32);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                shared += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let total = (a.len() + b.len()) as u32 - shared;
    (shared, total)
}

/// A pair of sets found: their indexes, smaller first, and the sizes of
/// their intersection and union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub a: usize,
    pub b: usize,
    pub shared: u32,
    pub total: u32,
}

/// The fingerprints a set of `size` needs to share with another to reach
/// `threshold`, rounded down a hair so float error never shortens a prefix.
fn needed(size: usize, threshold: f64) -> usize {
    ((threshold * size as f64) - 1e-9).ceil().max(0.0) as usize
}

/// Every pair of `sets` (each sorted) whose Jaccard index is at least
/// `threshold`, in no particular order.
pub fn pairs(sets: &[&[u64]], threshold: f64) -> Vec<Match> {
    if sets.len() < 2 {
        return Vec::new();
    }
    // Rank the fingerprints, rarest first: rare ones make short candidate
    // lists.
    let mut counts: FxHashMap<u64, u32> = FxHashMap::default();
    for set in sets {
        for &print in *set {
            *counts.entry(print).or_default() += 1;
        }
    }
    let mut order: Vec<(u32, u64)> = counts.iter().map(|(&print, &n)| (n, print)).collect();
    order.sort_unstable();
    let rank: FxHashMap<u64, u32> = order
        .iter()
        .enumerate()
        .map(|(rank, &(_, print))| (print, rank as u32))
        .collect();
    let ranked: Vec<Vec<u32>> = sets
        .iter()
        .map(|set| {
            let mut ranks: Vec<u32> = set.iter().map(|print| rank[print]).collect();
            ranks.sort_unstable();
            ranks
        })
        .collect();

    let mut visit: Vec<usize> = (0..sets.len()).collect();
    visit.sort_by_key(|&i| (ranked[i].len(), i));
    let mut index: FxHashMap<u32, Vec<usize>> = FxHashMap::default();
    let mut seen = vec![usize::MAX; sets.len()];
    let mut found = Vec::new();
    for &x in &visit {
        let size = ranked[x].len();
        if size == 0 {
            continue;
        }
        let prefix = size - needed(size, threshold).min(size) + 1;
        let prefix = prefix.min(size);
        let smallest = needed(size, threshold);
        for &token in &ranked[x][..prefix] {
            let Some(earlier) = index.get(&token) else {
                continue;
            };
            for &y in earlier {
                if seen[y] == x || ranked[y].len() < smallest {
                    continue;
                }
                seen[y] = x;
                let (shared, total) = overlap(&ranked[x], &ranked[y]);
                if shared as f64 / total as f64 >= threshold {
                    found.push(Match {
                        a: x.min(y),
                        b: x.max(y),
                        shared,
                        total,
                    });
                }
            }
        }
        for &token in &ranked[x][..prefix] {
            index.entry(token).or_default().push(x);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pair, compared one by one.
    fn brute(sets: &[&[u64]], threshold: f64) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for a in 0..sets.len() {
            for b in a + 1..sets.len() {
                if jaccard(sets[a], sets[b]) >= threshold {
                    out.push((a, b));
                }
            }
        }
        out
    }

    #[test]
    fn the_search_finds_what_comparing_every_pair_finds() {
        // Sets drawn from a small alphabet so that many pairs come close.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let sets: Vec<Vec<u64>> = (0..300)
            .map(|_| {
                let size = 5 + (next() % 40) as usize;
                let mut set: Vec<u64> = (0..size).map(|_| next() % 60).collect();
                set.sort_unstable();
                set.dedup();
                set
            })
            .collect();
        let refs: Vec<&[u64]> = sets.iter().map(Vec::as_slice).collect();
        for threshold in [0.5, 0.6, 0.7, 0.8, 0.82, 0.9, 1.0] {
            let mut got: Vec<(usize, usize)> =
                pairs(&refs, threshold).iter().map(|m| (m.a, m.b)).collect();
            got.sort_unstable();
            assert_eq!(got, brute(&refs, threshold), "threshold {threshold}");
        }
    }

    #[test]
    fn a_pair_knows_its_shared_and_total_fingerprints() {
        let a: &[u64] = &[1, 2, 3, 4];
        let b: &[u64] = &[2, 3, 4, 5];
        let found = pairs(&[a, b], 0.5);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].shared, found[0].total), (3, 5));
    }
}
