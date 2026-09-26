//! Semantic clones (Type-4, `--semantic`, experimental).
//!
//! Two functions are a semantic clone when they do the same thing but are
//! written differently — renamed, restructured, or in another language, like
//! a validation rule implemented once in a Rust backend and again in a Svelte
//! frontend. Token-based detection cannot see that; embeddings can: an
//! [`Embedder`] turns the code of every function into a vector, and functions
//! whose vectors point the same way are candidates.
//!
//! A pair of functions `a` and `b` is reported when
//!
//! 1. they live in different files, neither calls the other by name, the
//!    clones already found do not cover both (90% of the lines of each),
//!    and the path filters (`--skip-local`, `--skip-isolated`) allow the
//!    pair. A pair ruled out here is left out of each function's matches
//!    altogether, so a copy that token detection already reported does not
//!    stand in the way of a function's real semantic match;
//! 2. `b` is the closest match of `a` among the functions of `b`'s grammar,
//!    or within [`NEAR_BEST`] of it, and the same holds for `a` among the
//!    functions of `a`'s grammar (a mutual near-best match): three
//!    implementations of one feature make three pairs. A pair that is not
//!    each other's very best must clear [`group_floor`], higher than the
//!    threshold, so a function's weaker neighbours stay out;
//! 3. the cosine similarity of their vectors reaches the threshold; and
//! 4. the similarity stands out: it is at least [`MIN_Z`] standard
//!    deviations above the mean similarity of `a` to the functions of `b`'s
//!    grammar, and of `b` to the functions of `a`'s grammar, each background
//!    leaving out the function's closest matches (see `TRIM`).
//!
//! Rule 2 keeps a function that resembles many others (a request handler, a
//! getter) from pairing with each of them. Rule 4 is what makes one threshold
//! work across languages: two functions in different languages score lower
//! than two in one language whatever they do, so a cosine cut-off low enough
//! for a Rust/TypeScript pair lets through unrelated pairs within one
//! language, while the distance from each function's own background does not
//! depend on the language pair. Rule 1 drops pairs that are related rather
//! than duplicated: a function and a helper it calls, or two functions of one
//! file, which share names and context.

use cpd_core::detect::PathLabel;
use cpd_core::models::{CloneKind, CpdClone, Fragment, Location};
use cpd_core::paths::clean_source_id;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

/// How many standard deviations above a function's mean similarity to a
/// grammar a pair must score (rule 4 of the module docs).
pub const MIN_Z: f32 = 3.0;
/// Best matches left out of a function's background when its z-score is
/// computed (rule 4 of the module docs).
const TRIM: usize = TOP;
/// How far below a function's best match another match may score and still
/// count as a best match (rule 2 of the module docs).
pub const NEAR_BEST: f32 = 0.05;
/// Matches remembered per function and grammar; a feature implemented more
/// often than this in one language reports its closest copies only.
const TOP: usize = 8;
/// A background of fewer functions than this (outside the function's own
/// file) is too thin for a z-score; rule 4 is then not applied for it.
pub const MIN_BACKGROUND: usize = 8;
/// A callee name shorter than this is too common to tell a call from a
/// namesake (`new`, `get`, `run`), so it does not exclude a pair.
const MIN_CALLEE_NAME: usize = 5;
/// Rows of the similarity matrix computed together, so each column vector
/// is read from memory once per block instead of once per row.
const ROW_BLOCK: usize = 32;

/// Turns function source code into vectors.
pub trait Embedder: Send + Sync {
    /// One vector per text, in input order. Every vector must have the same
    /// length; it need not be normalized.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String>;
}

/// One function of a source, ready to embed.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticUnit {
    /// Grammar of the extractor that found the function (`oxc`, `rust`, ...).
    /// Mutual best matches are taken per grammar, and z-scores against the
    /// functions of one grammar.
    pub grammar: &'static str,
    /// Declared or inferred name (`<arrow>` / `<anonymous>` when none).
    pub name: String,
    pub start: Location,
    pub end: Location,
    /// Inclusive detection-token index range inside the owning source.
    pub range: [u32; 2],
    /// Detection tokens covered by the function.
    pub token_count: u32,
    /// The text given to the embedder: the function's code without comments.
    pub text: String,
}

impl SemanticUnit {
    /// Attach the token range from the owning source's token spans. Returns
    /// `None` when no detection token lies inside the function (a body of
    /// comments or type declarations only).
    pub fn build(
        grammar: &'static str,
        name: String,
        start: Location,
        end: Location,
        text: String,
        spans: &[(Location, Location)],
    ) -> Option<Self> {
        let (first, last) = cpd_core::similarity::token_range(spans, &start, &end)?;
        if text.trim().is_empty() {
            return None;
        }
        Some(Self {
            grammar,
            name,
            start,
            end,
            range: [first as u32, (last - 1) as u32],
            token_count: (last - first) as u32,
            text,
        })
    }

    /// Lines spanned, in jscpd's `end - start` convention.
    pub fn line_span(&self) -> u32 {
        self.end.line.saturating_sub(self.start.line)
    }
}

/// The functions of one source, as the semantic search needs them.
#[derive(Debug, Clone)]
pub struct UnitSource {
    /// Source id; an embedded block keeps its `path:format` form.
    pub id: String,
    pub format: String,
    pub units: Vec<SemanticUnit>,
    /// The source's place for the path filters; pairs whose labels skip
    /// each other are never compared. Default: filters off.
    pub path_label: PathLabel,
}

/// Settings of the semantic pass.
#[derive(Debug, Clone, Copy)]
pub struct SemanticParams {
    /// Lowest cosine similarity reported (rule 3 of the module docs).
    pub threshold: f32,
    /// Functions with fewer detection tokens are not embedded.
    pub min_tokens: usize,
    /// Functions spanning fewer lines are not embedded.
    pub min_lines: usize,
    /// Which pairs to look for: within one language, across languages, or
    /// both.
    pub scope: SemanticScope,
}

