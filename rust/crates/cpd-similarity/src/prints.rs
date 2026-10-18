//! Fingerprints of a normalized syntax tree.
//!
//! A normalized tree is made of lists and atoms. A list holds a keyword
//! naming its node first, then its children; an atom is a keyword
//! (`:symbol`, `:literal`, a node type) or a string (a name that stays, an
//! operator). Every list and every atom of a unit's tree is one fingerprint,
//! so a fingerprint stands for a whole subtree: the unit's own list, each
//! statement, each expression, each leaf. Two units score the Jaccard index
//! of their fingerprint sets, and the size of a unit is the number of lists
//! and atoms in its tree.
//!
//! A fingerprint is a 64-bit hash of the subtree's content, the same for
//! equal subtrees wherever they are. Keywords, strings and lists hash with
//! three seeds, so a keyword never equals a string of the same text.
//!
//! A name that does not stay is the atom `:symbol`, and the fingerprint of a
//! subtree also tells how the names in it repeat (issue #1217): each one
//! counts how many names back the same name occurred last in the subtree, 0
//! when it did not. This is Baker's parameterized matching with the subtree
//! as its window: `d = a + b * c` and `s = p + q * r` share their
//! fingerprints, `x = x + x * x` has others. A renamed copy keeps every
//! fingerprint, and an edit still changes only the subtrees around it.
//! Names of one text in different [`Space`]s are different names, so
//! `self.x = x` and `f(x=x)` still match `self.x = y` and `f(x=y)`.
//!
//! A list records a repeat once, where it first falls inside: when the two
//! places of a name are in different children of the list. Its children
//! hold the repeats inside them, so the list's fingerprint tells all the
//! repeats of its subtree, and each repeat is worked on at one list only.

use rustc_hash::FxHashMap;
use std::sync::LazyLock;
use xxhash_rust::xxh3::xxh3_64_with_seed;

const KEYWORD_SEED: u64 = 0x6b65_7977_6f72_6473;
const STRING_SEED: u64 = 0x7374_7269_6e67_7321;
const LIST_SEED: u64 = 0x6c69_7374_7321_2121;

/// The fingerprint of `:symbol`, the atom a name that does not stay becomes.
static SYMBOL: LazyLock<u64> = LazyLock::new(|| keyword("symbol"));

/// How `--similarity` compares the names that become `:symbol`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Names {
    /// A name has to stand for one name on the other side (issue #1217).
    #[default]
    Consistent,
    /// Any name matches any other, as in jscpd 5.4
    /// (`--ignore-identifiers=blind`).
    Blind,
}

/// Where a name lives. Two names of one text in two spaces are two names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Space {
    /// A local, a parameter, a function.
    Plain,
    /// A field or a method of what holds it: `x` in `self.x`, or declared
    /// in a class body.
    Member,
    /// A keyword argument or the key of an initializer: `x` in `f(x=y)` or
    /// `{x: y}`.
    Label,
    /// A type, a package or a module, a lifetime: `Storage` and `storage`
    /// in `storage storage.Storage`.
    Type,
    /// The unit's own name. No name stands for it: an accessor named after
    /// the field it reads is no other function than one that is not.
    Own,
}

/// A node of a normalized tree, or nothing where the syntax drops out
/// (punctuation, keywords of the language, comments).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value {
    None,
    Atom(u64),
    /// A name that does not stay: `:symbol`, the `n`-th name of its unit.
    Name(u32),
    /// A list: its fingerprint, the lists and atoms in it, itself included,
    /// the names in it, `[first, end)`, and those of them whose name
    /// occurred last before the list, `[start, end)` in [`Prints`]'s
    /// `open`.
    List {
        hash: u64,
        nodes: u32,
        names: [u32; 2],
        open: [u32; 2],
    },
}

impl Value {
    fn nodes(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Atom(_) | Self::Name(_) => 1,
            Self::List { nodes, .. } => nodes,
        }
    }

    /// The names in it, `[first, end)`, when it holds any. A subtree's names
    /// are numbered one after another.
    fn names(self) -> Option<[u32; 2]> {
        match self {
            Self::Name(n) => Some([n, n + 1]),
            Self::List { names, .. } if names[0] < names[1] => Some(names),
            _ => None,
        }
    }
}

/// The fingerprint of a keyword, such as `:symbol` or a node type.
pub(crate) fn keyword(name: &str) -> u64 {
    xxh3_64_with_seed(name.as_bytes(), KEYWORD_SEED)
}

/// The fingerprint of a string atom, a name or an operator that stays.
pub(crate) fn string(text: &str) -> u64 {
    xxh3_64_with_seed(text.as_bytes(), STRING_SEED)
}

/// The fingerprints of one unit's tree, collected as its lists are built.
#[derive(Debug, Default)]
pub(crate) struct Prints {
    all: Vec<u64>,
    buffer: Vec<u8>,
    names: Names,
    /// For every name of the unit, in order: how many names back the same
    /// name occurred last, 0 for its first occurrence.
    back: Vec<u32>,
    /// Where each name occurred last, by its space and the fingerprint of
    /// its text.
    last: FxHashMap<(Space, u64), u32>,
    /// The names of each list whose name occurred last before the list, one
    /// list after another (see [`Value::List`]).
    open: Vec<u32>,
}

impl Prints {
    /// The fingerprints of a unit whose names compare by `names`.
    pub(crate) fn new(names: Names) -> Self {
        Self {
            names,
            ..Self::default()
        }
    }

    pub(crate) fn names(&self) -> Names {
        self.names
    }

