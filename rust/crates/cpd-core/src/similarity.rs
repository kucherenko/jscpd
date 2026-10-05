//! Function-level similarity (issue #999, stage 2).
//!
//! Each function is summarized by the bag of `k`-grams over the pre-order
//! sequence of its AST node types ("shingles"). Two functions are similar
//! when the weighted Jaccard index of their shingle bags reaches the
//! configured threshold. Candidate pairs come from MinHash + LSH banding so
//! the search stays close to linear in the number of functions; the exact
//! bag Jaccard is only computed for candidates.
//!
//! Node *types* only by default: identifier names and literal values do not
//! take part, so a renamed copy scores 1.0 and an edited copy scores by how
//! much of its structure survived. Positions always reference the original
//! source.
//!
//! The role-aware mode ([`SimilarityIdentifiers::RoleAware`], issue #1136)
//! adds the names whose role changes what the code does: each method a call
//! invokes joins the sequence right after the call's node, so `store.load(x)`
//! and `store.save(x)` no longer look the same, while variables, parameters
//! and receivers stay anonymous. Extractors record those names as
//! [`RoleName`]s whatever the mode; the mode decides whether a signature
//! uses them.
//!
//! Literals follow [`SimilarityLiterals`] (issue #1139). A literal's node
//! type is its category, so by default a string and a number differ while
//! two strings with other text match. The other modes add the parsed value
//! after the literal, put one marker in place of every literal, or leave
//! literals out. Extractors record each literal as a [`LiteralLeaf`] over
//! its nodes whatever the mode, as they do names.
//!
//! The scoring is grammar-agnostic: node-type ids are opaque `u16`s from
//! whichever extractor produced them (`cpd_tokenizer::functions`), and a
//! signature records its `grammar` so functions are only compared within
//! one grammar. Adding a language means adding an extractor, not touching
//! this module.

use crate::detect::{PathFilters, PreparedSource};
use crate::models::{CloneKind, CpdClone, Fragment, Location, SimilarityMethod};
use rustc_hash::{FxHashMap, FxHashSet};

/// Which identifier names take part in a function's structural summary
/// (`--similarity-identifiers`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarityIdentifiers {
    /// No names: the summary holds node types only, so a renamed copy
    /// scores like the original.
    #[default]
    Ignore,
    /// Names by their role in the code: the method a call invokes counts;
    /// variables, parameters, receivers and every other name do not.
    RoleAware,
}

impl SimilarityIdentifiers {
    /// The names of a function this mode keeps out of the ones its
    /// extractor recorded: all of them in role-aware mode, none otherwise.
    pub fn names(self, names: &[RoleName]) -> &[RoleName] {
        match self {
            Self::Ignore => &[],
            Self::RoleAware => names,
        }
    }

    /// The value as `--similarity-identifiers` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::RoleAware => "role-aware",
        }
    }
}

impl std::str::FromStr for SimilarityIdentifiers {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "ignore" => Ok(Self::Ignore),
            "role-aware" => Ok(Self::RoleAware),
            other => Err(format!(
                "unknown value '{other}', expected ignore or role-aware"
            )),
        }
    }
}

/// How literals take part in a function's structural summary
/// (`--similarity-literals`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarityLiterals {
    /// The category and the parsed value: two literals match when both are
    /// equal, so `timeout=10.0` and `timeout=30.0` differ.
    Values,
    /// The category alone, as the grammar's node types tell it: a string, a
    /// number, a boolean, `null` or `None`, a regular expression.
    #[default]
    Categories,
    /// One marker for every literal, so a string matches a number.
    Generic,
    /// No literals: the summary keeps only the structure around them.
    Omit,
}

impl SimilarityLiterals {
    /// The value as `--similarity-literals` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Values => "values",
            Self::Categories => "categories",
            Self::Generic => "generic",
            Self::Omit => "omit",
        }
    }
}

impl std::str::FromStr for SimilarityLiterals {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "values" => Ok(Self::Values),
            "categories" => Ok(Self::Categories),
            "generic" => Ok(Self::Generic),
            "omit" => Ok(Self::Omit),
            other => Err(format!(
                "unknown value '{other}', expected values, categories, generic or omit"
            )),
        }
    }
}

/// What a function's summary keeps besides its node types: the names of
/// `--similarity-identifiers` and the literals of `--similarity-literals`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignaturePolicy {
    pub identifiers: SimilarityIdentifiers,
    pub literals: SimilarityLiterals,
}

/// What an extractor records about one function: the pre-order node types
/// of its syntax tree, the names role-aware similarity can keep, and its
/// literals.
#[derive(Debug, Clone, Copy, Default)]
pub struct Structure<'a> {
    pub kinds: &'a [u16],
    pub names: &'a [RoleName],
    pub literals: &'a [LiteralLeaf],
}