/// Which pairs `--semantic` reports. A language is a grammar: a Svelte
/// component's script and a `.ts` file are one language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SemanticScope {
    /// Every pair.
    #[default]
    All,
    /// Pairs within one language: similar implementations of one feature.
    Same,
    /// Pairs across languages: a rule written once per side.
    Cross,
}

impl SemanticScope {
    pub const NAMES: &'static str = "all, same, cross";

    pub fn as_str(self) -> &'static str {
        match self {
            SemanticScope::All => "all",
            SemanticScope::Same => "same",
            SemanticScope::Cross => "cross",
        }
    }

    fn allows(self, same_language: bool) -> bool {
        match self {
            SemanticScope::All => true,
            SemanticScope::Same => same_language,
            SemanticScope::Cross => !same_language,
        }
    }
}

impl std::str::FromStr for SemanticScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(SemanticScope::All),
            "same" => Ok(SemanticScope::Same),
            "cross" => Ok(SemanticScope::Cross),
            other => Err(format!(
                "unknown scope '{other}': must be one of: {}",
                SemanticScope::NAMES
            )),
        }
    }
}

/// Find semantic clones among the functions of `sources`. Pairs already
/// covered by a clone in `existing` (an exact, renamed or similar match
/// spanning both functions) are left out before the best matches are taken,
/// so nothing is reported twice and a copy never hides a real match.
///
/// Fails only when the embedder does.
pub fn find_semantic_clones(
    sources: &[UnitSource],
    embedder: &dyn Embedder,
    params: &SemanticParams,
    existing: &[CpdClone],
) -> Result<Vec<CpdClone>, String> {
    let items = eligible_items(sources, params);
    if items.len() < 2 {
        return Ok(Vec::new());
    }
    let unit = |item: &Item| &sources[item.source].units[item.unit];
    let texts: Vec<&str> = items.iter().map(|item| unit(item).text.as_str()).collect();
    let vectors = embedder.embed(&texts)?;
    let space = VectorSpace::new(&vectors, texts.len())?;

    let grammars = grammar_ids(&items, |item| unit(item).grammar);
    let mut related = call_pairs(&items, |item| unit(item));
    for (list, covered) in related
        .iter_mut()
        .zip(covered_pairs(&items, sources, existing))
    {
        if !covered.is_empty() {
            list.extend(covered);
            list.sort_unstable();
            list.dedup();
        }
    }
    let labels: Vec<&PathLabel> = items
        .iter()
        .map(|item| &sources[item.source].path_label)
        .collect();
    let rows = space.scan(&items, &grammars.of_item, grammars.count, &related, &labels);
    let judge = pair_judge(&items, sources, &rows, &grammars.of_item, params);

    let mut clones = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let own = grammars.of_item[i];
        let targets = row
            .iter()
            .enumerate()
            .filter(|&(grammar, _)| params.scope.allows(grammar == own));
        for (_, background) in targets {
            for (j, similarity) in background.near_best() {
                // Each mutual pair is seen from both ends; keep one.
                if j <= i || !rows[j][own].is_near_best(i) {
                    continue;
                }
                // Two functions that are each other's best match need the
                // threshold; a further member of a group needs more.
                let mutual_best = background.best() == Some(j) && rows[j][own].best() == Some(i);
                let floor = match mutual_best {
                    true => params.threshold,
                    false => group_floor(params.threshold),
                };
                if similarity < floor {
                    continue;
                }
                if let Some(clone) = judge(i, j, similarity) {
                    clones.push(clone);
                }
            }
        }
    }
    let mut clones = drop_nested(clones);
    clones.sort_by(|x, y| x.position_key().cmp(&y.position_key()));
    Ok(clones)
}

/// The similarity a pair needs when the two functions are near-best but not
/// best matches of each other: halfway from the threshold to identical
/// (0.8 at the default 0.6), so a third copy of a feature is reported while
/// the weaker neighbours of a function stay out.
pub fn group_floor(threshold: f32) -> f32 {
    threshold + (1.0 - threshold) / 2.0
}

/// The rules a mutual near-best pair must still pass, as a closure over the
/// run's items, rows and sources; see [`find_semantic_clones`].
fn pair_judge<'a>(
    items: &'a [Item],
    sources: &'a [UnitSource],
    rows: &'a [Vec<Background>],
    grammar_of: &'a [usize],
    params: &'a SemanticParams,
) -> impl Fn(usize, usize, f32) -> Option<CpdClone> + 'a {
    move |i, j, similarity| {
        if similarity < params.threshold {
            return None;
        }
        let z_i = rows[i][grammar_of[j]].z(similarity);
        let z_j = rows[j][grammar_of[i]].z(similarity);
        if z_i.into_iter().chain(z_j).any(|z| z < MIN_Z) {
            return None;
        }
        let (a, b) = (&items[i], &items[j]);
        let (src_a, src_b) = (&sources[a.source], &sources[b.source]);
        let unit_a = &src_a.units[a.unit];
        let unit_b = &src_b.units[b.unit];
        Some(make_clone(src_a, unit_a, src_b, unit_b, similarity))
    }
}

/// Drop a pair whose functions both sit inside the functions of another
/// reported pair of the same two sources — helpers nested in two copies of
/// a component pair up too, and the outer pair already says it all.
fn drop_nested(mut clones: Vec<CpdClone>) -> Vec<CpdClone> {
    let lines = |f: &Fragment| f.end.line - f.start.line;
    clones.sort_by_key(|c| std::cmp::Reverse(lines(&c.fragment_a) + lines(&c.fragment_b)));
    let inside = |inner: &Fragment, outer: &Fragment| {
        inner.source_id == outer.source_id
            && outer.start.line <= inner.start.line
            && inner.end.line <= outer.end.line
    };
    let mut kept: Vec<CpdClone> = Vec::with_capacity(clones.len());
    for clone in clones {
        let nested = kept.iter().any(|outer| {
            (inside(&clone.fragment_a, &outer.fragment_a)
                && inside(&clone.fragment_b, &outer.fragment_b))
                || (inside(&clone.fragment_a, &outer.fragment_b)
                    && inside(&clone.fragment_b, &outer.fragment_a))
        });
        if !nested {
            kept.push(clone);
        }
    }
    kept
}

