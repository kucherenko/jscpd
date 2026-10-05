//! Python functions through the ruff parser.

use super::{FunctionExtractor, RawFunction};
use crate::line_index::LineIndex;
use cpd_core::similarity::{RoleName, name_hash};
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{AnyNodeRef, Expr, StmtFunctionDef};
use ruff_text_size::Ranged;

/// Python through the ruff parser: every `def` and `async def`, methods and
/// nested functions included. A function starts at `def` (or the `async`
/// before it), so its decorators stay out of both its span and its node
/// sequence. Type annotations, type parameters and docstrings are part of
/// the function and keep their place in the sequence.
pub struct PythonExtractor;

impl FunctionExtractor for PythonExtractor {
    fn grammar(&self) -> &'static str {
        "python"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["python"]
    }

    fn extract(&self, source: &str, _format: &str) -> Vec<RawFunction> {
        let Ok(parsed) = ruff_python_parser::parse_module(source) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.as_bytes());
        let mut functions = Functions {
            source,
            line_index: &line_index,
            frames: Vec::new(),
            out: Vec::new(),
        };
        functions.visit_body(&parsed.syntax().body);
        functions.out.sort_by_key(|f| f.start.offset);
        functions.out
    }
}

struct Frame {
    name: String,
    start: usize,
    end: usize,
    kinds: Vec<u16>,
    names: Vec<RoleName>,
}

struct Functions<'s> {
    source: &'s str,
    line_index: &'s LineIndex,
    frames: Vec<Frame>,
    out: Vec<RawFunction>,
}

impl Functions<'_> {
    /// Where the code of `f` starts: its `def`, or the `async` before it.
    /// The node's own range starts at its first decorator.
    fn def_start(&self, f: &StmtFunctionDef) -> usize {
        let name_start = f.name.range.start().to_usize();
        let head = &self.source[..name_start];
        let Some(def) = head.rfind("def") else {
            return f.range.start().to_usize();
        };
        let before = head[..def].trim_end();
        match f.is_async && before.ends_with("async") {
            true => before.len() - "async".len(),
            false => def,
        }
    }
}

impl<'a> SourceOrderVisitor<'a> for Functions<'_> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        let opened = match node {
            AnyNodeRef::StmtFunctionDef(f) => {
                self.frames.push(Frame {
                    name: f.name.to_string(),
                    start: self.def_start(f),
                    end: f.range.end().to_usize(),
                    kinds: Vec::new(),
                    names: Vec::new(),
                });
                true
            }
            _ => false,
        };
        let start = node.start().to_usize();
        let kind = node.kind() as u16;
        let called = match node {
            AnyNodeRef::ExprCall(call) => called_attribute(&call.func),
            _ => None,
        };
        let own = self.frames.len().saturating_sub(1);
        for (i, frame) in self.frames.iter_mut().enumerate() {
            // A decorator sits before `def`, outside the function. The
            // function's own node starts at its decorators too, and counts.
            if start < frame.start && !(opened && i == own) {
                continue;
            }
            frame.kinds.push(kind);
            if let Some(hash) = called {
                frame.names.push(RoleName {
                    after: (frame.kinds.len() - 1) as u32,
                    hash,
                });
            }
        }
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, node: AnyNodeRef<'a>) {
        if !matches!(node, AnyNodeRef::StmtFunctionDef(_)) {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let start = self.line_index.location(frame.start);
        self.out.push(RawFunction {
            grammar: "python",
            name: frame.name,
            head: start.clone(),
            start,
            end: self.line_index.location(frame.end),
            kinds: frame.kinds,
            names: frame.names,
        });
    }
}

/// The [`name_hash`] of the method a call invokes when its callee is an
/// attribute: `load` in `store.load(x)` and in `self.repo.load(x)`. A plain
/// call such as `load_user(x)` keeps no name, so a copy that calls a
/// renamed helper still matches.
fn called_attribute(func: &Expr) -> Option<u64> {
    match func {
        Expr::Attribute(attribute) => Some(name_hash(attribute.attr.as_str())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{extract_functions, supports_functions};

    const USERS: &str = "\
import functools


@functools.cache
def load_user(store: Store, user_id: int) -> User:
    \"\"\"Load one user.\"\"\"
    key = f\"user:{user_id}\"
    cached = store.cache.get(key)
    if cached:
        return cached
    user = store.load(user_id)
    store.cache.set(key, user)
    return user


class Users:
    async def first(self, ids: list[int]) -> User | None:
        def pick(found: list[User]) -> User | None:
            return found[0] if found else None

        return pick(await self.repo.fetch(ids))
";

    #[test]
    fn extracts_defs_methods_and_nested_functions_in_source_order() {
        assert!(supports_functions("python"));
        let fns = extract_functions(USERS, "python");
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["load_user", "first", "pick"]);
        assert!(fns.iter().all(|f| f.grammar == "python"));
        let load = &fns[0];
        assert!(
            USERS[load.start.offset as usize..].starts_with("def load_user"),
            "the span starts at def, after the decorator"
        );
        assert_eq!(load.head, load.start);
        assert_eq!((load.start.line, load.end.line), (5, 13));
        let first = &fns[1];
        assert!(USERS[first.start.offset as usize..].starts_with("async def first"));
        assert!(
            first.kinds.len() > fns[2].kinds.len(),
            "a nested function adds to the outer one"
        );
    }

    #[test]
    fn decorators_stay_out_of_the_node_sequence() {
        let plain = extract_functions("def f(a):\n    return a.run(1)\n", "python");
        let decorated = extract_functions(
            "@app.get(\"/users\")\ndef f(a):\n    return a.run(1)\n",
            "python",
        );
        assert_eq!(plain[0].kinds, decorated[0].kinds);
        assert_eq!(plain[0].names, decorated[0].names);
    }

    #[test]
    fn renamed_copies_share_kinds_and_called_methods_are_recorded() {
        let load = "def a(store, x):\n    return store.load(x)\n";
        let receiver = "def b(repo, y):\n    return repo.load(y)\n";
        let save = "def c(store, x):\n    return store.save(x)\n";
        let helper = "def d(store, x):\n    return load_item(x)\n";
        let [load, receiver, save] =
            [load, receiver, save].map(|src| extract_functions(src, "python").remove(0));
        assert_eq!(load.kinds, receiver.kinds);
        assert_eq!(load.kinds, save.kinds);
        assert_eq!(
            load.names, receiver.names,
            "the receiver's name does not count"
        );
        assert_eq!(load.names.len(), 1);
        assert_ne!(load.names[0].hash, save.names[0].hash);
        assert!(
            extract_functions(helper, "python")[0].names.is_empty(),
            "a plain call keeps no name"
        );
    }

    #[test]
    fn a_source_that_does_not_parse_yields_nothing() {
        assert!(extract_functions("def broken(:\n    pass\n", "python").is_empty());
    }
}
