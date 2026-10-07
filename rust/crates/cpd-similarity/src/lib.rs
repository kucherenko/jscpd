//! Structural similarity of functions (`--similarity`).
//!
//! Every function and method is parsed and its syntax tree normalized: the
//! names of the functions and methods it calls stay, and so do its
//! operators, while local names, field names and literals become markers and
//! comments, punctuation and parentheses around one expression drop out.
//! Each subtree of the normalized tree is a fingerprint ([`prints`]), and
//! two units score the Jaccard index of their fingerprint sets: shared over
//! all. A renamed copy scores 1, a copy with an edited statement loses the
//! fingerprints of that statement and of the subtrees around it.
//!
//! The units of each language ([`syntax`], [`clojure`]):
//!
//! - Python: functions and methods; a function in a function is part of it,
//!   a method of a class declared in a function is a unit of its own;
//! - JavaScript and TypeScript: functions and methods with a body outside
//!   every function, and the variables and fields a function is assigned to;
//! - Java: methods with a body, not those of classes declared in a method;
//! - Kotlin, Scala, C#, C, C++, PHP, Ruby and Swift: functions and methods
//!   with a body outside every function, constructors left out;
//! - Go: functions and methods with a body;
//! - Rust: functions with a body, not those in a function or a `mod tests`;
//! - Clojure: every top-level form except `ns`;
//!
//! and the same in the code blocks of Markdown files and the scripts of Vue,
//! Svelte and Astro components. Units pair within one language, JavaScript
//! and TypeScript being one, and C and C++ another. Test files are left out
//! ([`reads`]).
//!
//! The search is exact ([`join`]): every pair whose score reaches the
//! threshold is found.

mod clojure;
mod join;
mod prints;
mod syntax;
pub mod test_files;

pub use join::jaccard;
/// Shared with `--semantic`, which names C and C++ functions the same way.
pub use syntax::declared_name;

use cpd_core::detect::{PathFilters, PreparedSource};
use cpd_core::models::{
    CloneKind, CpdClone, Fragment, Location, SimilarityMethod, StructuralMatch,
};
use cpd_tokenizer::line_index::LineIndex;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;
use std::path::Path;

/// The threshold `--similarity` compares at when it is given without one.
pub const DEFAULT_THRESHOLD: f64 = 0.8;

/// The fewest normalized nodes a unit has to have to be compared
/// (`--min-nodes`): smaller ones match too easily.
pub const DEFAULT_MIN_NODES: u32 = 20;

/// The jscpd formats whose files hold units.
const FORMATS: &[&str] = &[
    "javascript",
    "jsx",
    "typescript",
    "tsx",
    "python",
    "java",
    "kotlin",
    "scala",
    "csharp",
    "go",
    "rust",
    "c",
    "c-header",
    "cpp",
    "cpp-header",
    "php",
    "ruby",
    "swift",
    "clojure",
];

/// Whether files of `format` hold units.
pub fn supports(format: &str) -> bool {
    FORMATS.contains(&format)
}

/// The formats whose files hold units, for messages and docs.
pub fn supported_formats() -> &'static [&'static str] {
    FORMATS
}

/// Formats whose files embed code with units in it: Markdown with its code
/// blocks, and Vue, Svelte and Astro components with their scripts.
pub fn embeds(format: &str) -> bool {
    matches!(format, "markdown" | "md" | "vue" | "svelte" | "astro")
}

/// Whether `--similarity` reads the file at `path`, of `format`: a file of a
/// format with units or one that embeds them, not a test file. Of the
/// Python formats it reads `.py`, of the Clojure ones the source files, not
/// EDN data, and no TypeScript declaration file.
pub fn reads(path: &Path, format: &str) -> bool {
    if is_test_file(path) {
        return false;
    }
    if embeds(format) {
        return true;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match format {
        // A `.pyi` stub declares and does not implement; a script without an
        // extension is read.
        "python" => extension != "pyi",
        // `.edn` is data.
        "clojure" => extension != "edn",
        "typescript" => !name.ends_with(".d.ts"),
        _ => supports(format),
    }
}

/// A test file, by the conventions of [`test_files::is_test_path`], or a
/// pytest `conftest.py`.
fn is_test_file(path: &Path) -> bool {
    test_files::is_test_path(path)
        || path.file_name().and_then(|name| name.to_str()) == Some("conftest.py")
}

/// The language a unit of `format` is written in.
fn language(format: &str) -> &'static str {
    match format {
        "javascript" | "jsx" => "javascript",
        "typescript" | "tsx" => "typescript",
        "c" | "c-header" => "c",
        "cpp" | "cpp-header" => "cpp",
        "python" => "python",
        "java" => "java",
        "kotlin" => "kotlin",
        "scala" => "scala",
        "csharp" => "csharp",
        "go" => "go",
        "rust" => "rust",
        "php" => "php",
        "ruby" => "ruby",
        "swift" => "swift",
        "clojure" => "clojure",
        _ => "",
    }
}