/// One eligible function: where it lives and which file it belongs to.
struct Item {
    source: usize,
    unit: usize,
    /// Index of the host file: an embedded block counts as its host file.
    file: u32,
}

fn eligible_items(sources: &[UnitSource], params: &SemanticParams) -> Vec<Item> {
    let mut files: FxHashMap<&str, u32> = FxHashMap::default();
    let mut items = Vec::new();
    for (si, src) in sources.iter().enumerate() {
        let next = files.len() as u32;
        let file = *files.entry(clean_source_id(&src.id)).or_insert(next);
        for (ui, unit) in src.units.iter().enumerate() {
            if (unit.token_count as usize) < params.min_tokens
                || (unit.line_span() as usize) < params.min_lines
            {
                continue;
            }
            items.push(Item {
                source: si,
                unit: ui,
                file,
            });
        }
    }
    items
}

struct Grammars {
    of_item: Vec<usize>,
    count: usize,
}

fn grammar_ids(items: &[Item], grammar: impl Fn(&Item) -> &'static str) -> Grammars {
    let mut ids: Vec<&'static str> = Vec::new();
    let of_item = items
        .iter()
        .map(|item| {
            let g = grammar(item);
            ids.iter().position(|&known| known == g).unwrap_or_else(|| {
                ids.push(g);
                ids.len() - 1
            })
        })
        .collect();
    Grammars {
        of_item,
        count: ids.len(),
    }
}

/// For every item, the sorted items it calls or is called by (rule 1). A
/// call is the callee's name followed by `(`; a function's own name is never
/// a call, so the header `fn name(` and recursion do not count, and two
/// namesakes (a port keeps the name) are not mistaken for caller and callee.
fn call_pairs<'u>(items: &[Item], unit: impl Fn(&Item) -> &'u SemanticUnit) -> Vec<Vec<usize>> {
    let mut by_name: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    for (i, item) in items.iter().enumerate() {
        let name = unit(item).name.as_str();
        if name.chars().count() >= MIN_CALLEE_NAME && !name.starts_with('<') {
            by_name.entry(name).or_default().push(i);
        }
    }
    let mut related: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    if by_name.is_empty() {
        return related;
    }
    for (i, item) in items.iter().enumerate() {
        let own = unit(item);
        for callee in called_names(&own.text) {
            if callee == own.name {
                continue;
            }
            for &j in by_name.get(callee).map(Vec::as_slice).unwrap_or_default() {
                if j != i {
                    related[i].push(j);
                    related[j].push(i);
                }
            }
        }
    }
    for list in &mut related {
        list.sort_unstable();
        list.dedup();
    }
    related
}

/// Identifiers directly followed by `(` (spaces allowed in between).
fn called_names(text: &str) -> impl Iterator<Item = &str> {
    let bytes = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            let c = bytes[i];
            if !(c.is_ascii_alphabetic() || c == b'_' || c == b'$') {
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'$'))
            {
                i += 1;
            }
            let end = i;
            let mut k = i;
            while k < bytes.len() && matches!(bytes[k], b' ' | b'\t') {
                k += 1;
            }
            if bytes.get(k) == Some(&b'(') {
                return Some(&text[start..end]);
            }
        }
        None
    })
}

/// Unit-normalized vectors stored row-major, one row per item.
struct VectorSpace {
    dims: usize,
    data: Vec<f32>,
}

impl VectorSpace {
    fn new(vectors: &[Vec<f32>], expected: usize) -> Result<Self, String> {
        if vectors.len() != expected {
            return Err(format!(
                "the embedding model returned {} vectors for {} functions",
                vectors.len(),
                expected
            ));
        }
        let dims = vectors.first().map_or(0, Vec::len);
        if dims == 0 {
            return Err("the embedding model returned empty vectors".to_string());
        }
        let mut data = Vec::with_capacity(dims * vectors.len());
        for v in vectors {
            if v.len() != dims {
                return Err(format!(
                    "the embedding model returned vectors of {} and {} dimensions",
                    dims,
                    v.len()
                ));
            }
            let norm = v
                .iter()
                .map(|x| f64::from(*x) * f64::from(*x))
                .sum::<f64>()
                .sqrt();
            // A zero or non-finite vector matches nothing: leave it zero.
            let scale = if norm.is_finite() && norm > 0.0 {
                (1.0 / norm) as f32
            } else {
                0.0
            };
            data.extend(
                v.iter()
                    .map(|x| if x.is_finite() { x * scale } else { 0.0 }),
            );
        }
        Ok(Self { dims, data })
    }

    fn row(&self, i: usize) -> &[f32] {
        &self.data[i * self.dims..(i + 1) * self.dims]
    }

