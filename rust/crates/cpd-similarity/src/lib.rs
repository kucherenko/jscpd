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
//! A signature summarizes a unit, a function by default and also a class, a
//! variable or a type alias where the extractor finds them (issue #1132).
//! Units are only compared with units of their [`UnitKind`], and a pair
//! that lies inside a reported pair of classes is part of that pair, so a
//! copied class is one pair and not one per method.
//!
//! The scoring is grammar-agnostic: node-type ids are opaque `u16`s from
//! whichever extractor produced them ([`functions`]), and a
//! signature records its `grammar` so functions are only compared within
//! one grammar. Adding a language means adding an extractor, not touching
//! this module.

use cpd_core::detect::{PathFilters, PreparedSource};
use cpd_core::models::{CloneKind, CpdClone, Fragment, Location, SimilarityMethod};
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

    /// The values `--similarity-identifiers` takes.
    pub const NAMES: &'static [&'static str] = &["ignore", "role-aware"];

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
            other => Err(unknown(other, Self::NAMES)),
        }
    }
}

/// How literals take part in a function's structural summary
/// (`--similarity-literals`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarityLiterals {
    /// The parsed value with the category, in place of the category alone:
    /// two literals match when both are equal, so `timeout=10.0` and
    /// `timeout=30.0` differ. A pair never scores higher than by category.
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
    /// The values `--similarity-literals` takes.
    pub const NAMES: &'static [&'static str] = &["values", "categories", "generic", "omit"];

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
            other => Err(unknown(other, Self::NAMES)),
        }
    }
}

/// How the decorators of a unit take part in its structural summary
/// (`--similarity-decorators`): `@app.get("/users")` or `@dataclass`. Only
/// extractors that record [`DecoratorLeaf`]s have any, Python's so far. The
/// unit's span starts after its own decorators in every mode, and its size
/// limits read its code without them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarityDecorators {
    /// Left out of the unit they decorate and of the units around it: a
    /// decorator routes, caches or registers code, and the copy is in the
    /// code.
    #[default]
    Omit,
    /// Each decorator adds its name, as `get` for `@app.get("/users")`; its
    /// arguments do not count.
    Names,
    /// Each decorator adds its name and takes part whole, its arguments with
    /// it, their names and literals as the other options say.
    Full,
}

impl SimilarityDecorators {
    /// The values `--similarity-decorators` takes.
    pub const NAMES: &'static [&'static str] = &["omit", "names", "full"];

    /// The value as `--similarity-decorators` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Omit => "omit",
            Self::Names => "names",
            Self::Full => "full",
        }
    }
}

impl std::str::FromStr for SimilarityDecorators {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "omit" => Ok(Self::Omit),
            "names" => Ok(Self::Names),
            "full" => Ok(Self::Full),
            other => Err(unknown(other, Self::NAMES)),
        }
    }
}

/// Which units `--similarity` compares (`--similarity-candidates`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SimilarityCandidates {
    /// Every unit the extractors find.
    #[default]
    All,
    /// Definitions: the units at the top of a module or in a class body. A
    /// function or class declared in a function, a closure or a callback,
    /// is part of the code of that function and no candidate of its own. A
    /// class starts a scope of its own, so the methods of a class declared
    /// in a function are candidates.
    Definitions,
}

impl SimilarityCandidates {
    /// The values `--similarity-candidates` takes.
    pub const NAMES: &'static [&'static str] = &["all", "definitions"];

    /// The value as `--similarity-candidates` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Definitions => "definitions",
        }
    }
}

impl std::str::FromStr for SimilarityCandidates {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "definitions" => Ok(Self::Definitions),
            other => Err(unknown(other, Self::NAMES)),
        }
    }
}

/// Where a unit is declared, as its extractor finds it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitContext {
    /// Declared in a function: a nested function, a closure, a callback,
    /// or a class local to the function. A method is declared in its class,
    /// wherever the class is.
    pub local: bool,
    /// Test code: a test case, a suite or a hook of a test framework, a
    /// test class, or a unit inside one.
    pub test: bool,
}

/// The units `--similarity` compares: `--similarity-candidates` and
/// `--similarity-skip-tests`. A unit that is no candidate still counts in
/// the summary of the unit around it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CandidatePolicy {
    pub scope: SimilarityCandidates,
    /// Test code is no candidate.
    pub skip_tests: bool,
}

impl CandidatePolicy {
    /// Whether a unit declared in `context` is compared.
    pub fn admits(&self, context: UnitContext) -> bool {
        let local = self.scope == SimilarityCandidates::Definitions && context.local;
        !local && !(self.skip_tests && context.test)
    }
}

/// The error for a mode `value` that is none of `names`.
fn unknown(value: &str, names: &[&str]) -> String {
    format!(
        "unknown value '{value}', expected one of: {}",
        names.join(", ")
    )
}

/// What a function's summary keeps besides its node types: the names of
/// `--similarity-identifiers`, the literals of `--similarity-literals` and
/// the decorators of `--similarity-decorators`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignaturePolicy {
    pub identifiers: SimilarityIdentifiers,
    pub literals: SimilarityLiterals,
    pub decorators: SimilarityDecorators,
}

pub use cpd_core::models::UnitKind;

pub mod functions;