/// A literal in a function's node-type sequence: the `len` nodes from index
/// `at`. That is the literal's own node, followed by the nodes of its parts
/// where the grammar gives it some, as for Python's adjacent strings
/// (`"a" "b"`). `value` is the [`literal_hash`] of its category and parsed
/// value. A literal holds no call, so no [`RoleName`] lands inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralLeaf {
    pub at: u32,
    pub len: u32,
    pub value: u64,
}

/// The sequence symbol of a literal's value in the values mode: FNV-1a over
/// a domain tag, the literal's node type and the bytes of its parsed value,
/// with the top bit set as for a name, so that no value equals a node type.
/// The node type keeps the string `"1"` and the number `1` apart.
pub fn literal_hash(kind: u16, value: &[u8]) -> u64 {
    let fnv = |acc: u64, byte: &u8| (acc ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    let tagged = b"literal".iter().fold(FNV_OFFSET, fnv);
    let typed = kind.to_le_bytes().iter().fold(tagged, fnv);
    value.iter().fold(typed, fnv) | (1 << 63)
}

/// The one symbol the generic mode puts in place of every literal. Bit 62
/// alone keeps it apart from node types, which fit in 16 bits, and from
/// names and values, which have the top bit set.
pub const LITERAL_MARKER: u64 = 1 << 62;

/// A name role-aware similarity keeps: the method a call invokes, placed
/// in the function's node-type sequence right after the node at index
/// `after`, the call itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleName {
    pub after: u32,
    /// [`name_hash`] of the name.
    pub hash: u64,
}

impl RoleName {
    pub fn new(after: usize, name: &str) -> Self {
        Self {
            after: after as u32,
            hash: name_hash(name),
        }
    }
}

/// The sequence symbol of a kept name: FNV-1a over its bytes with the top
/// bit set, so that no name can equal a node type id, which fits in 16 bits.
pub fn name_hash(name: &str) -> u64 {
    let hash = name.bytes().fold(FNV_OFFSET, |acc, byte| {
        (acc ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    hash | (1 << 63)
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The size of a function's code when its span also holds text that is not
/// code, as a Python docstring and comments do: the tokens and the line span
/// that `--min-tokens` and `--min-lines` read. JavaScript has no such text
/// to leave out: its comments yield no tokens, and a JSDoc block sits before
/// the function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSize {
    pub tokens: u32,
    pub lines: u32,
}

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
    /// Detection tokens covered by the function, or its code tokens when
    /// [`Self::with_code_size`] gave them.
    pub token_count: u32,
    /// The line span `--min-lines` reads: [`Self::line_span`], less the
    /// lines that hold no code when [`Self::with_code_size`] said which.
    pub code_lines: u32,
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
        let structure = Structure {
            kinds,
            ..Structure::default()
        };
        Self::build_with(
            grammar,
            name,
            start,
            end,
            structure,
            SignaturePolicy::default(),
            spans,
        )
    }

    /// [`Self::build`] over everything an extractor recorded, summarized as
    /// `policy` says: the names role-aware similarity keeps join the
    /// sequence after their node, so the shingles around a call see which
    /// method it invokes, and literals take part as `policy.literals` says.
    /// Under the default policy the signature is the one [`Self::build`]
    /// makes from the node types.
    pub fn build_with(
        grammar: &'static str,
        name: String,
        start: Location,
        end: Location,
        structure: Structure<'_>,
        policy: SignaturePolicy,
        spans: &[(Location, Location)],
    ) -> Option<Self> {
        let (first, last) = token_range(spans, &start, &end)?;
        let shingles = match summary(structure, policy) {
            None => shingles_from_kinds(structure.kinds, SHINGLE_K),
            Some(symbols) => shingles_from_symbols(&symbols, SHINGLE_K),
        };
        if shingles.is_empty() {
            return None;
        }
        let minhash = minhash(&shingles);
        let code_lines = end.line.saturating_sub(start.line);
        Some(Self {
            grammar,
            name,
            start,
            end,
            range: [first as u32, (last - 1) as u32],
            token_count: (last - first) as u32,
            code_lines,
            shingles,
            minhash,
        })
    }

    /// Read the size limits on the function's code alone when its span also
    /// holds text that is not code: a Python docstring would otherwise carry
    /// a one-line getter past `--min-tokens` and `--min-lines`.
    pub fn with_code_size(mut self, size: Option<CodeSize>) -> Self {
        if let Some(size) = size {
            self.token_count = self.token_count.min(size.tokens);
            self.code_lines = self.code_lines.min(size.lines);
        }
        self
    }

    /// Lines spanned, in jscpd's `end - start` convention.
    pub fn line_span(&self) -> u32 {
        self.end.line.saturating_sub(self.start.line)
    }
}

/// The detection tokens lying inside the byte range `start..end` of a
/// source, as a half-open index range into its `spans`; `None` when there
/// are none (a body of comments or type declarations only).
pub fn token_range(
    spans: &[(Location, Location)],
    start: &Location,
    end: &Location,
) -> Option<(usize, usize)> {
    let first = spans.partition_point(|(s, _)| s.offset < start.offset);
    let last = spans.partition_point(|(_, e)| e.offset <= end.offset);
    (first < last).then_some((first, last))
}

/// Hash every `k`-gram of `kinds`; the result is sorted so it can be used as
/// a multiset by [`bag_jaccard`].
pub fn shingles_from_kinds(kinds: &[u16], k: usize) -> Vec<u64> {
    shingles(kinds, k)
}

/// [`shingles_from_kinds`] over a sequence of node types and name symbols
/// ([`name_hash`]). A sequence of node types alone hashes to the same
/// shingles either way.
pub fn shingles_from_symbols(symbols: &[u64], k: usize) -> Vec<u64> {
    shingles(symbols, k)
}

fn shingles<T: Copy + Into<u64>>(sequence: &[T], k: usize) -> Vec<u64> {
    if sequence.len() < k {
        return Vec::new();
    }
    let mut out: Vec<u64> = sequence
        .windows(k)
        .map(|w| {
            w.iter().fold(FNV_OFFSET, |acc, &t| {
                (acc ^ t.into()).wrapping_mul(FNV_PRIME)
            })
        })
        .collect();
    out.sort_unstable();
    out
}

/// The symbol sequence of a function under `policy`, or `None` when it is
/// the node types alone: the policy keeps no name and changes no literal.
/// That keeps the default signatures what they were before the policy
/// existed.
fn summary(structure: Structure<'_>, policy: SignaturePolicy) -> Option<Vec<u64>> {
    let names = policy.identifiers.names(structure.names);
    let literals = match policy.literals {
        SimilarityLiterals::Categories => &[][..],
        _ => structure.literals,
    };
    if names.is_empty() && literals.is_empty() {
        return None;
    }
    Some(symbols(structure.kinds, names, literals, policy.literals))
}

/// `items` sorted by `key`. Extractors record names and literals in the
/// order they walk, which is sorted already, so this rarely copies.
fn sorted_by<T: Clone, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> std::borrow::Cow<'_, [T]> {
    match items.windows(2).all(|w| key(&w[0]) <= key(&w[1])) {
        true => std::borrow::Cow::Borrowed(items),
        false => {
            let mut copy = items.to_vec();
            copy.sort_by_key(key);
            std::borrow::Cow::Owned(copy)
        }
    }
}