/// The group a unit of `format` pairs within: JavaScript pairs with
/// TypeScript and C with C++, read by one grammar or nearly one.
fn group(format: &str) -> &'static str {
    match language(format) {
        "javascript" => "typescript",
        "c" => "cpp",
        other => other,
    }
}

/// A unit: a function, a method, a Clojure form.
#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    /// Its name, or a stand-in such as `<anonymous>`.
    pub name: String,
    pub start: Location,
    /// Where it ends: on its last line.
    pub end: Location,
    /// Lists and atoms in its normalized tree.
    pub nodes: u32,
    /// Its fingerprints, sorted.
    pub fingerprints: Vec<u64>,
    /// The detection tokens inside it, as an inclusive range into the
    /// tokens of its source, once [`FormSource::new`] knows them.
    pub range: [u32; 2],
    pub tokens: u32,
}

impl Form {
    /// The lines it spans, in jscpd's `end - start` convention that
    /// `--min-lines` reads.
    pub fn line_span(&self) -> u32 {
        self.end.line.saturating_sub(self.start.line)
    }
}

/// The units of `source`, a file of `format`, without the code in its
/// `jscpd:ignore` blocks and in the byte ranges `ignored` (the matches of
/// `--ignore-pattern`): a unit that lies in such code is none, and such code
/// adds nothing to the units around it.
pub fn forms(source: &str, format: &str, ignored: &[[usize; 2]]) -> Vec<Form> {
    if source.is_empty() || !supports(format) {
        return Vec::new();
    }
    let mut ranges = cpd_tokenizer::tokenizer::ignored_ranges(format, source);
    ranges.extend_from_slice(ignored);
    let ignored = cpd_tokenizer::tokenizer::merge_ranges(ranges);
    let index = LineIndex::new(source.as_bytes());
    let form = |name: String, start: usize, end: usize, nodes: u32, fingerprints: Vec<u64>| Form {
        name,
        start: index.location(start),
        end: end_location(&index, source, start, end),
        nodes,
        fingerprints,
        range: [0, 0],
        tokens: 0,
    };
    if format == "clojure" {
        return clojure::forms(source, &ignored)
            .into_iter()
            .map(|f| form(f.name, f.start_byte, f.end_byte, f.nodes, f.fingerprints))
            .collect();
    }
    let Some(grammar) = syntax::Grammar::for_format(format) else {
        return Vec::new();
    };
    syntax::forms(source, grammar, &ignored)
        .into_iter()
        .map(|f| form(f.name, f.start_byte, f.end_byte, f.nodes, f.fingerprints))
        .collect()
}

/// Where a unit spanning `start..end` ends: at its last byte when it ends
/// with a line break, so that it ends on the line of its code.
fn end_location(index: &LineIndex, source: &str, start: usize, end: usize) -> Location {
    match end > start && source.as_bytes().get(end - 1) == Some(&b'\n') {
        true => index.location(end - 1),
        false => index.location(end),
    }
}