    /// `:symbol` for a name that does not stay, `text` in `space`.
    pub(crate) fn name(&mut self, space: Space, text: &str) -> Value {
        if self.names == Names::Blind || space == Space::Own {
            return Value::Atom(*SYMBOL);
        }
        let n = self.back.len() as u32;
        let last = self.last.insert((space, string(text)), n);
        self.back.push(last.map_or(0, |last| n - last));
        Value::Name(n)
    }

    /// A list of `children`, the ones that are not [`Value::None`], in
    /// order: its fingerprint joins the set with every atom among them. The
    /// lists among them joined it when they were built.
    ///
    /// The fingerprint also holds the repeats that fall inside this list
    /// first: the names whose name occurred last in the list, but not in
    /// their own child. Each is where it is in the list and how far back.
    pub(crate) fn list(&mut self, children: &[Value]) -> Value {
        self.buffer.clear();
        let mut nodes = 1u32;
        let mut names = [u32::MAX, 0];
        for child in children {
            let hash = match *child {
                Value::None => continue,
                Value::Atom(hash) => {
                    self.all.push(hash);
                    hash
                }
                Value::Name(_) => {
                    self.all.push(*SYMBOL);
                    *SYMBOL
                }
                Value::List { hash, .. } => hash,
            };
            if let Some([first, end]) = child.names() {
                names = [names[0].min(first), names[1].max(end)];
            }
            nodes = nodes.saturating_add(child.nodes());
            self.buffer.extend_from_slice(&hash.to_le_bytes());
        }
        let start = self.open.len();
        let mut repeats = 0u64;
        for child in children {
            match *child {
                Value::Name(n) => self.repeat_or_open(n, names[0], &mut repeats),
                Value::List { open, .. } => {
                    for i in open[0]..open[1] {
                        let n = self.open[i as usize];
                        self.repeat_or_open(n, names[0], &mut repeats);
                    }
                }
                _ => {}
            }
        }
        if repeats > 0 {
            self.buffer.extend_from_slice(&repeats.to_le_bytes());
        }
        let hash = xxh3_64_with_seed(&self.buffer, LIST_SEED);
        self.all.push(hash);
        let names = match names[0] < names[1] {
            true => names,
            false => [0, 0],
        };
        let open = [start as u32, self.open.len() as u32];
        Value::List {
            hash,
            nodes,
            names,
            open,
        }
    }

    /// Records the name `n`, open in a child of a list whose names start at
    /// `first`, as a repeat of the list when its name occurred last in it;
    /// it stays open for the lists above when its name occurred before.
    fn repeat_or_open(&mut self, n: u32, first: u32, repeats: &mut u64) {
        let back = self.back[n as usize];
        if back == 0 {
            return;
        }
        match n - back >= first {
            true => {
                self.buffer.extend_from_slice(&(n - first).to_le_bytes());
                self.buffer.extend_from_slice(&back.to_le_bytes());
                *repeats += 1;
            }
            false => self.open.push(n),
        }
    }

    /// `[:symbol text]`, the list a name or an operator that stays becomes.
    pub(crate) fn symbol(&mut self, text: &str) -> Value {
        self.list(&[Value::Atom(*SYMBOL), Value::Atom(string(text))])
    }

    /// The set of fingerprints, sorted, of the tree whose root is `root`.
    pub(crate) fn finish(mut self, root: Value) -> Vec<u64> {
        if let Value::Atom(hash) = root {
            self.all.push(hash);
        }
        self.all.sort_unstable();
        self.all.dedup();
        self.all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_subtrees_share_a_fingerprint_and_a_list_counts_its_atoms() {
        let mut prints = Prints::default();
        let call = prints.symbol("map");
        let again = prints.symbol("map");
        assert_eq!(call, again);
        // `[:symbol "map"]` is one list and two atoms.
        assert_eq!(
            call,
            Value::List {
                hash: call_hash(call),
                nodes: 3,
                names: [0, 0],
                open: [0, 0],
            }
        );
        let outer = prints.list(&[Value::Atom(keyword("call")), call, Value::None]);
        let set = prints.finish(outer);
        // :symbol, "map", [:symbol "map"], :call, [:call [:symbol "map"]].
        assert_eq!(set.len(), 5);
        assert!(matches!(outer, Value::List { nodes: 5, .. }));
    }

    #[test]
    fn a_list_tells_how_the_names_in_it_repeat() {
        // Two pairs of names in one list: the pairs, the list.
        let build = |names: [&str; 4], blind: bool| {
            let mut prints = Prints::new(match blind {
                true => Names::Blind,
                false => Names::Consistent,
            });
            let mut pairs = Vec::new();
            for two in names.chunks(2) {
                let (x, y) = (
                    prints.name(Space::Plain, two[0]),
                    prints.name(Space::Plain, two[1]),
                );
                pairs.push(prints.list(&[Value::Atom(keyword("pair")), x, y]));
            }
            let both = prints.list(&[Value::Atom(keyword("both")), pairs[0], pairs[1]]);
            [pairs[0], pairs[1], both].map(call_hash)
        };
        let same = build(["a", "b", "a", "b"], false);
        assert_eq!(same, build(["p", "q", "p", "q"], false));
        // Each pair names two names either way; the list sees them swap.
        let swapped = build(["a", "b", "b", "a"], false);
        assert_eq!(same[1], swapped[1]);
        assert_ne!(same[2], swapped[2]);
        assert_ne!(same[0], build(["a", "a", "a", "b"], false)[0]);
        // Blind, the names do not count.
        assert_eq!(
            build(["a", "b", "a", "b"], true),
            build(["a", "b", "b", "a"], true)
        );
    }

    #[test]
    fn a_keyword_and_a_string_of_one_text_differ() {
        assert_ne!(keyword("symbol"), string("symbol"));
    }

    fn call_hash(value: Value) -> u64 {
        match value {
            Value::List { hash, .. } => hash,
            _ => unreachable!(),
        }
    }
}