/// The node types of a function with its kept names and its literals as
/// `mode` says: each name right after the node at its `after` index, each
/// literal's nodes followed by its value, replaced by [`LITERAL_MARKER`], or
/// left out. The categories mode leaves the literals' nodes as they are.
fn symbols(
    kinds: &[u16],
    names: &[RoleName],
    literals: &[LiteralLeaf],
    mode: SimilarityLiterals,
) -> Vec<u64> {
    let names = sorted_by(names, |n| n.after);
    let literals = sorted_by(literals, |l| l.at);
    let mut out = Vec::with_capacity(kinds.len() + names.len() + literals.len());
    let mut names = names.iter().peekable();
    let mut literals = literals.iter().peekable();
    let node = |kind: &u16| u64::from(*kind);
    let mut i = 0;
    while i < kinds.len() {
        // A literal inside one already walked, which no extractor records,
        // would overlap it.
        while literals.next_if(|l| (l.at as usize) < i).is_some() {}
        let next = match literals.next_if(|l| l.at as usize == i) {
            Some(literal) => {
                let end = (i + literal.len.max(1) as usize).min(kinds.len());
                match mode {
                    SimilarityLiterals::Values => {
                        out.extend(kinds[i..end].iter().map(node));
                        out.push(literal.value);
                    }
                    SimilarityLiterals::Categories => out.extend(kinds[i..end].iter().map(node)),
                    SimilarityLiterals::Generic => out.push(LITERAL_MARKER),
                    SimilarityLiterals::Omit => {}
                }
                end
            }
            None => {
                out.push(node(&kinds[i]));
                i + 1
            }
        };
        while let Some(name) = names.next_if(|n| (n.after as usize) < next) {
            out.push(name.hash);
        }
        i = next;
    }
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
    /// Canonical path of the file behind a symlink; empty when it is `id`.
    /// `--skip-isolated` reads it as it does for token clones.
    pub real_path: String,
    pub functions: Vec<FunctionSig>,
}