/// The units of the code `source`, a file of `host_format`, embeds, each
/// with the format of its block: the code blocks of a Markdown file, the
/// scripts and frontmatter of a component. Positions are the host's, so a
/// unit reports where it sits in the `.md` or `.vue` file. `ignored` are
/// byte ranges of the host.
pub fn embedded_forms(
    source: &str,
    host_format: &str,
    ignored: &[[usize; 2]],
) -> Vec<(String, Form)> {
    let blocks = match host_format {
        "markdown" | "md" => cpd_tokenizer::markdown::code_blocks(source),
        "vue" | "svelte" | "astro" => cpd_tokenizer::sfc::script_blocks(source, host_format),
        _ => return Vec::new(),
    };
    let host = LineIndex::new(source.as_bytes());
    let mut out = Vec::new();
    for (format, range) in blocks {
        if !supports(&format) {
            continue;
        }
        let local = cpd_tokenizer::tokenizer::ranges_in(ignored, range.clone());
        let place = |location: &Location| host.location(range.start + location.offset as usize);
        for mut form in forms(&source[range.clone()], &format, &local) {
            form.start = place(&form.start);
            form.end = place(&form.end);
            out.push((format.clone(), form));
        }
    }
    out
}

/// The detection tokens lying inside the byte range `start..end` of a
/// source, as a half-open index range into its `spans`; `None` when there
/// are none.
pub fn token_range(
    spans: &[(Location, Location)],
    start: &Location,
    end: &Location,
) -> Option<(usize, usize)> {
    let first = spans.partition_point(|(s, _)| s.offset < start.offset);
    let last = spans.partition_point(|(_, e)| e.offset <= end.offset);
    (first < last).then_some((first, last))
}

/// The units of one source, as the search reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct FormSource {
    pub id: String,
    pub format: String,
    /// Canonical path of the file behind a symlink; empty when it is `id`.
    /// `--skip-isolated` reads it as it does for token clones.
    pub real_path: String,
    pub forms: Vec<Form>,
}

impl FormSource {
    /// The units `forms` of `prepared`, a source a scan prepared for
    /// detection, with the detection tokens each holds.
    pub fn new(prepared: &PreparedSource, forms: Vec<Form>) -> Self {
        Self::with_spans(
            prepared.id.clone(),
            prepared.format.clone(),
            prepared.real_path.clone(),
            forms,
            &prepared.spans,
        )
    }

    /// The units `forms` of a source whose detection tokens span `spans`.
    pub fn with_spans(
        id: String,
        format: String,
        real_path: String,
        mut forms: Vec<Form>,
        spans: &[(Location, Location)],
    ) -> Self {
        for form in &mut forms {
            if let Some((first, last)) = token_range(spans, &form.start, &form.end) {
                form.range = [first as u32, (last - 1) as u32];
                form.tokens = (last - first) as u32;
            }
        }
        Self {
            id,
            format,
            real_path,
            forms,
        }
    }
}

/// What a search finds: the pairs to report as clones and, when the search
/// is asked for them, every pair.
#[derive(Debug, Default)]
pub struct SimilarPairs {
    /// Every pair whose score reaches the threshold, most similar first, the
    /// ones the clones of the token passes cover too: the `edn` report lists
    /// them. Empty unless the search was asked to keep them.
    pub all: Vec<CpdClone>,
    /// The pairs that link look-alike units, in the order of their
    /// positions: clones of their own. A group of look-alikes is reported as
    /// the pairs that connect it, each unit with its closest match, as the
    /// token passes pair every copy of a fragment with the first one; units
    /// the clones of the token passes connect already are left out.
    pub reported: Vec<CpdClone>,
}

/// How many units of a group of look-alikes a unit is linked with when not
/// every pair is kept: enough for the group to stay connected when
/// `--skip-local` or `--skip-isolated` drops some of its pairs.
const LINKS: usize = 8;

/// A pair of indexed units, each its (source, unit), with the fingerprints
/// they share and the ones they have in all.
#[derive(Debug, Clone, Copy)]
struct Edge {
    a: (usize, usize),
    b: (usize, usize),
    shared: u32,
    total: u32,
}

/// The units of many sources, to pair among themselves or to search for a
/// source outside them, such as a snippet. Owns its sources so a long-lived
/// holder (the MCP server) can build it once per scan.
pub struct SimilarityIndex {
    sources: Vec<FormSource>,
    /// The units large enough to compare: (source, unit) by group.
    groups: FxHashMap<&'static str, Vec<(usize, usize)>>,
    min_nodes: u32,
    min_lines: u32,
}

