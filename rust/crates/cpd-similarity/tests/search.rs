// Similarity as a user of the crate sees it: the units read from source and
// the pairs the search reports.

use cpd_core::detect::PathFilters;
use cpd_similarity::{FormSource, SimilarPairs, find_similar, forms, jaccard};

const ALPHA: &str = "def alpha(xs):\n    ys = filter(xs, 1)\n    total = 0\n    for y in ys:\n        total += y * 2\n    return map(total, inc)\n";
const GAMMA: &str = "def gamma(items):\n    kept = sorted(items, 5)\n    sum = 0\n    for k in kept:\n        sum -= k * 2\n    print(sum)\n    return map(sum, dec)\n";

fn source(id: &str, format: &str, code: &str) -> FormSource {
    FormSource {
        id: id.to_string(),
        format: format.to_string(),
        real_path: String::new(),
        forms: forms(code, format, &[]),
    }
}

fn search(sources: Vec<FormSource>, threshold: f64) -> SimilarPairs {
    find_similar(sources, threshold, 1, 1, &[], &PathFilters::default(), true)
}

/// The pairs found, each as its two ids in a fixed order and its score.
fn pairs(found: &SimilarPairs) -> Vec<(String, String, f64)> {
    let mut out: Vec<_> = found
        .all
        .iter()
        .map(|c| {
            let (a, b) = (
                c.fragment_a.source_id.clone(),
                c.fragment_b.source_id.clone(),
            );
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            (a, b, f64::from(c.similarity.unwrap_or_default()))
        })
        .collect();
    out.sort_by(|x, y| x.partial_cmp(y).unwrap());
    out
}

fn score(a: &str, b: &str, format: &str) -> f64 {
    let (a, b) = (forms(a, format, &[]), forms(b, format, &[]));
    jaccard(&a[0].fingerprints, &b[0].fingerprints)
}

#[test]
fn the_threshold_is_inclusive() {
    let s = score(ALPHA, GAMMA, "python");
    assert!(s > 0.0 && s < 1.0, "a partial match, got {s}");
    let sources = || {
        vec![
            source("a.py", "python", ALPHA),
            source("g.py", "python", GAMMA),
        ]
    };
    assert_eq!(
        pairs(&search(sources(), s)).len(),
        1,
        "a score equal to the threshold pairs"
    );
    assert!(
        search(sources(), s.next_up()).all.is_empty(),
        "just above it does not"
    );
}

#[test]
fn the_pairs_do_not_depend_on_the_order_of_the_sources() {
    let renamed = ALPHA.replace("alpha", "beta").replace("total", "acc");
    let forward = vec![
        source("a.py", "python", ALPHA),
        source("b.py", "python", &renamed),
        source("g.py", "python", GAMMA),
    ];
    let mut backward = forward.clone();
    backward.reverse();
    assert_eq!(pairs(&search(forward, 0.5)), pairs(&search(backward, 0.5)));
}

#[test]
fn the_score_is_symmetric_and_whole_for_a_renamed_copy() {
    let renamed = ALPHA.replace("alpha", "beta").replace("total", "acc");
    assert_eq!(score(ALPHA, GAMMA, "python"), score(GAMMA, ALPHA, "python"));
    assert_eq!(score(ALPHA, &renamed, "python"), 1.0);
    assert_eq!(jaccard(&[], &[]), 0.0, "nothing shared out of nothing");
}

#[test]
fn empty_sources_and_formats_without_units_yield_nothing() {
    assert!(forms("", "python", &[]).is_empty());
    assert!(forms("a { color: red }", "css", &[]).is_empty());
    assert!(search(Vec::new(), 0.8).all.is_empty());
    let found = search(
        vec![
            source("a.css", "css", "a { color: red }"),
            source("b.css", "css", "a { color: red }"),
        ],
        0.0,
    );
    assert!(found.all.is_empty() && found.reported.is_empty());
}