    /// Every item's background per grammar: similarity statistics over the
    /// items of other files that it neither calls nor is called by and that
    /// the path filters let it pair with.
    fn scan(
        &self,
        items: &[Item],
        grammar_of: &[usize],
        grammars: usize,
        related: &[Vec<usize>],
        labels: &[&PathLabel],
    ) -> Vec<Vec<Background>> {
        let n = items.len();
        (0..n.div_ceil(ROW_BLOCK))
            .into_par_iter()
            .flat_map_iter(|block| {
                let rows = block * ROW_BLOCK..((block + 1) * ROW_BLOCK).min(n);
                let mut out = vec![vec![Background::EMPTY; grammars]; rows.len()];
                for j in 0..n {
                    let column = self.row(j);
                    for (r, i) in rows.clone().enumerate() {
                        if items[i].file == items[j].file
                            || related[i].binary_search(&j).is_ok()
                            || labels[i].skips(labels[j])
                        {
                            continue;
                        }
                        let sim = dot(self.row(i), column);
                        out[r][grammar_of[j]].add(sim, j);
                    }
                }
                out
            })
            .collect()
    }
}

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    // Eight independent lanes let the compiler vectorize the loop.
    let mut lanes = [0f32; 8];
    let (chunks_a, chunks_b) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = chunks_a
        .remainder()
        .iter()
        .zip(chunks_b.remainder())
        .map(|(x, y)| x * y)
        .sum();
    for (x, y) in chunks_a.zip(chunks_b) {
        for k in 0..8 {
            lanes[k] += x[k] * y[k];
        }
    }
    lanes.iter().sum::<f32>() + tail
}

/// Similarity statistics of one item against the items of one grammar,
/// with its [`TOP`] closest matches, best first.
#[derive(Debug, Clone, Copy)]
struct Background {
    count: u32,
    sum: f64,
    sum_sq: f64,
    top: [(f32, u32); TOP],
    len: u8,
}

impl Background {
    const EMPTY: Self = Self {
        count: 0,
        sum: 0.0,
        sum_sq: 0.0,
        top: [(f32::NEG_INFINITY, u32::MAX); TOP],
        len: 0,
    };

    fn add(&mut self, sim: f32, item: usize) {
        self.count += 1;
        self.sum += f64::from(sim);
        self.sum_sq += f64::from(sim) * f64::from(sim);
        // Items arrive in ascending order, and a tie ranks after the
        // earlier item: results do not depend on how rows were computed.
        let len = self.len as usize;
        let at = self.top[..len].partition_point(|&(s, _)| s >= sim);
        if at == TOP {
            return;
        }
        let end = len.min(TOP - 1);
        self.top.copy_within(at..end, at + 1);
        self.top[at] = (sim, item as u32);
        self.len = (len + 1).min(TOP) as u8;
    }

    /// The matches within [`NEAR_BEST`] of the best one, with their scores.
    fn near_best(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        let floor = self.top[0].0 - NEAR_BEST;
        self.top[..self.len as usize]
            .iter()
            .take_while(move |&&(s, _)| s >= floor)
            .map(|&(s, item)| (item as usize, s))
    }

    fn best(&self) -> Option<usize> {
        (self.len > 0).then_some(self.top[0].1 as usize)
    }

    fn is_near_best(&self, item: usize) -> bool {
        self.near_best().any(|(i, _)| i == item)
    }

    /// Standard score of `sim` against this background, leaving out the
    /// [`TRIM`] best matches: they are the candidates being judged, and a
    /// feature implemented three times must not hide each copy behind the
    /// others. `None` when what remains is too small or flat to judge.
    fn z(&self, sim: f32) -> Option<f32> {
        let trim = TRIM.min(self.len as usize);
        let count = self.count as usize - trim;
        if count < MIN_BACKGROUND {
            return None;
        }
        let top = &self.top[..trim];
        let sum = self.sum - top.iter().map(|&(s, _)| f64::from(s)).sum::<f64>();
        let sum_sq = self.sum_sq
            - top
                .iter()
                .map(|&(s, _)| f64::from(s) * f64::from(s))
                .sum::<f64>();
        let n = count as f64;
        let mean = sum / n;
        let std = (sum_sq / n - mean * mean).max(0.0).sqrt();
        (std > 1e-6).then(|| ((f64::from(sim) - mean) / std) as f32)
    }
}

/// True when the clones already found between the two sources cover at
/// least 90% of the lines of both functions, together: a function copied
/// with one edited line is two exact clones with a gap, and that pair is
/// already reported.
/// For every item, the sorted items whose pair the clones in `existing`
/// already cover (rule 1). Only functions that some clone between their two
/// sources meets are tried, so the cost follows the clones, not the items.
fn covered_pairs(items: &[Item], sources: &[UnitSource], existing: &[CpdClone]) -> Vec<Vec<usize>> {
    let mut covered: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    if existing.is_empty() {
        return covered;
    }
    let source_of: FxHashMap<&str, usize> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut items_of: Vec<Vec<usize>> = vec![Vec::new(); sources.len()];
    for (i, item) in items.iter().enumerate() {
        items_of[item.source].push(i);
    }
    // The clones between two different sources, keyed lower index first.
    let mut between: FxHashMap<(usize, usize), Vec<&CpdClone>> = FxHashMap::default();
    for clone in existing {
        let a = source_of.get(clone.fragment_a.source_id.as_str());
        let b = source_of.get(clone.fragment_b.source_id.as_str());
        if let (Some(&a), Some(&b)) = (a, b)
            && a != b
        {
            between.entry((a.min(b), a.max(b))).or_default().push(clone);
        }
    }
    let unit = |i: usize| &sources[items[i].source].units[items[i].unit];
    // The items of `source` that one of `clones` meets on that source's side.
    let met = |source: usize, clones: &[&CpdClone]| -> Vec<usize> {
        let id = sources[source].id.as_str();
        items_of[source]
            .iter()
            .copied()
            .filter(|&i| {
                clones.iter().any(|c| {
                    [&c.fragment_a, &c.fragment_b]
                        .into_iter()
                        .any(|f| f.source_id == id && meets(f, unit(i)))
                })
            })
            .collect()
    };
    for (&(sa, sb), clones) in &between {
        for i in met(sa, clones) {
            for j in met(sb, clones) {
                if items[i].file != items[j].file
                    && covered_by(&sources[sa].id, unit(i), &sources[sb].id, unit(j), clones)
                {
                    covered[i].push(j);
                    covered[j].push(i);
                }
            }
        }
    }
    for list in &mut covered {
        list.sort_unstable();
        list.dedup();
    }
    covered
}

