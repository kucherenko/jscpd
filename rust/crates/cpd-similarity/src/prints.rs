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

use xxhash_rust::xxh3::xxh3_64_with_seed;

const KEYWORD_SEED: u64 = 0x6b65_7977_6f72_6473;
const STRING_SEED: u64 = 0x7374_7269_6e67_7321;
const LIST_SEED: u64 = 0x6c69_7374_7321_2121;

/// A node of a normalized tree, or nothing where the syntax drops out
/// (punctuation, keywords of the language, comments).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value {
    None,
    Atom(u64),
    /// A list: its fingerprint and the lists and atoms in it, itself
    /// included.
    List {
        hash: u64,
        nodes: u32,
    },
}

impl Value {
    fn nodes(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Atom(_) => 1,
            Self::List { nodes, .. } => nodes,
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
}

impl Prints {
    /// A list of `children`, the ones that are not [`Value::None`], in
    /// order: its fingerprint joins the set with every atom among them. The
    /// lists among them joined it when they were built.
    pub(crate) fn list(&mut self, children: &[Value]) -> Value {
        self.buffer.clear();
        let mut nodes = 1u32;
        for child in children {
            let hash = match *child {
                Value::None => continue,
                Value::Atom(hash) => {
                    self.all.push(hash);
                    hash
                }
                Value::List { hash, .. } => hash,
            };
            nodes = nodes.saturating_add(child.nodes());
            self.buffer.extend_from_slice(&hash.to_le_bytes());
        }
        let hash = xxh3_64_with_seed(&self.buffer, LIST_SEED);
        self.all.push(hash);
        Value::List { hash, nodes }
    }

    /// `[:symbol text]`, the list a name or an operator that stays becomes.
    pub(crate) fn symbol(&mut self, text: &str) -> Value {
        self.list(&[Value::Atom(keyword("symbol")), Value::Atom(string(text))])
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
                nodes: 3
            }
        );
        let outer = prints.list(&[Value::Atom(keyword("call")), call, Value::None]);
        let set = prints.finish(outer);
        // :symbol, "map", [:symbol "map"], :call, [:call [:symbol "map"]].
        assert_eq!(set.len(), 5);
        assert!(matches!(outer, Value::List { nodes: 5, .. }));
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