/// Whether a unit of the `outer` kind holds units of the `inner` kind: a
/// pair of them inside a pair of `outer` units is part of that pair. A class
/// holds its methods and fields, a variable the functions and classes of
/// its value, as in `export const handler = wrap(async (event) => ...)`,
/// and a function the classes defined in it; a function in a function is
/// compared on its own, as before units existed.
pub fn holds(outer: UnitKind, inner: UnitKind) -> bool {
    match outer {
        UnitKind::Class | UnitKind::Variable => true,
        UnitKind::Function => inner != UnitKind::Function,
        UnitKind::Type => false,
    }
}

/// The kind a unit is compared within: a unit pairs only with units of its
/// family. Variables and type aliases are one family, since an alias can be
/// written as a plain assignment.
fn family(unit: UnitKind) -> UnitKind {
    match unit {
        UnitKind::Type => UnitKind::Variable,
        other => other,
    }
}

/// The kind of a pair of units of `a` and `b`, which share a family: a
/// variable that pairs with a type alias is an alias written without
/// `TypeAlias`.
fn pair_unit(a: UnitKind, b: UnitKind) -> UnitKind {
    match a == b {
        true => a,
        false => UnitKind::Type,
    }
}

/// What an extractor records about one function: the pre-order node types
/// of its syntax tree, the names role-aware similarity can keep, its
/// literals and its decorators.
#[derive(Debug, Clone, Copy, Default)]
pub struct Structure<'a> {
    pub kinds: &'a [u16],
    pub names: &'a [RoleName],
    pub literals: &'a [LiteralLeaf],
    pub decorators: &'a [DecoratorLeaf],
}

/// A decorator in a unit's node-type sequence: the `len` nodes from index
/// `at`, its own node and its expression, and the [`decorator_hash`] of its
/// name, the name or attribute it calls: `get` in `@app.get("/users")`,
/// or the kind of its node when it calls none. The names and literals of
/// its arguments lie inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoratorLeaf {
    pub at: u32,
    pub len: u32,
    pub name: u64,
}

/// The sequence symbol of a decorator's name in the names and full modes of
/// `--similarity-decorators`: [`name_hash`] apart from the names of called
/// methods, so `@app.get` and a call of `store.get` stay two symbols.
pub fn decorator_hash(name: &str) -> u64 {
    name_hash(name) ^ DECORATOR_SALT
}

/// Keeps decorator names apart from method names; its top bit is clear, so
/// a decorator's symbol keeps the top bit of [`name_hash`].
const DECORATOR_SALT: u64 = 0x4465_636f_7261_746f;

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

/// The sequence symbol of a literal's value in the values mode: xxh3 of the
/// bytes of its parsed value, seeded by the literal's node type, with the
/// top bit set as for a name, so that no value equals a node type. The seed
/// keeps the string `"1"` and the number `1` apart.
pub fn literal_hash(kind: u16, value: &[u8]) -> u64 {
    let seed = LITERAL_SEED ^ u64::from(kind).wrapping_mul(FNV_PRIME);
    xxhash_rust::xxh3::xxh3_64_with_seed(value, seed) | (1 << 63)
}

/// Keeps the seeds of [`literal_hash`] apart from xxh3's default.
const LITERAL_SEED: u64 = 0x6c69_7465_7261_6c73;

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