/// Whether the fragment and the function share a line.
fn meets(frag: &Fragment, f: &SemanticUnit) -> bool {
    frag.start.line <= f.end.line && f.start.line <= frag.end.line
}

/// Whether `clones` cover 90% of the lines of both `a` (in source `id_a`)
/// and `b` (in `id_b`), counting only clones that meet both functions.
fn covered_by(
    id_a: &str,
    a: &SemanticUnit,
    id_b: &str,
    b: &SemanticUnit,
    clones: &[&CpdClone],
) -> bool {
    let lines = |f: &SemanticUnit| vec![false; (f.end.line - f.start.line + 1) as usize];
    let (mut in_a, mut in_b) = (lines(a), lines(b));
    // Mark the lines of `f` inside `frag`; false when they do not meet.
    let mark = |covered: &mut [bool], frag: &Fragment, f: &SemanticUnit| {
        let lo = frag.start.line.max(f.start.line);
        let hi = frag.end.line.min(f.end.line);
        for line in lo..=hi {
            covered[(line - f.start.line) as usize] = true;
        }
    };
    for c in clones {
        let (frag_a, frag_b) = if c.fragment_a.source_id == id_a && c.fragment_b.source_id == id_b {
            (&c.fragment_a, &c.fragment_b)
        } else if c.fragment_a.source_id == id_b && c.fragment_b.source_id == id_a {
            (&c.fragment_b, &c.fragment_a)
        } else {
            continue;
        };
        // A clone of `a` with some other function of `b`'s file says
        // nothing about this pair.
        if meets(frag_a, a) && meets(frag_b, b) {
            mark(&mut in_a, frag_a, a);
            mark(&mut in_b, frag_b, b);
        }
    }
    let share =
        |covered: &[bool]| covered.iter().filter(|&&c| c).count() as f32 / covered.len() as f32;
    share(&in_a) >= 0.9 && share(&in_b) >= 0.9
}