impl SimilarityIndex {
    /// Index every unit of at least `min_nodes` nodes spanning at least
    /// `min_lines` lines.
    pub fn build(sources: Vec<FormSource>, min_nodes: u32, min_lines: u32) -> Self {
        let mut groups: FxHashMap<&'static str, Vec<(usize, usize)>> = FxHashMap::default();
        for (si, source) in sources.iter().enumerate() {
            let group = group(&source.format);
            for (fi, form) in source.forms.iter().enumerate() {
                if eligible(form, min_nodes, min_lines) {
                    groups.entry(group).or_default().push((si, fi));
                }
            }
        }
        Self {
            sources,
            groups,
            min_nodes,
            min_lines,
        }
    }

    /// The indexed sources, in the order they were given.
    pub fn sources(&self) -> &[FormSource] {
        &self.sources
    }

    /// The pairs of indexed units whose score reaches `threshold`: the ones
    /// to report, and every one of them when `keep_all` is set. The pairs
    /// that `filters` drops for token clones (`--skip-local`,
    /// `--skip-isolated`) are left out, and so is a pair of units one of
    /// which holds the other. The clones in `existing` connect the units
    /// they cover.
    pub fn pairs(
        &self,
        threshold: f64,
        existing: &[CpdClone],
        filters: &PathFilters,
        keep_all: bool,
    ) -> SimilarPairs {
        let mut groups: Vec<(&&str, &Vec<(usize, usize)>)> = self.groups.iter().collect();
        groups.sort_by_key(|(group, _)| **group);
        let mut edges = Vec::new();
        for (_, items) in groups {
            self.group_edges(items, threshold, filters, keep_all, &mut edges);
        }
        edges.sort_by(|x, y| self.edge_order(x, y));
        let reported = self.links(&edges, &Coverage::new(existing));
        let mut all: Vec<CpdClone> = match keep_all {
            true => edges.iter().map(|edge| self.clone_of(edge)).collect(),
            false => Vec::new(),
        };
        all.sort_by(report_order);
        SimilarPairs { all, reported }
    }

    /// The pairs of `items`, the units of one group, whose score reaches
    /// `threshold`. Units with the same fingerprints score 1.0 with each
    /// other, so the search runs on one of each and the pairs of the others
    /// follow from it: every pair when `keep_all` is set, else the links to
    /// a few of them.
    fn group_edges(
        &self,
        items: &[(usize, usize)],
        threshold: f64,
        filters: &PathFilters,
        keep_all: bool,
        edges: &mut Vec<Edge>,
    ) {
        let set = |i: usize| {
            let (si, fi) = items[i];
            self.sources[si].forms[fi].fingerprints.as_slice()
        };
        let mut first: FxHashMap<&[u64], usize> = FxHashMap::default();
        let mut copies: Vec<Vec<usize>> = Vec::new();
        for i in 0..items.len() {
            match first.entry(set(i)) {
                Entry::Occupied(entry) => copies[*entry.get()].push(i),
                Entry::Vacant(entry) => {
                    entry.insert(copies.len());
                    copies.push(vec![i]);
                }
            }
        }
        let linked = |n: usize| match keep_all {
            true => n,
            false => n.min(LINKS),
        };
        let mut add = |x: usize, y: usize, shared: u32, total: u32| {
            let (a, b) = (items[x], items[y]);
            if self.apart(a, b, filters) {
                edges.push(Edge {
                    a,
                    b,
                    shared,
                    total,
                });
            }
        };
        for same in &copies {
            let size = set(same[0]).len() as u32;
            for (i, &x) in same.iter().enumerate().skip(1) {
                for &y in &same[..linked(i)] {
                    add(y, x, size, size);
                }
            }
        }
        let distinct: Vec<&[u64]> = copies.iter().map(|same| set(same[0])).collect();
        for found in join::pairs(&distinct, threshold) {
            let (xs, ys) = (&copies[found.a], &copies[found.b]);
            for &x in xs {
                for &y in &ys[..linked(ys.len())] {
                    add(x, y, found.shared, found.total);
                }
            }
        }
    }

    /// Whether two units can pair: they are not the same span or one inside
    /// the other in one file, and `filters` keeps them.
    fn apart(
        &self,
        (sa, fa): (usize, usize),
        (sb, fb): (usize, usize),
        filters: &PathFilters,
    ) -> bool {
        let (src_a, src_b) = (&self.sources[sa], &self.sources[sb]);
        let (a, b) = (&src_a.forms[fa], &src_b.forms[fb]);
        !same_span(src_a, a, src_b, b)
            && !nested(src_a, a, src_b, b)
            && !filters
                .should_skip_sources((&src_a.id, &src_a.real_path), (&src_b.id, &src_b.real_path))
    }