/// Pull the function signatures out of prepared sources (clones only the
/// sources that carry any, so a run without `--similarity` copies nothing).
pub fn collect_function_sources<'a>(
    prepared: impl IntoIterator<Item = &'a PreparedSource>,
) -> Vec<FunctionSource> {
    prepared
        .into_iter()
        .filter(|p| !p.functions.is_empty())
        .map(|p| FunctionSource {
            id: p.id.clone(),
            format: p.format.clone(),
            real_path: p.real_path.clone(),
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
        f.token_count >= min_tokens && f.code_lines >= min_lines
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

    /// The indexed functions structurally similar to the functions of
    /// `source`, a source outside the index such as a snippet, as `similar`
    /// clones, most similar first. As in [`Self::all_pairs`], a pair that
    /// the clones in `existing` already cover is left out.
    pub fn query_clones(
        &self,
        source: &FunctionSource,
        threshold: f32,
        existing: &[CpdClone],
    ) -> Vec<CpdClone> {
        let coverage = Coverage::new(existing);
        let coverage = &coverage;
        let mut clones: Vec<CpdClone> = source
            .functions
            .iter()
            .flat_map(|query| {
                self.query(query, threshold)
                    .into_iter()
                    .filter_map(move |(si, fi, sim)| {
                        let other = &self.sources[si];
                        let found = &other.functions[fi];
                        (!coverage.covers(lines_of(source, query), lines_of(other, found)))
                            .then(|| make_clone(source, query, other, found, sim))
                    })
            })
            .collect();
        clones.sort_by(|x, y| {
            y.similarity
                .unwrap_or_default()
                .total_cmp(&x.similarity.unwrap_or_default())
                .then_with(|| x.position_key().cmp(&y.position_key()))
        });
        clones
    }

    /// All similar pairs among the indexed functions, as `similar` clones.
    /// Pairs the clones in `existing` already cover (see [`Coverage`]) are
    /// left out so nothing is reported twice, and so are the pairs that
    /// `filters` drops for token clones (`--skip-local`, `--skip-isolated`).
    pub fn all_pairs(
        &self,
        threshold: f32,
        existing: &[CpdClone],
        filters: &PathFilters,
    ) -> Vec<CpdClone> {
        let coverage = Coverage::new(existing);
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
                let (src_a, src_b) = (&self.sources[sa], &self.sources[sb]);
                if filters.should_skip_sources(
                    (&src_a.id, &src_a.real_path),
                    (&src_b.id, &src_b.real_path),
                ) {
                    return None;
                }
                let sim = score(fa_sig, fb_sig, threshold)?;
                if coverage.covers(lines_of(src_a, fa_sig), lines_of(src_b, fb_sig)) {
                    return None;
                }
                let _ = (fa, fb);
                Some(make_clone(src_a, fa_sig, src_b, fb_sig, sim))
            })
            .collect();
        clones.sort_by(|x, y| x.position_key().cmp(&y.position_key()));
        clones
    }
}

/// Find similar function pairs across `sources` (issue #999, stage 2),
/// leaving out the pairs `existing` covers and the ones `filters` drops.
pub fn find_similar_functions(
    sources: Vec<FunctionSource>,
    threshold: f32,
    min_tokens: usize,
    min_lines: usize,
    existing: &[CpdClone],
    filters: &PathFilters,
) -> Vec<CpdClone> {
    if sources.is_empty() {
        return Vec::new();
    }
    SimilarityIndex::build(sources, min_tokens, min_lines).all_pairs(threshold, existing, filters)
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

/// A function as [`Coverage::covers`] reads it: its source and lines.
fn lines_of<'a>(source: &'a FunctionSource, f: &FunctionSig) -> (&'a str, u32, u32) {
    (&source.id, f.start.line, f.end.line)
}

/// What the clones found so far report, indexed by source. A pair of
/// functions is covered when one clone spans both, or when two clones copy
/// one fragment into each of them: detection pairs every copy of a fragment
/// with its first copy only, so with three copies the pair of the second and
/// the third is implied and is not reported again as `similar`.
pub struct Coverage<'a> {
    /// For every source, the fragments of the clones in it, each with the
    /// fragment it was copied with.
    by_source: FxHashMap<&'a str, Vec<(&'a Fragment, &'a Fragment)>>,
}

impl<'a> Coverage<'a> {
    pub fn new(existing: impl IntoIterator<Item = &'a CpdClone>) -> Self {
        let mut by_source: FxHashMap<&'a str, Vec<(&'a Fragment, &'a Fragment)>> =
            FxHashMap::default();
        for clone in existing {
            let (a, b) = (&clone.fragment_a, &clone.fragment_b);
            by_source
                .entry(a.source_id.as_str())
                .or_default()
                .push((a, b));
            by_source
                .entry(b.source_id.as_str())
                .or_default()
                .push((b, a));
        }
        Self { by_source }
    }

    /// Whether the clones cover the pair of functions `a` and `b`, each its
    /// source id with its first and last line.
    pub fn covers(&self, a: (&str, u32, u32), b: (&str, u32, u32)) -> bool {
        // The fragments copied into a function: the partners of the clones
        // that span at least 90% of its lines.
        let partners = |(id, start, end): (&str, u32, u32)| {
            self.by_source
                .get(id)
                .into_iter()
                .flatten()
                .filter(move |(here, _)| spans_lines(here, start, end))
                .map(|(_, there)| *there)
        };
        let into_b: Vec<&Fragment> = partners(b).collect();
        if into_b.is_empty() {
            return false;
        }
        partners(a).any(|x| {
            (x.source_id == b.0 && spans_lines(x, b.1, b.2))
                || into_b
                    .iter()
                    .any(|y| x.source_id == y.source_id && overlap(x, y))
        })
    }