fn make_clone(
    src_a: &UnitSource,
    a: &SemanticUnit,
    src_b: &UnitSource,
    b: &SemanticUnit,
    similarity: f32,
) -> CpdClone {
    let frag = |src: &UnitSource, f: &SemanticUnit| {
        Fragment::new(src.id.clone(), f.start.clone(), f.end.clone(), f.range)
    };
    // Deterministic fragment order: by source id, then position.
    let a_first = (src_a.id.as_str(), a.start.line) <= (src_b.id.as_str(), b.start.line);
    let ((first_src, first), (second_src, second)) = if a_first {
        ((src_a, a), (src_b, b))
    } else {
        ((src_b, b), (src_a, a))
    };
    CpdClone {
        format: first_src.format.clone(),
        fragment_a: frag(first_src, first),
        fragment_b: frag(second_src, second),
        token_count: first.token_count.min(second.token_count),
        is_new: false,
        kind: CloneKind::Semantic,
        similarity: Some(similarity.min(1.0)),
        similarity_method: None,
        unmatched_lines: [0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn loc(line: u32, offset: u32) -> Location {
        Location::new(line, 0, offset)
    }

    /// A function on lines `line..line+9` whose text is `text`.
    fn unit(grammar: &'static str, name: &str, line: u32, text: &str) -> SemanticUnit {
        SemanticUnit {
            grammar,
            name: name.to_string(),
            start: loc(line, line * 100),
            end: loc(line + 9, line * 100 + 90),
            range: [line * 10, line * 10 + 59],
            token_count: 60,
            text: text.to_string(),
        }
    }

    fn source(id: &str, format: &str, units: Vec<SemanticUnit>) -> UnitSource {
        UnitSource {
            id: id.to_string(),
            format: format.to_string(),
            units,
            path_label: PathLabel::default(),
        }
    }

    const PARAMS: SemanticParams = SemanticParams {
        threshold: 0.6,
        min_tokens: 50,
        min_lines: 5,
        scope: SemanticScope::All,
    };

    /// Embeds a text as the vector registered for its first word.
    struct Table(HashMap<String, Vec<f32>>);

    impl Embedder for Table {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
            texts
                .iter()
                .map(|t| {
                    let key = t.split_whitespace().next().unwrap_or_default();
                    self.0
                        .get(key)
                        .cloned()
                        .ok_or(format!("no vector for {key}"))
                })
                .collect()
        }
    }

    /// The two files of every clone found.
    fn pairs(found: &[CpdClone]) -> Vec<(&str, &str)> {
        found
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.as_str(),
                    c.fragment_b.source_id.as_str(),
                )
            })
            .collect()
    }

    /// A unit vector along `axis` blended with `noise` of axis `noise_axis`.
    fn vec_on(axis: usize, noise_axis: usize, noise: f32) -> Vec<f32> {
        let mut v = vec![0.0; 128];
        v[axis] = 1.0;
        v[noise_axis] += noise;
        v
    }

    /// Unrelated functions in files of their own, as many as a small
    /// project has, so every background is big enough for a z-score.
    const FILLERS: usize = 30;

    /// `sources` with a Rust and a TypeScript background around them, so
    /// every function has enough others to be judged against.
    fn with_backgrounds(mut sources: Vec<UnitSource>) -> Vec<UnitSource> {
        sources.extend(filler("back", "rust", "rust"));
        sources.extend(filler("front", "oxc", "typescript"));
        sources
    }

    fn filler(prefix: &str, grammar: &'static str, format: &str) -> Vec<UnitSource> {
        (0..FILLERS)
            .map(|k| {
                let text = format!("{prefix}{k} body");
                source(
                    &format!("{prefix}/filler{k}.x"),
                    format,
                    vec![unit(grammar, &format!("filler{k}"), 1, &text)],
                )
            })
            .collect()
    }

    /// An embedder knowing `named` and the filler vectors, which lie mostly
    /// on axes of their own.
    fn embedder(named: &[(&str, Vec<f32>)]) -> Table {
        let mut table: HashMap<String, Vec<f32>> = named
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        for (prefix, offset) in [("back", 64), ("front", 96)] {
            for k in 0..FILLERS {
                let mut v = vec![0.0; 128];
                v[offset + k] = 1.0;
                v[0] = 0.15;
                v[1] = 0.15;
                table.insert(format!("{prefix}{k}"), v);
            }
        }
        Table(table)
    }

    #[test]
    fn a_mutual_best_match_across_languages_is_a_semantic_clone() {
        let sources = with_backgrounds(vec![
            source(
                "backend/src/pricing.rs",
                "rust",
                vec![unit("rust", "cart_totals", 10, "totals-rs fn cart_totals")],
            ),
            source(
                "frontend/src/Cart.svelte:typescript",
                "typescript",
                vec![unit(
                    "oxc",
                    "computeTotals",
                    20,
                    "totals-ts function computeTotals",
                )],
            ),
        ]);
        let embedder = embedder(&[
            ("totals-rs", vec_on(0, 2, 0.5)),
            ("totals-ts", vec_on(0, 3, 0.6)),
        ]);
        let clones = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        assert_eq!(clones.len(), 1, "{clones:#?}");
        let c = &clones[0];
        assert_eq!(c.kind, CloneKind::Semantic);
        assert_eq!(c.format, "rust");
        assert_eq!(c.fragment_a.source_id, "backend/src/pricing.rs");
        assert_eq!(
            c.fragment_b.source_id,
            "frontend/src/Cart.svelte:typescript"
        );
        assert_eq!((c.fragment_a.start.line, c.fragment_b.start.line), (10, 20));
        let sim = c.similarity.unwrap();
        assert!((0.7..0.8).contains(&sim), "{sim}");
        assert_eq!(c.token_count, 60);
    }

    #[test]
    fn the_threshold_is_a_cosine_floor() {
        let sources = with_backgrounds(vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa x")]),
            source("b.ts", "typescript", vec![unit("oxc", "b", 1, "pb x")]),
        ]);
        // cos = 1 / (1 + 0.75^2) = 0.64
        let embedder = embedder(&[("pa", vec_on(4, 5, 0.75)), ("pb", vec_on(4, 6, 0.75))]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        assert_eq!(found.len(), 1);
        let sim = found[0].similarity.unwrap();
        let strict = SemanticParams {
            threshold: sim + 0.01,
            ..PARAMS
        };
        assert!(
            find_semantic_clones(&sources, &embedder, &strict, &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn only_the_best_match_of_a_function_is_paired() {
        // `b1` and `b2` both resemble `a`; `b1` more. `b2` finds `a` as its
        // best match, but `a` does not return the favour.
        let sources = with_backgrounds(vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa")]),
            source("b1.ts", "typescript", vec![unit("oxc", "b1", 1, "pb1")]),
            source("b2.ts", "typescript", vec![unit("oxc", "b2", 1, "pb2")]),
        ]);
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.3)),
            ("pb1", vec_on(4, 6, 0.3)),
            ("pb2", vec_on(4, 7, 0.5)),
        ]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        let pairs = pairs(&found);
        assert!(pairs.contains(&("a.rs", "b1.ts")), "{pairs:?}");
        assert!(
            !pairs.iter().any(|(x, y)| *x == "b2.ts" && *y == "a.rs"),
            "{pairs:?}"
        );
        assert!(!pairs.contains(&("a.rs", "b2.ts")), "{pairs:?}");
    }

    #[test]
    fn functions_of_one_file_are_never_paired() {
        let mut sources = vec![
            source(
                "cart.svelte:typescript",
                "typescript",
                vec![unit("oxc", "load", 1, "pa"), unit("oxc", "save", 20, "pb")],
            ),
            // The host file's markup is another source of the same file.
            source(
                "cart.svelte:javascript",
                "javascript",
                vec![unit("oxc", "other", 40, "pc")],
            ),
        ];
        sources.extend(filler("front", "oxc", "typescript"));
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.1)),
            ("pb", vec_on(4, 6, 0.1)),
            ("pc", vec_on(4, 7, 0.1)),
        ]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn a_function_and_the_helper_it_calls_are_not_a_clone() {
        let mut sources = vec![
            source(
                "routes.rs",
                "rust",
                vec![unit(
                    "rust",
                    "list_articles",
                    1,
                    "pa fn list_articles() { db.articles_page (1) }",
                )],
            ),
            source(
                "db.rs",
                "rust",
                vec![unit("rust", "articles_page", 1, "pb fn articles_page() {}")],
            ),
            // Its real counterpart, a little less similar than the callee.
            source(
                "db2.rs",
                "rust",
                vec![unit("rust", "fetch_page", 1, "pc fn fetch_page() {}")],
            ),
        ];
        sources.extend(filler("back", "rust", "rust"));
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.2)),
            ("pb", vec_on(4, 6, 0.2)),
            ("pc", vec_on(4, 7, 0.4)),
        ]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        let pairs = pairs(&found);
        assert!(pairs.contains(&("db2.rs", "routes.rs")), "{pairs:?}");
        assert!(
            !pairs.contains(&("db.rs", "routes.rs")),
            "a function never pairs with the helper it calls: {pairs:?}"
        );
    }

    #[test]
    fn a_path_filtered_best_match_leaves_room_for_the_allowed_one() {
        // `a` resembles its neighbour `n` most, but `--skip-local` forbids
        // that pair: `a` must still pair with `b` from the other scan root.
        let roots = [
            std::path::PathBuf::from("app"),
            std::path::PathBuf::from("web"),
        ];
        let filters = cpd_core::detect::PathFilters {
            skip_local: true,
            scan_roots: &roots,
            isolated_groups: &[],
        };
        let mut sources = vec![
            source("app/a.ts", "typescript", vec![unit("oxc", "a", 1, "pa")]),
            source("app/n.ts", "typescript", vec![unit("oxc", "n", 1, "pn")]),
            source("web/b.ts", "typescript", vec![unit("oxc", "b", 1, "pb")]),
        ];
        sources.extend(
            filler("front", "oxc", "typescript")
                .into_iter()
                .map(|mut src| {
                    src.id = format!("web/{}", src.id);
                    src
                }),
        );
        for src in &mut sources {
            src.path_label = filters.label(&src.id);
        }
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.1)),
            ("pn", vec_on(4, 6, 0.1)),
            ("pb", vec_on(4, 7, 0.4)),
        ]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        let pairs = pairs(&found);
        assert!(pairs.contains(&("app/a.ts", "web/b.ts")), "{pairs:?}");
        assert!(!pairs.contains(&("app/a.ts", "app/n.ts")), "{pairs:?}");
    }

    #[test]
    fn pairs_nested_in_a_reported_pair_are_dropped() {
        let pair = |a: (u32, u32), b: (u32, u32)| {
            let mut c = CpdClone::exact(
                "tsx",
                Fragment::new("Add.tsx", loc(a.0, 0), loc(a.1, 0), [0, 1]),
                Fragment::new("Edit.tsx", loc(b.0, 0), loc(b.1, 0), [0, 1]),
                50,
            );
            c.kind = CloneKind::Semantic;
            c
        };
        let kept = drop_nested(vec![
            pair((20, 30), (25, 35)),
            pair((1, 100), (1, 110)),
            pair((120, 130), (20, 30)),
        ]);
        let spans: Vec<(u32, u32)> = kept
            .iter()
            .map(|c| (c.fragment_a.start.line, c.fragment_b.start.line))
            .collect();
        assert_eq!(
            spans,
            vec![(1, 1), (120, 20)],
            "the inner pair goes, an unrelated one stays"
        );
    }

    #[test]
    fn three_implementations_of_one_feature_make_three_pairs() {
        let mut sources = vec![
            source("a.ts", "typescript", vec![unit("oxc", "a", 1, "pa")]),
            source("b.ts", "typescript", vec![unit("oxc", "b", 1, "pb")]),
            source("c.ts", "typescript", vec![unit("oxc", "c", 1, "pc")]),
        ];
        sources.extend(filler("front", "oxc", "typescript"));
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.30)),
            ("pb", vec_on(4, 6, 0.30)),
            ("pc", vec_on(4, 7, 0.35)),
        ]);
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[]).unwrap();
        assert_eq!(
            pairs(&found),
            vec![("a.ts", "b.ts"), ("a.ts", "c.ts"), ("b.ts", "c.ts")]
        );
    }

    #[test]
    fn scope_keeps_pairs_within_or_across_languages() {
        let sources = with_backgrounds(vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa")]),
            source("b.rs", "rust", vec![unit("rust", "b", 1, "pb")]),
            source("c.ts", "typescript", vec![unit("oxc", "c", 1, "pc")]),
        ]);
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.2)),
            ("pb", vec_on(4, 6, 0.2)),
            ("pc", vec_on(4, 7, 0.3)),
        ]);
        let with = |scope| {
            let params = SemanticParams { scope, ..PARAMS };
            let found = find_semantic_clones(&sources, &embedder, &params, &[]).unwrap();
            pairs(&found)
                .into_iter()
                .map(|(x, y)| format!("{x}~{y}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            with(SemanticScope::All),
            ["a.rs~b.rs", "a.rs~c.ts", "b.rs~c.ts"]
        );
        assert_eq!(with(SemanticScope::Same), ["a.rs~b.rs"]);
        assert_eq!(with(SemanticScope::Cross), ["a.rs~c.ts", "b.rs~c.ts"]);
        assert_eq!("cross".parse::<SemanticScope>(), Ok(SemanticScope::Cross));
        assert!(
            "both"
                .parse::<SemanticScope>()
                .unwrap_err()
                .contains("all, same, cross")
        );
    }

    #[test]
    fn namesakes_are_not_mistaken_for_caller_and_callee() {
        let related = call_pairs(
            &[
                Item {
                    source: 0,
                    unit: 0,
                    file: 0,
                },
                Item {
                    source: 0,
                    unit: 1,
                    file: 1,
                },
            ],
            |item| {
                static UNITS: std::sync::OnceLock<Vec<SemanticUnit>> = std::sync::OnceLock::new();
                &UNITS.get_or_init(|| {
                    vec![
                        unit(
                            "rust",
                            "segment",
                            1,
                            "pub fn segment(&self) { segment_inner(1) }",
                        ),
                        unit(
                            "oxc",
                            "segment",
                            1,
                            "async segment(n) { return segment (n - 1) }",
                        ),
                    ]
                })[item.unit]
            },
        );
        assert!(related.iter().all(Vec::is_empty), "{related:?}");
    }

    #[test]
    fn called_names_finds_calls_not_mentions() {
        let names: Vec<&str> =
            called_names("fn f(x) { g (x); let y = h; obj.method(1); $ref(2) }").collect();
        assert_eq!(names, vec!["f", "g", "method", "$ref"]);
    }

    #[test]
    fn a_pair_already_reported_as_a_clone_is_skipped() {
        let mut sources = vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa")]),
            source("b.rs", "rust", vec![unit("rust", "b", 1, "pb")]),
        ];
        sources.extend(filler("back", "rust", "rust"));
        let embedder = embedder(&[("pa", vec_on(4, 5, 0.1)), ("pb", vec_on(4, 6, 0.1))]);
        assert_eq!(
            find_semantic_clones(&sources, &embedder, &PARAMS, &[])
                .unwrap()
                .len(),
            1
        );
        let exact = CpdClone::exact(
            "rust",
            Fragment::new("a.rs", loc(1, 0), loc(10, 0), [0, 50]),
            Fragment::new("b.rs", loc(2, 0), loc(10, 0), [0, 50]),
            50,
        );
        assert!(
            find_semantic_clones(&sources, &embedder, &PARAMS, &[exact])
                .unwrap()
                .is_empty()
        );
        // Two clones with a gap cover the pair together; one alone does not.
        let half = |from: u32, to: u32| {
            CpdClone::exact(
                "rust",
                Fragment::new("b.rs", loc(from, 0), loc(to, 0), [0, 20]),
                Fragment::new("a.rs", loc(from, 0), loc(to, 0), [0, 20]),
                20,
            )
        };
        let first = half(1, 5);
        let second = half(6, 10);
        assert_eq!(
            find_semantic_clones(&sources, &embedder, &PARAMS, std::slice::from_ref(&first))
                .unwrap()
                .len(),
            1
        );
        assert!(
            find_semantic_clones(&sources, &embedder, &PARAMS, &[first, second])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_copy_already_found_does_not_hide_the_real_semantic_match() {
        // `copy` is `a` word for word, and the token passes reported it; `c`
        // does what `a` does in its own way. With the copy in the running,
        // `a`'s best match would be the copy, the covered pair would then be
        // dropped, and `c` would be lost with it.
        let mut sources = vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa")]),
            source("copy.rs", "rust", vec![unit("rust", "copy", 1, "pcopy")]),
            source("c.rs", "rust", vec![unit("rust", "c", 1, "pc")]),
        ];
        sources.extend(filler("back", "rust", "rust"));
        let embedder = embedder(&[
            ("pa", vec_on(4, 5, 0.1)),
            ("pcopy", vec_on(4, 5, 0.1)),
            ("pc", vec_on(4, 6, 0.9)),
        ]);
        let copy = CpdClone::exact(
            "rust",
            Fragment::new("a.rs", loc(1, 0), loc(10, 0), [0, 50]),
            Fragment::new("copy.rs", loc(1, 0), loc(10, 0), [0, 50]),
            50,
        );
        let found = find_semantic_clones(&sources, &embedder, &PARAMS, &[copy]).unwrap();
        assert_eq!(pairs(&found), vec![("a.rs", "c.rs")], "{found:#?}");
    }

    #[test]
    fn small_functions_are_not_embedded() {
        struct Refuse;
        impl Embedder for Refuse {
            fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
                Err(format!("asked to embed {}", texts.len()))
            }
        }
        let mut tiny = unit("rust", "a", 1, "pa");
        tiny.token_count = 10;
        let sources = vec![
            source("a.rs", "rust", vec![tiny]),
            source("b.rs", "rust", vec![unit("rust", "b", 1, "pb")]),
        ];
        // One eligible function cannot pair with anything: no request at all.
        assert!(
            find_semantic_clones(&sources, &Refuse, &PARAMS, &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn embedder_errors_and_bad_vectors_are_reported() {
        struct Fixed(Vec<Vec<f32>>);
        impl Embedder for Fixed {
            fn embed(&self, _: &[&str]) -> Result<Vec<Vec<f32>>, String> {
                Ok(self.0.clone())
            }
        }
        let sources = vec![
            source("a.rs", "rust", vec![unit("rust", "a", 1, "pa")]),
            source("b.rs", "rust", vec![unit("rust", "b", 1, "pb")]),
        ];
        let short = find_semantic_clones(&sources, &Fixed(vec![vec![1.0]]), &PARAMS, &[]);
        assert!(short.unwrap_err().contains("1 vectors for 2 functions"));
        let ragged = Fixed(vec![vec![1.0, 0.0], vec![1.0]]);
        let err = find_semantic_clones(&sources, &ragged, &PARAMS, &[]).unwrap_err();
        assert!(err.contains("2 and 1 dimensions"), "{err}");
    }

    #[test]
    fn z_leaves_out_the_closest_matches_and_needs_eight_more() {
        let mut bg = Background::EMPTY;
        // Eight close matches, the part of the background that is trimmed.
        for k in 0..TOP {
            bg.add(0.9 - 0.01 * k as f32, k);
        }
        // Seven unrelated functions: too few to judge by.
        for k in 0..7 {
            bg.add(0.1 + 0.01 * k as f32, TOP + k);
        }
        assert_eq!(bg.z(0.9), None);
        bg.add(0.12, TOP + 7);
        let z = bg.z(0.9).unwrap();
        // Against the unrelated functions only, 0.9 stands out by far; the
        // close matches alone would have hidden it.
        assert!(z > 20.0, "{z}");
        let near: Vec<usize> = bg.near_best().map(|(item, _)| item).collect();
        assert_eq!(near, vec![0, 1, 2, 3, 4, 5], "within NEAR_BEST of 0.9");
    }

    #[test]
    fn dot_matches_a_plain_sum() {
        let a: Vec<f32> = (0..19).map(|i| i as f32 * 0.5).collect();
        let b: Vec<f32> = (0..19).map(|i| 1.0 - i as f32 * 0.25).collect();
        let plain: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((dot(&a, &b) - plain).abs() < 1e-3);
    }

    #[test]
    fn unit_build_maps_bytes_to_tokens() {
        let spans: Vec<(Location, Location)> = (0..10)
            .map(|i| (loc(i + 1, i * 10), loc(i + 1, i * 10 + 5)))
            .collect();
        let u = SemanticUnit::build(
            "rust",
            "f".into(),
            loc(3, 20),
            loc(8, 75),
            "fn f() {}".into(),
            &spans,
        )
        .unwrap();
        assert_eq!((u.range, u.token_count, u.line_span()), ([2, 7], 6, 5));
        assert!(
            SemanticUnit::build(
                "rust",
                "g".into(),
                loc(3, 20),
                loc(8, 75),
                "  ".into(),
                &spans
            )
            .is_none()
        );
    }
}