    /// Most similar first, then by the places of the two units, so the links
    /// chosen do not depend on the order of the search.
    fn edge_order(&self, x: &Edge, y: &Edge) -> std::cmp::Ordering {
        let product = |e: &Edge, f: &Edge| u64::from(e.shared) * u64::from(f.total);
        product(y, x)
            .cmp(&product(x, y))
            .then_with(|| self.ends(x).cmp(&self.ends(y)))
    }

    /// The two units of a pair as their source and offset, the first first.
    fn ends(&self, edge: &Edge) -> ((&str, u32), (&str, u32)) {
        let end = |(si, fi): (usize, usize)| {
            let source = &self.sources[si];
            (source.id.as_str(), source.forms[fi].start.offset)
        };
        let (a, b) = (end(edge.a), end(edge.b));
        match a <= b {
            true => (a, b),
            false => (b, a),
        }
    }

    /// The pairs of `edges`, most similar first, that connect two units
    /// neither an earlier pair nor a clone in `coverage` connects: each unit
    /// with its closest match, and no pair a token clone already implies.
    fn links(&self, edges: &[Edge], coverage: &Coverage) -> Vec<CpdClone> {
        let mut base = Vec::with_capacity(self.sources.len());
        let mut units = 0;
        for source in &self.sources {
            base.push(units);
            units += source.forms.len();
        }
        let id = |(si, fi): (usize, usize)| base[si] + fi;
        let span = |(si, fi): (usize, usize)| {
            let source = &self.sources[si];
            let form = &source.forms[fi];
            (source.id.as_str(), form.start.line, form.end.line)
        };
        let mut parent: Vec<usize> = (0..units).collect();
        for edge in edges {
            if coverage.covers(span(edge.a), span(edge.b)) {
                join_sets(&mut parent, id(edge.a), id(edge.b));
            }
        }
        let mut reported: Vec<CpdClone> = edges
            .iter()
            .filter(|edge| join_sets(&mut parent, id(edge.a), id(edge.b)))
            .map(|edge| self.clone_of(edge))
            .collect();
        reported.sort_by(|x, y| x.position_key().cmp(&y.position_key()));
        reported
    }

    fn clone_of(&self, edge: &Edge) -> CpdClone {
        let (src_a, src_b) = (&self.sources[edge.a.0], &self.sources[edge.b.0]);
        let (a, b) = (&src_a.forms[edge.a.1], &src_b.forms[edge.b.1]);
        make_clone(src_a, a, src_b, b, edge.shared, edge.total)
    }

    /// The indexed units whose score with a unit of `source`, a source
    /// outside the index such as a snippet, reaches `threshold`, as clones,
    /// most similar first. A pair the clones in `existing` cover is left out.
    pub fn query_clones(
        &self,
        source: &FormSource,
        threshold: f64,
        existing: &[CpdClone],
    ) -> Vec<CpdClone> {
        let coverage = Coverage::new(existing);
        let mut clones = Vec::new();
        let Some(items) = self.groups.get(group(&source.format)) else {
            return clones;
        };
        for form in &source.forms {
            if !eligible(form, self.min_nodes, self.min_lines) {
                continue;
            }
            let size = form.fingerprints.len() as f64;
            for &(si, fi) in items {
                let other = &self.sources[si];
                let found = &other.forms[fi];
                let other_size = found.fingerprints.len() as f64;
                if other_size < threshold * size - 1e-9 || size < threshold * other_size - 1e-9 {
                    continue;
                }
                let (shared, total) = join::overlap(&form.fingerprints, &found.fingerprints);
                if (shared as f64 / total as f64) < threshold {
                    continue;
                }
                let clone = make_clone(source, form, other, found, shared, total);
                if !coverage.covers_clone(&clone) {
                    clones.push(clone);
                }
            }
        }
        clones.sort_by(report_order);
        clones
    }
}

fn eligible(form: &Form, min_nodes: u32, min_lines: u32) -> bool {
    form.nodes >= min_nodes && form.line_span() >= min_lines
}