    /// [`Self::covers`] for a pair of whole functions, such as one an index
    /// found before these clones.
    pub fn covers_clone(&self, pair: &CpdClone) -> bool {
        let (a, b) = (&pair.fragment_a, &pair.fragment_b);
        self.covers(
            (&a.source_id, a.start.line, a.end.line),
            (&b.source_id, b.start.line, b.end.line),
        )
    }
}

/// Whether `frag` spans at least 90% of the lines `start..=end`.
fn spans_lines(frag: &Fragment, start: u32, end: u32) -> bool {
    let lo = frag.start.line.max(start);
    let hi = frag.end.line.min(end);
    if hi < lo {
        return false;
    }
    let overlap = hi - lo + 1;
    let span = end.saturating_sub(start) + 1;
    overlap as f32 >= 0.9 * span as f32
}

/// Whether two fragments of one source share at least 90% of the lines of
/// the shorter one.
fn overlap(x: &Fragment, y: &Fragment) -> bool {
    let lo = x.start.line.max(y.start.line);
    let hi = x.end.line.min(y.end.line);
    if hi < lo {
        return false;
    }
    let shorter = x
        .end
        .line
        .saturating_sub(x.start.line)
        .min(y.end.line.saturating_sub(y.start.line))
        + 1;
    (hi - lo + 1) as f32 >= 0.9 * shorter as f32
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
    // The clone's format is its first fragment's, whichever function that is:
    // a pair can join two formats of one grammar, such as a `.ts` file and the
    // script of a `.vue` file.
    let format = match a_first {
        true => src_a.format.clone(),
        false => src_b.format.clone(),
    };
    CpdClone {
        format,
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

    /// The signature of a function over 8 tokens with `structure`, built
    /// under `policy`.
    fn build_with(structure: Structure<'_>, policy: SignaturePolicy) -> FunctionSig {
        FunctionSig::build_with(
            "test",
            "f".into(),
            loc(1, 0),
            loc(8, 75),
            structure,
            policy,
            &spans(8),
        )
        .unwrap()
    }

    fn policy(identifiers: SimilarityIdentifiers, literals: SimilarityLiterals) -> SignaturePolicy {
        SignaturePolicy {
            identifiers,
            literals,
        }
    }

    #[test]
    fn under_the_default_policy_both_builders_make_the_same_signature() {
        let spans = spans(8);
        let kinds = [1, 2, 3, 4, 5, 6, 7];
        let names = [RoleName::new(2, "load")];
        let literals = [LiteralLeaf {
            at: 4,
            len: 1,
            value: literal_hash(5, b"x"),
        }];
        let plain = FunctionSig::build("test", "f".into(), loc(1, 0), loc(8, 75), &kinds, &spans);
        let recorded = Structure {
            kinds: &kinds,
            names: &names,
            literals: &literals,
        };
        assert_eq!(
            plain.as_ref(),
            Some(&build_with(recorded, SignaturePolicy::default())),
            "names and literals change nothing by default"
        );
        assert_eq!(summary(recorded, SignaturePolicy::default()), None);
        let symbols: Vec<u64> = kinds.iter().map(|&k| u64::from(k)).collect();
        assert_eq!(
            shingles_from_kinds(&kinds, 4),
            shingles_from_symbols(&symbols, 4)
        );
    }

    #[test]
    fn role_names_tell_called_methods_apart() {
        let spans = spans(8);
        let kinds = [1, 2, 3, 4, 5, 6, 7, 8];
        let build = |names: &[RoleName]| {
            FunctionSig::build_with(
                "test",
                "f".into(),
                loc(1, 0),
                loc(8, 75),
                Structure {
                    kinds: &kinds,
                    names,
                    ..Structure::default()
                },
                policy(
                    SimilarityIdentifiers::RoleAware,
                    SimilarityLiterals::Categories,
                ),
                &spans,
            )
            .unwrap()
        };
        let load = build(&[RoleName::new(3, "load")]);
        let load_again = build(&[RoleName::new(3, "load")]);
        let save = build(&[RoleName::new(3, "save")]);
        assert_eq!(bag_jaccard(&load.shingles, &load_again.shingles), 1.0);
        let other = bag_jaccard(&load.shingles, &save.shingles);
        assert!(other > 0.0 && other < 1.0, "{other}");
        let kept = [RoleName::new(3, "load")];
        assert!(SimilarityIdentifiers::Ignore.names(&kept).is_empty());
        assert_eq!(SimilarityIdentifiers::RoleAware.names(&kept), &kept);
    }

    #[test]
    fn names_land_after_their_node_whatever_order_they_come_in() {
        let a = RoleName::new(0, "a");
        let b = RoleName::new(2, "b");
        assert_eq!(
            symbols(&[10, 20, 30], &[b, a], &[], SimilarityLiterals::Categories),
            vec![10, a.hash, 20, 30, b.hash]
        );
    }

    #[test]
    fn name_symbols_never_equal_node_types() {
        for name in ["", "load", "x"] {
            assert!(name_hash(name) > u64::from(u16::MAX));
        }
        assert_ne!(name_hash("load"), name_hash("save"));
    }

    #[test]
    fn identifiers_mode_parses_its_two_values() {
        assert_eq!(
            "ignore".parse::<SimilarityIdentifiers>(),
            Ok(SimilarityIdentifiers::Ignore)
        );
        assert_eq!(
            "role-aware".parse::<SimilarityIdentifiers>(),
            Ok(SimilarityIdentifiers::RoleAware)
        );
        assert!("names".parse::<SimilarityIdentifiers>().is_err());
        assert_eq!(
            SimilarityIdentifiers::default(),
            SimilarityIdentifiers::Ignore
        );
        assert_eq!(SimilarityIdentifiers::RoleAware.as_str(), "role-aware");
    }

    #[test]
    fn literals_mode_parses_its_four_values() {
        for mode in [
            SimilarityLiterals::Values,
            SimilarityLiterals::Categories,
            SimilarityLiterals::Generic,
            SimilarityLiterals::Omit,
        ] {
            assert_eq!(mode.as_str().parse::<SimilarityLiterals>(), Ok(mode));
        }
        assert_eq!(
            SimilarityLiterals::default(),
            SimilarityLiterals::Categories
        );
        let error = "value".parse::<SimilarityLiterals>().unwrap_err();
        assert!(
            error.contains("values, categories, generic or omit"),
            "{error}"
        );
    }

    /// Kinds `[1, 2, 3, 4, 5, 6]` where nodes 2 and 3 are one literal (a
    /// string node and its part) and node 5 another.
    fn two_literals() -> ([u16; 6], [LiteralLeaf; 2]) {
        let string = LiteralLeaf {
            at: 1,
            len: 2,
            value: literal_hash(2, b"active"),
        };
        let number = LiteralLeaf {
            at: 4,
            len: 1,
            value: literal_hash(5, b"3"),
        };
        ([1, 2, 3, 4, 5, 6], [string, number])
    }

    #[test]
    fn each_literal_mode_shapes_the_sequence_its_own_way() {
        let (kinds, literals) = two_literals();
        let [string, number] = literals;
        let shape = |mode| symbols(&kinds, &[], &literals, mode);
        assert_eq!(
            shape(SimilarityLiterals::Categories),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            shape(SimilarityLiterals::Values),
            vec![1, 2, 3, string.value, 4, 5, number.value, 6]
        );
        assert_eq!(
            shape(SimilarityLiterals::Generic),
            vec![1, LITERAL_MARKER, 4, LITERAL_MARKER, 6]
        );
        assert_eq!(shape(SimilarityLiterals::Omit), vec![1, 4, 6]);
    }

    #[test]
    fn names_keep_their_place_when_literals_are_replaced_or_dropped() {
        let (kinds, literals) = two_literals();
        let call = RoleName::new(0, "load");
        let after_four = RoleName::new(3, "save");
        let names = [after_four, call];
        assert_eq!(
            symbols(&kinds, &names, &literals, SimilarityLiterals::Omit),
            vec![1, call.hash, 4, after_four.hash, 6]
        );
        assert_eq!(
            symbols(&kinds, &names, &literals, SimilarityLiterals::Generic),
            vec![
                1,
                call.hash,
                LITERAL_MARKER,
                4,
                after_four.hash,
                LITERAL_MARKER,
                6
            ]
        );
    }

    #[test]
    fn only_the_values_mode_tells_other_values_apart() {
        let kinds = [1, 2, 3, 4, 5, 6, 7, 8];
        let leaf = |value: &[u8]| LiteralLeaf {
            at: 3,
            len: 1,
            value: literal_hash(4, value),
        };
        let (ten, thirty) = ([leaf(b"10.0")], [leaf(b"30.0")]);
        let score = |mode| {
            let sig = |literals: &[LiteralLeaf]| {
                build_with(
                    Structure {
                        kinds: &kinds,
                        literals,
                        ..Structure::default()
                    },
                    policy(SimilarityIdentifiers::Ignore, mode),
                )
            };
            bag_jaccard(&sig(&ten).shingles, &sig(&thirty).shingles)
        };
        assert_eq!(score(SimilarityLiterals::Categories), 1.0);
        assert_eq!(score(SimilarityLiterals::Generic), 1.0);
        assert_eq!(score(SimilarityLiterals::Omit), 1.0);
        let values = score(SimilarityLiterals::Values);
        assert!(values > 0.0 && values < 1.0, "{values}");
    }

    #[test]
    fn generic_and_omit_match_literals_of_other_categories() {
        // The same function with a string (kind 4) or a number (kind 9) at
        // its fourth node.
        let string = [1, 2, 3, 4, 5, 6, 7, 8];
        let number = [1, 2, 3, 9, 5, 6, 7, 8];
        let score = |mode| {
            let sig = |kinds: &[u16], value: &[u8]| {
                let literals = [LiteralLeaf {
                    at: 3,
                    len: 1,
                    value: literal_hash(kinds[3], value),
                }];
                build_with(
                    Structure {
                        kinds,
                        literals: &literals,
                        ..Structure::default()
                    },
                    policy(SimilarityIdentifiers::Ignore, mode),
                )
            };
            bag_jaccard(&sig(&string, b"1").shingles, &sig(&number, b"1").shingles)
        };
        assert!(score(SimilarityLiterals::Categories) < 1.0);
        assert!(score(SimilarityLiterals::Values) < 1.0);
        assert_eq!(score(SimilarityLiterals::Generic), 1.0);
        assert_eq!(score(SimilarityLiterals::Omit), 1.0);
    }

    #[test]
    fn literal_symbols_never_equal_node_types_names_or_the_marker() {
        for (kind, value) in [(4u16, &b""[..]), (4, b"load"), (9, b"1")] {
            let symbol = literal_hash(kind, value);
            assert!(symbol > u64::from(u16::MAX));
            assert_ne!(symbol, LITERAL_MARKER);
            assert_ne!(symbol, name_hash(std::str::from_utf8(value).unwrap()));
        }
        assert!(LITERAL_MARKER > u64::from(u16::MAX));
        assert_eq!(
            LITERAL_MARKER & (1 << 63),
            0,
            "names and values set the top bit"
        );
        assert_ne!(
            literal_hash(4, b"1"),
            literal_hash(9, b"1"),
            "the category is part of the value"
        );
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
                real_path: String::new(),
                functions: vec![sig("base", &base, 0, 60)],
            },
            FunctionSource {
                id: "b.js".into(),
                format: "javascript".into(),
                real_path: String::new(),
                functions: vec![sig("edited", &edited, 0, 62), sig("other", &other, 70, 140)],
            },
        ];
        let clones =
            find_similar_functions(sources.clone(), 0.75, 10, 3, &[], &PathFilters::default());
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
        assert!(
            find_similar_functions(sources.clone(), 0.99, 10, 3, &[], &PathFilters::default())
                .is_empty()
        );
    }

    #[test]
    fn index_skips_nested_functions_small_functions_and_covered_pairs() {
        let base = kinds(1, 80);
        let outer = sig("outer", &base, 0, 60);
        let inner = sig("inner", &base, 10, 50);
        let same_file = vec![FunctionSource {
            id: "a.js".into(),
            format: "javascript".into(),
            real_path: String::new(),
            functions: vec![outer.clone(), inner],
        }];
        assert!(
            find_similar_functions(same_file.clone(), 0.5, 10, 3, &[], &PathFilters::default())
                .is_empty()
        );

        let two = vec![
            FunctionSource {
                id: "a.js".into(),
                format: "javascript".into(),
                real_path: String::new(),
                functions: vec![outer.clone()],
            },
            FunctionSource {
                id: "b.js".into(),
                format: "javascript".into(),
                real_path: String::new(),
                functions: vec![sig("copy", &base, 0, 60)],
            },
        ];
        assert_eq!(
            find_similar_functions(two.clone(), 0.5, 10, 3, &[], &PathFilters::default()).len(),
            1
        );
        assert!(
            find_similar_functions(two.clone(), 0.5, 100, 3, &[], &PathFilters::default())
                .is_empty(),
            "min_tokens"
        );
        assert!(
            find_similar_functions(two.clone(), 0.5, 10, 100, &[], &PathFilters::default())
                .is_empty(),
            "min_lines"
        );

        let mut exact = make_clone(&two[0], &outer, &two[1], &two[1].functions[0], 1.0);
        exact.kind = CloneKind::Exact;
        exact.similarity = None;
        assert!(
            find_similar_functions(two.clone(), 0.5, 10, 3, &[exact], &PathFilters::default())
                .is_empty(),
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
                real_path: String::new(),
                functions: vec![sig("orig", &base, 0, 60)],
            },
            FunctionSource {
                id: "b.py".into(),
                format: "python".into(),
                real_path: String::new(),
                functions: vec![foreign],
            },
        ];
        assert!(
            find_similar_functions(sources, 0.5, 10, 3, &[], &PathFilters::default()).is_empty()
        );
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
            real_path: String::new(),
            functions: vec![sig("far", &far, 0, 60), sig("near", &near, 70, 130)],
        }];
        let index = SimilarityIndex::build(sources, 10, 3);
        let hits = index.query(&sig("q", &base, 0, 60), 0.5);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].1, 1, "near first");
        assert!(hits[0].2 > hits[1].2);
    }

    #[test]
    fn query_clones_pair_a_snippet_with_the_index_unless_already_covered() {
        let base = kinds(1, 80);
        let mut near = base.clone();
        near[3] = 99;
        let sources = vec![FunctionSource {
            id: "lib.js".into(),
            format: "javascript".into(),
            real_path: String::new(),
            functions: vec![sig("near", &near, 0, 60)],
        }];
        let index = SimilarityIndex::build(sources, 10, 3);
        let snippet = FunctionSource {
            id: "snippet://check".into(),
            format: "javascript".into(),
            real_path: String::new(),
            functions: vec![sig("q", &base, 0, 60)],
        };
        let found = index.query_clones(&snippet, 0.5, &[]);
        assert_eq!(found.len(), 1);
        let clone = &found[0];
        assert_eq!(clone.kind, CloneKind::Similar);
        assert_eq!(clone.similarity_method, Some(SimilarityMethod::Ast));
        let ids = [&clone.fragment_a.source_id, &clone.fragment_b.source_id];
        assert!(ids.contains(&&"snippet://check".to_string()), "{ids:?}");
        let mut exact = clone.clone();
        exact.kind = CloneKind::Exact;
        assert!(
            index.query_clones(&snippet, 0.5, &[exact]).is_empty(),
            "already reported"
        );
    }

    /// A source holding one function, for the tests below.
    fn source(id: &str, format: &str, f: FunctionSig) -> FunctionSource {
        FunctionSource {
            id: id.into(),
            format: format.into(),
            real_path: String::new(),
            functions: vec![f],
        }
    }

    #[test]
    fn copies_paired_with_a_shared_first_copy_are_reported_once() {
        let base = kinds(1, 80);
        let sources = vec![
            source("a.js", "javascript", sig("one", &base, 0, 60)),
            source("b.js", "javascript", sig("two", &base, 0, 60)),
            source("c.js", "javascript", sig("three", &base, 0, 60)),
        ];
        let exact = |x: usize, y: usize| {
            let (sx, sy) = (&sources[x], &sources[y]);
            let mut clone = make_clone(sx, &sx.functions[0], sy, &sy.functions[0], 1.0);
            clone.kind = CloneKind::Exact;
            clone.similarity = None;
            clone
        };
        let (ab, ac) = (exact(0, 1), exact(0, 2));
        // Detection pairs every copy with the first one only: b ~ c is implied.
        let none = PathFilters::default();
        let both = find_similar_functions(sources.clone(), 0.5, 10, 3, &[ab.clone(), ac], &none);
        assert!(both.is_empty(), "{both:?}");
        // With a clone for b alone, c is no copy of anything reported yet.
        let one = find_similar_functions(sources, 0.5, 10, 3, &[ab], &none);
        let pairs: Vec<(&str, &str)> = one
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.as_str(),
                    c.fragment_b.source_id.as_str(),
                )
            })
            .collect();
        assert_eq!(pairs, vec![("a.js", "c.js"), ("b.js", "c.js")]);
    }

    #[test]
    fn path_filters_drop_function_pairs_as_they_drop_token_clones() {
        let base = kinds(1, 80);
        let sources = vec![
            source("/repo/app/x.js", "javascript", sig("x", &base, 0, 60)),
            source("/repo/app/y.js", "javascript", sig("y", &base, 0, 60)),
        ];
        let roots = [std::path::PathBuf::from("/repo/app")];
        let local = PathFilters {
            skip_local: true,
            scan_roots: &roots,
            isolated_groups: &[],
        };
        let none = PathFilters::default();
        assert_eq!(
            find_similar_functions(sources.clone(), 0.5, 10, 3, &[], &none).len(),
            1
        );
        assert!(find_similar_functions(sources, 0.5, 10, 3, &[], &local).is_empty());
    }

    #[test]
    fn a_pair_takes_the_format_of_its_first_fragment() {
        let base = kinds(1, 80);
        let ts = source("a.ts", "typescript", sig("a", &base, 0, 60));
        let vue = source("b.vue:javascript", "javascript", sig("b", &base, 0, 60));
        for sources in [vec![ts.clone(), vue.clone()], vec![vue, ts]] {
            let found = find_similar_functions(sources, 0.5, 10, 3, &[], &PathFilters::default());
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].fragment_a.source_id, "a.ts");
            assert_eq!(
                found[0].format, "typescript",
                "whatever the order of the sources"
            );
        }
    }

    #[test]
    fn the_size_limits_read_the_code_size_when_one_is_given() {
        let base = kinds(1, 80);
        let plain = sig("plain", &base, 0, 60);
        assert_eq!(plain.clone().with_code_size(None), plain);
        let getter = sig("getter", &base, 0, 60).with_code_size(Some(CodeSize {
            tokens: 12,
            lines: 1,
        }));
        assert_eq!((getter.token_count, getter.code_lines), (12, 1));
        assert_eq!(getter.line_span(), 60, "the span itself stays");
        let sources = vec![
            source("a.py", "python", getter),
            source("b.py", "python", sig("copy", &base, 0, 60)),
        ];
        assert!(
            find_similar_functions(sources, 0.5, 10, 3, &[], &PathFilters::default()).is_empty(),
            "a docstring does not carry a short function past the limits"
        );
    }
}