/// Structural summary of one unit: a function, method or arrow function, or
/// a class, variable or type alias (see [`UnitKind`]).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    /// Grammar that produced the node-type sequence; pairs are only formed
    /// within one grammar.
    pub grammar: &'static str,
    /// What the signature summarizes; pairs are only formed within one kind.
    pub unit: UnitKind,
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
    /// method it invokes, literals take part as `policy.literals` says and
    /// decorators as `policy.decorators` does. Under the default policy the
    /// signature is the one [`Self::build`] makes from the node types with
    /// the decorators' nodes taken out.
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
            unit: UnitKind::Function,
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

    /// The signature of a unit of `unit` kind rather than a function.
    pub fn with_unit(mut self, unit: UnitKind) -> Self {
        self.unit = unit;
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
/// the node types alone: the policy keeps no name, changes no literal and
/// the function holds no decorator. That keeps the default signatures of
/// functions without decorators what they were before the policy existed.
fn summary(structure: Structure<'_>, policy: SignaturePolicy) -> Option<Vec<u64>> {
    let names = policy.identifiers.names(structure.names);
    let literals = match policy.literals {
        SimilarityLiterals::Categories => &[][..],
        _ => structure.literals,
    };
    let decorators = structure.decorators;
    if names.is_empty() && literals.is_empty() && decorators.is_empty() {
        return None;
    }
    Some(symbols(
        structure.kinds,
        names,
        literals,
        decorators,
        policy,
    ))
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

/// The node types of a function with its kept names, its literals and its
/// decorators as `policy` says: each name right after the node at its
/// `after` index, and each literal with its value in place of its own node,
/// replaced by one [`LITERAL_MARKER`], or left out. The categories mode
/// leaves the literals' nodes as they are. A decorator is left out with
/// everything in it, replaced by its name, or kept after its name.
///
/// The value takes the place of the literal's node, and the nodes of its
/// parts stay, so the values mode only splits symbols the categories mode
/// has: a pair never scores higher in it. The generic and omit modes make a
/// literal one symbol or none, whatever the number of its nodes, so a
/// Python string, which is its node and one node per part, takes less room
/// in them and a pair can score lower than by category.
fn symbols(
    kinds: &[u16],
    names: &[RoleName],
    literals: &[LiteralLeaf],
    decorators: &[DecoratorLeaf],
    policy: SignaturePolicy,
) -> Vec<u64> {
    let mode = policy.literals;
    let names = sorted_by(names, |n| n.after);
    let literals = sorted_by(literals, |l| l.at);
    let decorators = sorted_by(decorators, |d| d.at);
    let mut out = Vec::with_capacity(kinds.len() + names.len() + literals.len());
    let mut names = names.iter().peekable();
    let mut literals = literals.iter().peekable();
    let mut decorators = decorators.iter().peekable();
    let node = |kind: &u16| u64::from(*kind);
    let mut i = 0;
    while i < kinds.len() {
        if let Some(decorator) = decorators.next_if(|d| d.at as usize == i) {
            if policy.decorators != SimilarityDecorators::Omit {
                out.push(decorator.name);
            }
            if policy.decorators != SimilarityDecorators::Full {
                // The names and literals of its arguments go with it.
                let end = (i + decorator.len.max(1) as usize).min(kinds.len());
                while literals.next_if(|l| (l.at as usize) < end).is_some() {}
                while names.next_if(|n| (n.after as usize) < end).is_some() {}
                i = end;
                continue;
            }
        }
        // A decorator inside one already walked would overlap it.
        while decorators.next_if(|d| (d.at as usize) < i).is_some() {}
        // A literal inside one already walked, which no extractor records,
        // would overlap it.
        while literals.next_if(|l| (l.at as usize) < i).is_some() {}
        let next = match literals.next_if(|l| l.at as usize == i) {
            Some(literal) => {
                let end = (i + literal.len.max(1) as usize).min(kinds.len());
                match mode {
                    SimilarityLiterals::Values => {
                        out.push(literal.value);
                        out.extend(kinds[i + 1..end].iter().map(node));
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
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSource {
    pub id: String,
    pub format: String,
    /// Canonical path of the file behind a symlink; empty when it is `id`.
    /// `--skip-isolated` reads it as it does for token clones.
    pub real_path: String,
    pub functions: Vec<FunctionSig>,
}

impl FunctionSource {
    /// The signatures `functions` of the units of `prepared`, a source a
    /// scan prepared for detection.
    pub fn new(prepared: &PreparedSource, functions: Vec<FunctionSig>) -> Self {
        Self {
            id: prepared.id.clone(),
            format: prepared.format.clone(),
            real_path: prepared.real_path.clone(),
            functions,
        }
    }
}

/// The pairs a similarity search finds: the ones to report, and the ones
/// inside a pair of units that hold others (two classes, or two functions
/// that define classes) whose own pair scored. Those are part of that pair.
#[derive(Debug, Default)]
pub struct SimilarPairs {
    /// The pairs to report, in report order.
    pub pairs: Vec<CpdClone>,
    /// The pairs inside the pairs that scored, when the search keeps them: a
    /// baseline records them, so the pair of two methods stays known when
    /// the pair of their classes breaks apart.
    pub inner: Vec<CpdClone>,
}

/// LSH index over the functions of many sources. Owns its sources so a
/// long-lived holder (the MCP server) can build it once per scan and answer
/// any number of queries from it.
pub struct SimilarityIndex {
    sources: Vec<FunctionSource>,
    /// (source index, function index) per item.
    items: Vec<(usize, usize)>,
    /// Per item, the items of its source that hold it ([`holds`]): a
    /// method's class, the outer class of a nested one.
    owners: Vec<Vec<usize>>,
    /// The items by unit family and LSH band: units of different families
    /// never pair, so a class does not take the place of a function in a
    /// full bucket.
    buckets: FxHashMap<(UnitKind, u8, u64), Vec<usize>>,
    min_tokens: u32,
    min_lines: u32,
}

impl SimilarityIndex {
    /// Index every function with at least `min_tokens` tokens and a line
    /// span of at least `min_lines` (jscpd's usual clone thresholds).
    pub fn build(sources: Vec<FunctionSource>, min_tokens: usize, min_lines: usize) -> Self {
        let mut items = Vec::new();
        let mut buckets: FxHashMap<(UnitKind, u8, u64), Vec<usize>> = FxHashMap::default();
        for (si, src) in sources.iter().enumerate() {
            for (fi, f) in src.functions.iter().enumerate() {
                if !Self::eligible(f, min_tokens as u32, min_lines as u32) {
                    continue;
                }
                let item = items.len();
                items.push((si, fi));
                for (band, key) in band_keys(&f.minhash) {
                    buckets
                        .entry((family(f.unit), band, key))
                        .or_default()
                        .push(item);
                }
            }
        }
        let owners = owners_of(items.len(), |item| {
            let (si, fi) = items[item];
            (si, &sources[si].functions[fi])
        });
        Self {
            sources,
            items,
            owners,
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

    /// The source of `item`, and its signature.
    fn unit(&self, item: usize) -> (&FunctionSource, &FunctionSig) {
        let (si, fi) = self.items[item];
        let source = &self.sources[si];
        (source, &source.functions[fi])
    }

    /// Indexed functions structurally similar to `query`, best first.
    /// Returns `(source index, function index, similarity)`.
    pub fn query(&self, query: &FunctionSig, threshold: f32) -> Vec<(usize, usize, f32)> {
        let mut hits: Vec<(usize, usize, f32)> = self
            .query_items(query, threshold)
            .into_iter()
            .map(|(item, sim)| {
                let (si, fi) = self.items[item];
                (si, fi, sim)
            })
            .collect();
        hits.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
        hits
    }

    /// The items structurally similar to `query`, unordered.
    fn query_items(&self, query: &FunctionSig, threshold: f32) -> Vec<(usize, f32)> {
        if !Self::eligible(query, self.min_tokens, self.min_lines) {
            return Vec::new();
        }
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        let mut hits = Vec::new();
        for (band, key) in band_keys(&query.minhash) {
            let Some(bucket) = self.buckets.get(&(family(query.unit), band, key)) else {
                continue;
            };
            for &item in bucket.iter().take(MAX_BUCKET) {
                if !seen.insert(item) {
                    continue;
                }
                if let Some(sim) = score(query, self.unit(item).1, threshold) {
                    hits.push((item, sim));
                }
            }
        }
        hits
    }

    /// The indexed functions structurally similar to the functions of
    /// `source`, a source outside the index such as a snippet, as `similar`
    /// clones, most similar first. As in [`Self::all_pairs`], a pair inside
    /// a matched pair of classes is part of it, and a pair that the clones
    /// in `existing` already cover is left out.
    pub fn query_clones(
        &self,
        source: &FunctionSource,
        threshold: f32,
        existing: &[CpdClone],
    ) -> Vec<CpdClone> {
        let mut hits: Vec<(usize, usize, f32)> = Vec::new();
        for (q, query) in source.functions.iter().enumerate() {
            for (item, sim) in self.query_items(query, threshold) {
                hits.push((q, item, sim));
            }
        }
        let query_owners = owners_of(source.functions.len(), |q| (0, &source.functions[q]));
        let matched: FxHashSet<(usize, usize)> =
            hits.iter().map(|&(q, item, _)| (q, item)).collect();
        let coverage = Coverage::new(existing);
        hits.retain(|&(q, item, _)| {
            let (other, found) = self.unit(item);
            !inside_pair(&query_owners[q], &self.owners[item], |oq, oi| {
                matched.contains(&(oq, oi))
            }) && !coverage.covers(
                lines_of(source, &source.functions[q]),
                lines_of(other, found),
            )
        });
        let mut clones: Vec<CpdClone> = hits
            .into_iter()
            .map(|(q, item, sim)| {
                let (other, found) = self.unit(item);
                make_clone(source, &source.functions[q], other, found, sim)
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
    /// `filters` drops for token clones (`--skip-local`, `--skip-isolated`)
    /// and the pairs inside a pair of classes that scored.
    pub fn all_pairs(
        &self,
        threshold: f32,
        existing: &[CpdClone],
        filters: &PathFilters,
    ) -> Vec<CpdClone> {
        self.similar_pairs(threshold, existing, filters, false)
            .pairs
    }

    /// [`Self::all_pairs`], with the pairs inside the pairs that scored kept
    /// apart when `keep_inner` asks for them. Without it they are not even
    /// scored.
    pub fn similar_pairs(
        &self,
        threshold: f32,
        existing: &[CpdClone],
        filters: &PathFilters,
        keep_inner: bool,
    ) -> SimilarPairs {
        let mut candidates: FxHashSet<(usize, usize)> = FxHashSet::default();
        for bucket in self.buckets.values() {
            let members = &bucket[..bucket.len().min(MAX_BUCKET)];
            for (x, &a) in members.iter().enumerate() {
                for &b in &members[x + 1..] {
                    candidates.insert((a.min(b), a.max(b)));
                }
            }
        }
        // The pairs of two units that hold others go first, biggest first,
        // so a pair inside one that scored is known before it would be
        // scored: a unit has more tokens than the units it holds. A pair
        // that holds nothing can come in any order.
        let mut holds = vec![false; self.items.len()];
        for owner in self.owners.iter().flatten() {
            holds[*owner] = true;
        }
        let (mut holders, rest): (Vec<_>, Vec<_>) = candidates
            .into_iter()
            .partition(|&(a, b)| holds[a] && holds[b]);
        let tokens = |item: usize| u64::from(self.unit(item).1.token_count);
        holders.sort_unstable_by_key(|&(a, b)| (std::cmp::Reverse(tokens(a) + tokens(b)), a, b));
        let coverage = Coverage::new(existing);
        // The pairs that scored, covered or not: what they hold is part of
        // them either way.
        let mut scored: FxHashSet<(usize, usize)> = FxHashSet::default();
        let (mut found, mut inner) = (Vec::new(), Vec::new());
        for (a, b) in holders.into_iter().chain(rest) {
            let inside = inside_pair(&self.owners[a], &self.owners[b], |x, y| {
                scored.contains(&(x.min(y), x.max(y)))
            });
            if inside && !keep_inner {
                continue;
            }
            let ((src_a, fa), (src_b, fb)) = (self.unit(a), self.unit(b));
            if self.items[a].0 == self.items[b].0 && nested(fa, fb) {
                continue;
            }
            if filters
                .should_skip_sources((&src_a.id, &src_a.real_path), (&src_b.id, &src_b.real_path))
            {
                continue;
            }
            let Some(sim) = score(fa, fb, threshold) else {
                continue;
            };
            if inside {
                inner.push(make_clone(src_a, fa, src_b, fb, sim));
                continue;
            }
            if holds[a] && holds[b] {
                scored.insert((a, b));
            }
            if !coverage.covers(lines_of(src_a, fa), lines_of(src_b, fb)) {
                found.push(make_clone(src_a, fa, src_b, fb, sim));
            }
        }
        found.sort_by(|x, y| x.position_key().cmp(&y.position_key()));
        inner.sort_by(|x, y| x.position_key().cmp(&y.position_key()));
        SimilarPairs {
            pairs: found,
            inner,
        }
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
    find_similar_units(
        sources, threshold, min_tokens, min_lines, existing, filters, false,
    )
    .pairs
}

/// [`find_similar_functions`] over every unit, with the pairs inside the
/// pairs of classes that scored kept apart when `keep_inner` asks for them,
/// as a baseline needs.
pub fn find_similar_units(
    sources: Vec<FunctionSource>,
    threshold: f32,
    min_tokens: usize,
    min_lines: usize,
    existing: &[CpdClone],
    filters: &PathFilters,
    keep_inner: bool,
) -> SimilarPairs {
    if sources.is_empty() {
        return SimilarPairs::default();
    }
    // The order of the files decides which functions a bucket keeps past
    // its cap, so one order for every run: the walk is parallel.
    let mut sources = sources;
    sources.sort_unstable_by(|a, b| a.format.cmp(&b.format).then_with(|| a.id.cmp(&b.id)));
    SimilarityIndex::build(sources, min_tokens, min_lines)
        .similar_pairs(threshold, existing, filters, keep_inner)
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
    if a.grammar != b.grammar || family(a.unit) != family(b.unit) {
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

/// Whether a pair of units lies inside a pair that scored: `owners_a` and
/// `owners_b` hold each of them, and `paired` tells whether two holders
/// scored as a pair.
fn inside_pair(
    owners_a: &[usize],
    owners_b: &[usize],
    paired: impl Fn(usize, usize) -> bool,
) -> bool {
    owners_a
        .iter()
        .any(|&a| owners_b.iter().any(|&b| paired(a, b)))
}

/// For each of `count` units, `unit(i)` giving its source and signature,
/// the units of its source that hold it ([`holds`]): a method's class, a
/// field's class, the outer class of a nested one. Units nest, so one sweep
/// over each source in the order of their tokens finds them.
fn owners_of<'a>(
    count: usize,
    unit: impl Fn(usize) -> (usize, &'a FunctionSig),
) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&i| {
        let (source, sig) = unit(i);
        (source, sig.range[0], std::cmp::Reverse(sig.range[1]), i)
    });
    let mut owners = vec![Vec::new(); count];
    // The units the sweep is inside, outermost first.
    let mut open: Vec<usize> = Vec::new();
    let mut current = None;
    for i in order {
        let (source, inner) = unit(i);
        if current != Some(source) {
            open.clear();
            current = Some(source);
        }
        while open
            .last()
            .is_some_and(|&top| unit(top).1.range[1] < inner.range[0])
        {
            open.pop();
        }
        owners[i] = open
            .iter()
            .copied()
            .filter(|&o| {
                let outer = unit(o).1;
                contains(outer, inner) && holds(outer.unit, inner.unit)
            })
            .collect();
        open.push(i);
    }
    owners
}

/// Whether the tokens of `outer` hold all the tokens of `inner`.
fn contains(outer: &FunctionSig, inner: &FunctionSig) -> bool {
    outer.range[0] <= inner.range[0] && inner.range[1] <= outer.range[1]
}

fn nested(a: &FunctionSig, b: &FunctionSig) -> bool {
    contains(a, b) || contains(b, a)
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

/// Leave out of the lines of each `--similarity` pair in `clones` the lines
/// that the clones of the token passes there already hold in its files, so
/// a line counts once: a pair of classes that holds an exact copy of one of
/// its methods adds the lines of the other methods only. Statistics and the
/// console read what remains (see [`CpdClone::matched_lines`]). Run it on
/// the clones a report shows, after `--kind` drops any.
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
        unit: Some(pair_unit(a.unit, b.unit)),
        unmatched_lines: [0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default policy with the literals of `mode`.
    fn literal_policy(mode: SimilarityLiterals) -> SignaturePolicy {
        SignaturePolicy {
            literals: mode,
            ..SignaturePolicy::default()
        }
    }

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
            ..SignaturePolicy::default()
        }
    }

    #[test]
    fn a_decorator_is_left_out_named_or_kept_whole() {
        // A decorator over nodes 1 to 3, with a call to `get` and a literal
        // inside it.
        let kinds = [1, 2, 3, 4, 5, 6];
        let decorators = [DecoratorLeaf {
            at: 1,
            len: 3,
            name: decorator_hash("get"),
        }];
        let literals = [LiteralLeaf {
            at: 3,
            len: 1,
            value: literal_hash(4, b"/users"),
        }];
        let names = [RoleName::new(2, "get")];
        let with = |decorators_mode| {
            let policy = SignaturePolicy {
                identifiers: SimilarityIdentifiers::RoleAware,
                literals: SimilarityLiterals::Values,
                decorators: decorators_mode,
            };
            symbols(&kinds, &names, &literals, &decorators, policy)
        };
        assert_eq!(with(SimilarityDecorators::Omit), vec![1, 5, 6]);
        assert_eq!(
            with(SimilarityDecorators::Names),
            vec![1, decorator_hash("get"), 5, 6]
        );
        assert_eq!(
            with(SimilarityDecorators::Full),
            vec![
                1,
                decorator_hash("get"),
                2,
                3,
                name_hash("get"),
                literals[0].value,
                5,
                6
            ],
            "its name, then its call and literal as their own options say"
        );
        assert_ne!(decorator_hash("get"), name_hash("get"));
        assert!(decorator_hash("get") > u64::from(u16::MAX));
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
            ..Structure::default()
        };
        assert_eq!(
            plain.as_ref(),
            Some(&build_with(recorded, SignaturePolicy::default())),
            "names and literals change nothing by default"
        );
        assert_eq!(summary(recorded, SignaturePolicy::default()), None);
        // A decorator over nodes 2 and 3, with the name in it.
        let decorators = [DecoratorLeaf {
            at: 1,
            len: 2,
            name: decorator_hash("cache"),
        }];
        let decorated = Structure {
            decorators: &decorators,
            ..recorded
        };
        assert_eq!(
            FunctionSig::build(
                "test",
                "f".into(),
                loc(1, 0),
                loc(8, 75),
                &[1, 4, 5, 6, 7],
                &spans
            ),
            Some(build_with(decorated, SignaturePolicy::default())),
            "the default policy takes the decorator's nodes out"
        );
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
            symbols(
                &[10, 20, 30],
                &[b, a],
                &[],
                &[],
                literal_policy(SimilarityLiterals::Categories)
            ),
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
            error.contains("one of: values, categories, generic, omit"),
            "{error}"
        );
    }

    #[test]
    fn candidates_leave_out_local_units_and_tests_as_asked() {
        let context = |local, test| UnitContext { local, test };
        let all = CandidatePolicy::default();
        let definitions = CandidatePolicy {
            scope: SimilarityCandidates::Definitions,
            ..all
        };
        let no_tests = CandidatePolicy {
            skip_tests: true,
            ..all
        };
        for (local, test) in [(false, false), (true, false), (false, true), (true, true)] {
            assert!(all.admits(context(local, test)), "every unit by default");
            assert_eq!(definitions.admits(context(local, test)), !local);
            assert_eq!(no_tests.admits(context(local, test)), !test);
        }
    }

    #[test]
    fn every_mode_name_parses_to_the_mode_it_names() {
        fn check<M>(names: &[&str], as_str: fn(M) -> &'static str)
        where
            M: std::str::FromStr<Err = String> + std::fmt::Debug,
        {
            for &name in names {
                let mode = name.parse::<M>().unwrap();
                assert_eq!(as_str(mode), name);
            }
            assert!("unknown".parse::<M>().is_err());
        }
        check(SimilarityIdentifiers::NAMES, SimilarityIdentifiers::as_str);
        check(SimilarityLiterals::NAMES, SimilarityLiterals::as_str);
        check(SimilarityDecorators::NAMES, SimilarityDecorators::as_str);
        check(SimilarityCandidates::NAMES, SimilarityCandidates::as_str);
        assert_eq!(SimilarityDecorators::default(), SimilarityDecorators::Omit);
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
        let shape = |mode| symbols(&kinds, &[], &literals, &[], literal_policy(mode));
        assert_eq!(
            shape(SimilarityLiterals::Categories),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            shape(SimilarityLiterals::Values),
            vec![1, string.value, 3, 4, number.value, 6],
            "a value takes the place of its literal's own node"
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
            symbols(
                &kinds,
                &names,
                &literals,
                &[],
                literal_policy(SimilarityLiterals::Omit)
            ),
            vec![1, call.hash, 4, after_four.hash, 6]
        );
        assert_eq!(
            symbols(
                &kinds,
                &names,
                &literals,
                &[],
                literal_policy(SimilarityLiterals::Generic)
            ),
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
    fn a_pair_never_scores_higher_by_value_than_by_category() {
        // A small generator, so the cases are the same on every run.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        for _ in 0..500 {
            let len = 8 + next(40) as usize;
            let mut a: Vec<u16> = (0..len).map(|_| 1 + next(6) as u16).collect();
            let mut b = a.clone();
            // Edit a few nodes of the copy, as an edited function would.
            for _ in 0..next(4) {
                let at = next(len as u64) as usize;
                b[at] = 1 + next(6) as u16;
            }
            if next(2) == 0 {
                a.push(7);
            }
            let leaves = |kinds: &[u16], values: &mut dyn FnMut(u64) -> u64| -> Vec<LiteralLeaf> {
                (0..kinds.len())
                    .step_by(3)
                    .map(|at| LiteralLeaf {
                        at: at as u32,
                        len: 1,
                        value: literal_hash(kinds[at], &values(3).to_le_bytes()),
                    })
                    .collect()
            };
            let (la, lb) = (leaves(&a, &mut next), leaves(&b, &mut next));
            let score = |mode| {
                let sig = |kinds: &[u16], literals: &[LiteralLeaf]| {
                    let symbols = symbols(kinds, &[], literals, &[], literal_policy(mode));
                    shingles_from_symbols(&symbols, SHINGLE_K)
                };
                bag_jaccard(&sig(&a, &la), &sig(&b, &lb))
            };
            let (by_category, by_value) = (
                score(SimilarityLiterals::Categories),
                score(SimilarityLiterals::Values),
            );
            assert!(by_value <= by_category, "{by_value} > {by_category}");
        }
    }

    #[test]
    fn the_order_sources_come_in_does_not_change_the_pairs() {
        // More copies of one function than a bucket keeps, so which ones
        // are paired depends on the order they are indexed in.
        let body = kinds(3, 40);
        let sources: Vec<FunctionSource> = (0..300)
            .map(|i| FunctionSource {
                id: format!("f{i:03}.js"),
                format: "javascript".into(),
                real_path: String::new(),
                functions: vec![sig("copy", &body, 0, 30)],
            })
            .collect();
        let pairs = |sources: Vec<FunctionSource>| {
            find_similar_functions(sources, 0.9, 10, 3, &[], &PathFilters::default())
                .into_iter()
                .map(|c| (c.fragment_a.source_id, c.fragment_b.source_id))
                .collect::<Vec<_>>()
        };
        let forward = pairs(sources.clone());
        let mut reversed = sources;
        reversed.reverse();
        assert_eq!(forward, pairs(reversed));
        assert!(forward.len() < 300 * 299 / 2, "the bucket cap applies");
    }

    /// A source with a class over tokens 0 to 60 and a method inside it over
    /// tokens 20 to 50, as a class with one method has.
    fn class_with_method(id: &str) -> FunctionSource {
        FunctionSource {
            id: id.into(),
            format: "python".into(),
            real_path: String::new(),
            functions: vec![
                sig("Holder", &kinds(5, 60), 0, 60).with_unit(UnitKind::Class),
                sig("method", &kinds(9, 40), 20, 50),
            ],
        }
    }

    #[test]
    fn units_pair_only_within_their_family() {
        let body = kinds(4, 50);
        let unit = |id: &str, unit| FunctionSource {
            id: id.into(),
            format: "python".into(),
            real_path: String::new(),
            functions: vec![sig("same", &body, 0, 40).with_unit(unit)],
        };
        let pairs = |a, b| {
            find_similar_functions(
                vec![unit("a.py", a), unit("b.py", b)],
                0.9,
                10,
                3,
                &[],
                &PathFilters::default(),
            )
            .len()
        };
        assert_eq!(pairs(UnitKind::Class, UnitKind::Class), 1);
        assert_eq!(pairs(UnitKind::Variable, UnitKind::Variable), 1);
        assert_eq!(pairs(UnitKind::Class, UnitKind::Function), 0);
        assert_eq!(pairs(UnitKind::Class, UnitKind::Variable), 0);
        assert_eq!(
            pairs(UnitKind::Type, UnitKind::Variable),
            1,
            "an alias written as a plain assignment"
        );
        let alias = find_similar_functions(
            vec![
                unit("a.py", UnitKind::Variable),
                unit("b.py", UnitKind::Type),
            ],
            0.9,
            10,
            3,
            &[],
            &PathFilters::default(),
        );
        assert_eq!(alias[0].unit, Some(UnitKind::Type));
    }

    /// A source `id` whose `outer` unit, over tokens 0 to 60, holds a
    /// function over the tokens `inner`.
    fn holder(id: &str, outer: UnitKind, inner: (u32, u32)) -> FunctionSource {
        FunctionSource {
            id: id.into(),
            format: "python".into(),
            real_path: String::new(),
            functions: vec![
                sig("Outer", &kinds(5, 60), 0, 60).with_unit(outer),
                sig("inner", &kinds(9, 40), inner.0, inner.1),
            ],
        }
    }

    /// The source, first line and unit of each clone.
    fn shapes(clones: &[CpdClone]) -> Vec<(&str, &str, u32, UnitKind)> {
        clones
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.as_str(),
                    c.fragment_b.source_id.as_str(),
                    c.fragment_a.start.line,
                    c.unit.unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn a_pair_of_classes_that_a_token_clone_reports_still_holds_its_methods() {
        // The classes span lines 1 to 61 and their last methods lines 50 to
        // 60: an exact clone of lines 1 to 56 reports the classes, not the
        // methods.
        let sources = vec![
            holder("a.py", UnitKind::Class, (49, 59)),
            holder("b.py", UnitKind::Class, (49, 59)),
        ];
        let fragment = |id: &str| Fragment {
            source_id: id.into(),
            source_root: None,
            start: loc(1, 0),
            end: loc(56, 555),
            range: [0, 55],
            blame: None,
        };
        let exact = CpdClone::exact("python", fragment("a.py"), fragment("b.py"), 55);
        let found =
            find_similar_units(sources, 0.9, 10, 3, &[exact], &PathFilters::default(), true);
        assert!(found.pairs.is_empty(), "{:?}", shapes(&found.pairs));
        assert_eq!(
            shapes(&found.inner),
            vec![("a.py", "b.py", 50, UnitKind::Function)],
            "the methods are part of the pair of classes, kept apart for a baseline"
        );
    }

    #[test]
    fn a_class_in_a_function_is_part_of_the_function_pair_and_a_function_is_not() {
        let class_in_function = |id: &str| FunctionSource {
            id: id.into(),
            format: "python".into(),
            real_path: String::new(),
            functions: vec![
                sig("make_api", &kinds(5, 60), 0, 60),
                sig("Api", &kinds(9, 40), 10, 50).with_unit(UnitKind::Class),
            ],
        };
        let clones = find_similar_functions(
            vec![class_in_function("a.py"), class_in_function("b.py")],
            0.9,
            10,
            3,
            &[],
            &PathFilters::default(),
        );
        assert_eq!(
            shapes(&clones),
            vec![("a.py", "b.py", 1, UnitKind::Function)]
        );
        let clones = find_similar_functions(
            vec![
                holder("a.py", UnitKind::Function, (10, 50)),
                holder("b.py", UnitKind::Function, (10, 50)),
            ],
            0.9,
            10,
            3,
            &[],
            &PathFilters::default(),
        );
        assert_eq!(
            shapes(&clones),
            vec![
                ("a.py", "b.py", 1, UnitKind::Function),
                ("a.py", "b.py", 11, UnitKind::Function)
            ],
            "nested functions pair on their own, as before units"
        );
    }

    #[test]
    fn a_token_clone_and_a_pair_of_classes_count_their_shared_lines_once() {
        let fragment = |id: &str, start: u32, end: u32| Fragment {
            source_id: id.into(),
            source_root: None,
            start: loc(start, start * 10),
            end: loc(end, end * 10),
            range: [start, end],
            blame: None,
        };
        let exact = CpdClone::exact(
            "python",
            fragment("a.py", 1, 17),
            fragment("b.py", 1, 17),
            90,
        );
        let mut classes = exact.clone();
        classes.fragment_a = fragment("a.py", 1, 29);
        classes.fragment_b = fragment("b.py", 3, 31);
        classes.kind = CloneKind::Similar;
        classes.similarity_method = Some(SimilarityMethod::Ast);
        classes.unit = Some(UnitKind::Class);
        let mut clones = vec![exact, classes];
        discount_token_lines(&mut clones);
        assert_eq!(clones[1].unmatched_lines, [17, 15]);
        assert_eq!(
            clones[1].matched_lines(),
            12,
            "29 lines, 17 of them in the exact clone"
        );
        assert_eq!(
            clones[0].unmatched_lines,
            [0, 0],
            "token clones keep theirs"
        );
        let mut alone = vec![clones[1].clone()];
        discount_token_lines(&mut alone);
        assert_eq!(
            alone[0].matched_lines(),
            29,
            "without the exact clone, as --kind ast leaves it"
        );
    }

    #[test]
    fn a_pair_inside_a_reported_pair_of_classes_is_part_of_it() {
        let lone_method = FunctionSource {
            id: "c.py".into(),
            format: "python".into(),
            real_path: String::new(),
            functions: vec![sig("method", &kinds(9, 40), 20, 50)],
        };
        let sources = vec![
            class_with_method("a.py"),
            class_with_method("b.py"),
            lone_method,
        ];
        let clones = find_similar_functions(sources, 0.9, 10, 3, &[], &PathFilters::default());
        let pairs: Vec<(&str, &str, u32)> = clones
            .iter()
            .map(|c| {
                (
                    c.fragment_a.source_id.as_str(),
                    c.fragment_b.source_id.as_str(),
                    c.fragment_a.start.line,
                )
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("a.py", "b.py", 1),
                ("a.py", "c.py", 21),
                ("b.py", "c.py", 21)
            ],
            "the two classes pair, and their methods only pair with the method outside a class"
        );
    }

    #[test]
    fn classes_do_not_crowd_functions_out_of_a_full_bucket() {
        let body = kinds(4, 50);
        let source = |id: String, unit| FunctionSource {
            id,
            format: "python".into(),
            real_path: String::new(),
            functions: vec![sig("same", &body, 0, 40).with_unit(unit)],
        };
        let mut sources: Vec<FunctionSource> = (0..MAX_BUCKET + 50)
            .map(|i| source(format!("class{i:03}.py"), UnitKind::Class))
            .collect();
        // Sources are indexed by id, so these two come after every class.
        sources.push(source("zz1.py".into(), UnitKind::Function));
        sources.push(source("zz2.py".into(), UnitKind::Function));
        let clones = find_similar_functions(sources, 0.9, 10, 3, &[], &PathFilters::default());
        assert!(
            clones
                .iter()
                .any(|c| c.unit == Some(UnitKind::Function) && c.fragment_a.source_id == "zz1.py"),
            "the two functions share no bucket with the classes"
        );
    }

    #[test]
    fn a_snippet_class_that_matches_takes_its_methods_with_it() {
        let index = SimilarityIndex::build(vec![class_with_method("a.py")], 10, 3);
        let snippet = class_with_method("snippet");
        let clones = index.query_clones(&snippet, 0.9, &[]);
        assert_eq!(clones.len(), 1, "{clones:?}");
        assert_eq!(
            clones[0].fragment_a.start.line, 1,
            "the class, not its method"
        );
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