/// Two units of one file over the same lines: Clojure forms on one line.
fn same_span(src_a: &FormSource, a: &Form, src_b: &FormSource, b: &Form) -> bool {
    src_a.id == src_b.id && a.start.line == b.start.line && a.end.line == b.end.line
}

/// Two units of one file one of which holds the other, such as a function
/// and a method of a class declared in it.
fn nested(src_a: &FormSource, a: &Form, src_b: &FormSource, b: &Form) -> bool {
    let holds =
        |x: &Form, y: &Form| x.start.offset <= y.start.offset && y.end.offset <= x.end.offset;
    src_a.id == src_b.id && (holds(a, b) || holds(b, a))
}

/// The set `x` belongs to in the union-find `parent`.
fn set_of(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Joins the sets of `x` and `y`; whether they were apart.
fn join_sets(parent: &mut [usize], x: usize, y: usize) -> bool {
    let (x, y) = (set_of(parent, x), set_of(parent, y));
    if x == y {
        return false;
    }
    parent[x.max(y)] = x.min(y);
    true
}

/// The pairs of units in `sources` whose score reaches `threshold`, of at
/// least `min_nodes` nodes and `min_lines` lines each (see
/// [`SimilarityIndex::pairs`]).
pub fn find_similar(
    sources: Vec<FormSource>,
    threshold: f64,
    min_nodes: u32,
    min_lines: u32,
    existing: &[CpdClone],
    filters: &PathFilters,
    keep_all: bool,
) -> SimilarPairs {
    if sources.is_empty() {
        return SimilarPairs::default();
    }
    SimilarityIndex::build(sources, min_nodes, min_lines)
        .pairs(threshold, existing, filters, keep_all)
}

/// Most similar first, then by language, the first file and line, and the
/// second file and line.
fn report_order(x: &CpdClone, y: &CpdClone) -> std::cmp::Ordering {
    let score = |c: &CpdClone| c.structure.as_ref().map_or(0.0, StructuralMatch::score);
    let language = |c: &CpdClone| c.structure.as_ref().map(|s| s.language.clone());
    score(y)
        .total_cmp(&score(x))
        .then_with(|| language(x).cmp(&language(y)))
        .then_with(|| x.fragment_a.source_id.cmp(&y.fragment_a.source_id))
        .then_with(|| x.fragment_a.start.line.cmp(&y.fragment_a.start.line))
        .then_with(|| x.fragment_b.source_id.cmp(&y.fragment_b.source_id))
        .then_with(|| x.fragment_b.start.line.cmp(&y.fragment_b.start.line))
}

fn make_clone(
    src_a: &FormSource,
    a: &Form,
    src_b: &FormSource,
    b: &Form,
    shared: u32,
    total: u32,
) -> CpdClone {
    let fragment = |src: &FormSource, f: &Form| Fragment {
        source_id: src.id.clone(),
        source_root: None,
        start: f.start.clone(),
        end: f.end.clone(),
        range: f.range,
        blame: None,
    };
    // The first fragment is the one of the first file, then the first one in
    // it.
    let a_first = (src_a.id.as_str(), a.start.offset) <= (src_b.id.as_str(), b.start.offset);
    let ((src_a, a), (src_b, b)) = match a_first {
        true => ((src_a, a), (src_b, b)),
        false => ((src_b, b), (src_a, a)),
    };
    // A pair can join JavaScript and TypeScript, or C and C++: it is
    // TypeScript or C++ then.
    let languages = [language(&src_a.format), language(&src_b.format)];
    let language = match languages {
        [a, b] if a == b => a,
        _ => group(&src_a.format),
    };
    CpdClone {
        format: src_a.format.clone(),
        fragment_a: fragment(src_a, a),
        fragment_b: fragment(src_b, b),
        token_count: a.tokens.min(b.tokens),
        is_new: false,
        kind: CloneKind::Similar,
        similarity: Some((shared as f64 / total as f64) as f32),
        similarity_method: Some(SimilarityMethod::Ast),
        structure: Some(StructuralMatch {
            language: language.to_string(),
            nodes: [a.nodes, b.nodes],
            shared,
            total,
        }),
        unmatched_lines: [0, 0],
    }
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

/// Leave out of the lines of each `--similarity` pair in `clones` the lines
/// that the clones of the token passes there already hold in its files, so
/// a line counts once. Statistics and the console read what remains (see
/// [`CpdClone::matched_lines`]). Run it on the clones a report shows, after
/// `--kind` drops any.
pub fn discount_token_lines(clones: &mut [CpdClone]) {
    let token_clone =
        |c: &CpdClone| c.similarity_method != Some(SimilarityMethod::Ast) && !c.kind.is_semantic();
    // The lines of each file that token clones hold, merged.
    let mut held: FxHashMap<String, Vec<(u32, u32)>> = FxHashMap::default();
    for clone in clones.iter().filter(|c| token_clone(c)) {
        for f in [&clone.fragment_a, &clone.fragment_b] {
            held.entry(f.source_id.clone())
                .or_default()
                .push((f.start.line, f.end.line));
        }
    }
    for ranges in held.values_mut() {
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for &(start, end) in ranges.iter() {
            match merged.last_mut() {
                Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        *ranges = merged;
    }
    let shared = |f: &Fragment| -> u32 {
        held.get(f.source_id.as_str()).map_or(0, |ranges| {
            ranges
                .iter()
                .map(|&(start, end)| {
                    let (lo, hi) = (start.max(f.start.line), end.min(f.end.line));
                    if lo <= hi { hi - lo + 1 } else { 0 }
                })
                .sum()
        })
    };
    for clone in clones
        .iter_mut()
        .filter(|c| c.similarity_method == Some(SimilarityMethod::Ast))
    {
        clone.unmatched_lines = [shared(&clone.fragment_a), shared(&clone.fragment_b)];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, format: &str, code: &str) -> FormSource {
        FormSource {
            id: id.to_string(),
            format: format.to_string(),
            real_path: String::new(),
            forms: forms(code, format, &[]),
        }
    }

    const ALPHA: &str = "def alpha(xs):\n    ys = filter(xs, 1)\n    total = 0\n    for y in ys:\n        total += y * 2\n    return map(total, inc)\n";
    const BETA: &str = "def beta(items):\n    kept = filter(items, 5)\n    sum = 0\n    for k in kept:\n        sum += k * 2\n    return map(sum, dec)\n";
    const GAMMA: &str = "def gamma(items):\n    kept = sorted(items, 5)\n    sum = 0\n    for k in kept:\n        sum -= k * 2\n    print(sum)\n    return map(sum, dec)\n";

    #[test]
    fn renamed_copies_pair_and_the_score_counts_shared_fingerprints() {
        let sources = vec![
            source("a.py", "python", ALPHA),
            source("b.py", "python", BETA),
            source("c.py", "python", GAMMA),
        ];
        let found = find_similar(
            sources,
            DEFAULT_THRESHOLD,
            20,
            3,
            &[],
            &PathFilters::default(),
            true,
        );
        assert_eq!(found.all.len(), 1);
        let pair = &found.all[0];
        assert_eq!(pair.similarity, Some(1.0));
        assert_eq!(pair.fragment_a.source_id, "a.py");
        let structure = pair.structure.as_ref().unwrap();
        assert_eq!(structure.language, "python");
        assert_eq!(structure.shared, structure.total);
        assert_eq!(found.reported.len(), 1);
    }

    #[test]
    fn small_units_and_units_of_other_languages_do_not_pair() {
        let ts = "function alpha(xs) {\n  const ys = filter(xs, 1);\n  let total = 0;\n  for (const y of ys) { total += y * 2; }\n  return map(total, inc);\n}\n";
        let sources = vec![
            source("a.py", "python", ALPHA),
            source("a.ts", "typescript", ts),
            source("b.js", "javascript", &ts.replace("alpha", "beta")),
        ];
        let found = find_similar(
            sources.clone(),
            DEFAULT_THRESHOLD,
            20,
            3,
            &[],
            &PathFilters::default(),
            true,
        );
        assert_eq!(found.all.len(), 1);
        assert_eq!(
            found.all[0].structure.as_ref().unwrap().language,
            "typescript"
        );
        let none = find_similar(
            sources,
            DEFAULT_THRESHOLD,
            1000,
            3,
            &[],
            &PathFilters::default(),
            true,
        );
        assert!(none.all.is_empty());
    }

    #[test]
    fn a_pair_a_token_clone_covers_is_found_but_not_reported() {
        let sources = vec![
            source("a.py", "python", ALPHA),
            source("b.py", "python", BETA),
        ];
        let token_clone = CpdClone {
            format: "python".into(),
            fragment_a: Fragment::new(
                "a.py",
                Location::new(1, 0, 0),
                Location::new(6, 0, 0),
                [0, 40],
            ),
            fragment_b: Fragment::new(
                "b.py",
                Location::new(1, 0, 0),
                Location::new(6, 0, 0),
                [0, 40],
            ),
            token_count: 40,
            is_new: false,
            kind: CloneKind::Renamed,
            similarity: None,
            similarity_method: None,
            structure: None,
            unmatched_lines: [0, 0],
        };
        let found = find_similar(
            sources,
            DEFAULT_THRESHOLD,
            20,
            3,
            &[token_clone],
            &PathFilters::default(),
            true,
        );
        assert_eq!(found.all.len(), 1);
        assert!(found.reported.is_empty());
    }

    #[test]
    fn a_snippet_finds_the_units_like_its_own() {
        let index = SimilarityIndex::build(
            vec![
                source("a.py", "python", ALPHA),
                source("c.py", "python", GAMMA),
            ],
            20,
            3,
        );
        let snippet = source("<snippet>", "python", BETA);
        let found = index.query_clones(&snippet, DEFAULT_THRESHOLD, &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].fragment_b.source_id, "a.py");
    }

    #[test]
    fn markdown_blocks_hold_units_at_their_place_in_the_host() {
        let md = format!("# Title\n\n```python\n{ALPHA}```\n");
        let found = embedded_forms(&md, "markdown", &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "python");
        assert_eq!(found[0].1.start.line, 4);
        assert_eq!(found[0].1.end.line, 9);
    }

    #[test]
    fn test_files_and_data_files_are_not_read() {
        assert!(reads(Path::new("src/app.py"), "python"));
        assert!(!reads(Path::new("tests/test_app.py"), "python"));
        assert!(!reads(Path::new("src/conftest.py"), "python"));
        assert!(!reads(Path::new("src/types.pyi"), "python"));
        assert!(!reads(Path::new("config.edn"), "clojure"));
        assert!(reads(Path::new("src/core.cljc"), "clojure"));
        assert!(!reads(Path::new("src/index.d.ts"), "typescript"));
        assert!(!reads(Path::new("src/app.test.ts"), "typescript"));
        assert!(reads(Path::new("docs/guide.md"), "markdown"));
        assert!(
            reads(Path::new("bin/deploy"), "python"),
            "a script without an extension"
        );
        assert!(reads(Path::new("src/core.cljx"), "clojure"));
    }

    #[test]
    fn a_unit_does_not_pair_with_one_nested_in_it() {
        let code = "def make_handler(config):\n    class Handler:\n        def handle(self, request):\n            check(request)\n            return send(config, request)\n    return Handler\n";
        let found = find_similar(
            vec![source("factory.py", "python", code)],
            0.01,
            1,
            0,
            &[],
            &PathFilters::default(),
            true,
        );
        assert!(found.all.is_empty(), "{:?}", found.all);
    }

    #[test]
    fn a_group_of_look_alikes_is_reported_as_the_pairs_that_link_it() {
        let sources = || -> Vec<FormSource> {
            (0..5)
                .map(|i| {
                    let code = format!(
                        "def f{i}(order):\n    total = price(order) * 2\n    return round(total, 2)\n"
                    );
                    source(&format!("{i}.py"), "python", &code)
                })
                .collect()
        };
        let found = find_similar(
            sources(),
            DEFAULT_THRESHOLD,
            1,
            0,
            &[],
            &PathFilters::default(),
            true,
        );
        assert_eq!(found.all.len(), 10, "every pair of the five copies");
        assert_eq!(found.reported.len(), 4, "each copy linked once");
        let found = find_similar(
            sources(),
            DEFAULT_THRESHOLD,
            1,
            0,
            &[],
            &PathFilters::default(),
            false,
        );
        assert!(found.all.is_empty());
        assert_eq!(found.reported.len(), 4);
    }
}
