//! Python functions through the ruff parser.

use super::{FunctionExtractor, MAX_OPEN_FUNCTIONS, RawFunction};
use crate::line_index::LineIndex;
use cpd_core::similarity::{CodeSize, LiteralLeaf, RoleName, literal_hash, name_hash};
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{AnyNodeRef, Expr, Number, Stmt, StmtFunctionDef};
use ruff_text_size::{Ranged, TextRange};
use std::collections::HashSet;

/// How deep the walk goes into the syntax tree; deeper nodes are left out
/// of every sequence. ruff's visitor recurses once per level, and generated
/// code such as a sum of a hundred thousand terms would otherwise overflow
/// the thread's stack. Hand-written code stays far below it.
const MAX_DEPTH: usize = 1000;

/// The stack for dropping a tree deeper than [`MAX_DEPTH`]: a base, plus a
/// generous allowance per byte of source for a debug build's drop frames.
/// It is reserved address space; only the pages the drop touches are used.
const DROP_STACK_BASE: usize = 1 << 20;
const DROP_STACK_PER_BYTE: usize = 256;

/// Python through the ruff parser: every `def` and `async def` with code in
/// its body, methods and nested functions included. A function starts at
/// `def` (or the `async` before it), so its decorators stay out of both its
/// span and its node sequence. Type annotations, type parameters and
/// docstrings keep their place in the sequence, while the size limits read
/// the code alone: a docstring and comments add no tokens or lines there.
/// A declaration whose body is `...` or a docstring, such as an `@overload`
/// signature, a `.pyi` stub or a `Protocol` member, is no function to
/// compare, as with bodyless TypeScript functions.
///
/// Literals are strings with their parts (`"a" "b"` is one), bytes, numbers,
/// booleans, `None`, `...`, and the text of an f-string or t-string between
/// its replacement fields, which stand as nodes of their own. A docstring is
/// documentation and keeps its node types whatever `--similarity-literals`
/// says.
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
        let code_tokens: Vec<usize> = parsed
            .tokens()
            .iter()
            .filter(|token| is_code(token.kind()))
            .map(|token| token.start().to_usize())
            .collect();
        let mut functions = Functions {
            source,
            line_index: &line_index,
            code_tokens: &code_tokens,
            frames: Vec::new(),
            defs: Vec::new(),
            depth: 0,
            too_deep: false,
            strings: 0,
            docstrings: HashSet::new(),
            out: Vec::new(),
        };
        functions.visit_body(&parsed.syntax().body);
        let (mut out, too_deep) = (functions.out, functions.too_deep);
        if too_deep {
            // Dropping a tree takes a call per level as well. ruff's parser
            // grows its stack for generated code nested this deep, so the
            // drop gets a stack of its own: a level per byte of source at
            // most, as in `- - - x`.
            stacker::grow(
                DROP_STACK_BASE + source.len() * DROP_STACK_PER_BYTE,
                move || drop(parsed),
            );
        }
        out.sort_by_key(|f| f.start.offset);
        out
    }
}

/// Whether a token is code: comments, line breaks and indentation are not.
fn is_code(kind: TokenKind) -> bool {
    !kind.is_trivia()
        && !matches!(
            kind,
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::EndOfFile
        )
}

/// The docstring of `f`: a string literal standing as the first statement
/// of its body.
fn docstring(f: &StmtFunctionDef) -> Option<TextRange> {
    body_docstring(&f.body)
}

/// The docstring of a `def` or `class` body: its first statement when that
/// is a string literal.
fn body_docstring(body: &[Stmt]) -> Option<TextRange> {
    match body.first()? {
        Stmt::Expr(stmt) if matches!(stmt.value.as_ref(), Expr::StringLiteral(_)) => {
            Some(stmt.range())
        }
        _ => None,
    }
}

/// The parsed value of a number: an int by its digits in base 10, a float
/// by its bits, a complex number by both of its parts. So `0x10` equals
/// `16` and `1.0` equals `1.00`, while the int `1` and the float `1.0`
/// differ.
fn number_value(number: &Number) -> Vec<u8> {
    match number {
        Number::Int(int) => [b"i".as_slice(), int.to_string().as_bytes()].concat(),
        Number::Float(float) => [b"f".as_slice(), &float.to_bits().to_le_bytes()].concat(),
        Number::Complex { real, imag } => [
            b"c".as_slice(),
            &real.to_bits().to_le_bytes(),
            &imag.to_bits().to_le_bytes(),
        ]
        .concat(),
    }
}

/// Whether `f` only declares a function: its body holds nothing but `...`
/// and a docstring.
fn is_stub(f: &StmtFunctionDef) -> bool {
    let skip = usize::from(docstring(f).is_some());
    f.body.iter().skip(skip).all(|stmt| {
        matches!(stmt, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::EllipsisLiteral(_)))
    })
}

struct Frame {
    name: String,
    start: usize,
    end: usize,
    docstring: Option<TextRange>,
    kinds: Vec<u16>,
    names: Vec<RoleName>,
    literals: Vec<LiteralLeaf>,
    /// The string or bytes literal being walked, by its index in
    /// `literals`: its length is known when the walk leaves its parts.
    open_string: Option<usize>,
}

struct Functions<'s> {
    source: &'s str,
    line_index: &'s LineIndex,
    /// Where the file's code tokens start, in order.
    code_tokens: &'s [usize],
    frames: Vec<Frame>,
    /// For every `def` being walked, whether it opened a frame: a stub and
    /// a `def` nested past [`MAX_OPEN_FUNCTIONS`] do not.
    defs: Vec<bool>,
    depth: usize,
    /// Whether the walk met nodes deeper than [`MAX_DEPTH`].
    too_deep: bool,
    /// How many string or bytes expressions the walk is inside: their parts
    /// belong to their literal and are no literals of their own.
    strings: usize,
    /// Where the docstrings of the `def`s and `class`es walked so far start.
    docstrings: HashSet<usize>,
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

    /// Code tokens starting inside `start..end`.
    fn tokens_in(&self, start: usize, end: usize) -> u32 {
        let first = self.code_tokens.partition_point(|&t| t < start);
        let last = self.code_tokens.partition_point(|&t| t < end);
        (last - first) as u32
    }

    /// The size of a function's code: its tokens and line span without its
    /// docstring and comments.
    fn code_size(&self, frame: &Frame) -> CodeSize {
        let start_line = self.line_index.location(frame.start).line;
        let end_line = self.line_index.location(frame.end).line;
        let mut tokens = self.tokens_in(frame.start, frame.end);
        let mut lines = end_line.saturating_sub(start_line);
        if let Some(doc) = frame.docstring {
            let (doc_start, doc_end) = (doc.start().to_usize(), doc.end().to_usize());
            tokens = tokens.saturating_sub(self.tokens_in(doc_start, doc_end));
            let first = self.line_index.location(doc_start).line;
            if first > start_line {
                let last = self.line_index.location(doc_end).line;
                lines = lines.saturating_sub(last - first + 1);
            }
        }
        CodeSize { tokens, lines }
    }

    /// The [`literal_hash`] of `node` when it is a literal of type `kind`,
    /// and whether it is a string or bytes expression, whose parts follow.
    fn literal(&mut self, node: AnyNodeRef<'_>, kind: u16) -> Option<(u64, bool)> {
        let hash = |value: &[u8]| literal_hash(kind, value);
        match node {
            AnyNodeRef::ExprStringLiteral(string) => {
                self.strings += 1;
                let doc = self.docstrings.contains(&string.start().to_usize());
                (!doc).then(|| (hash(string.value.to_str().as_bytes()), true))
            }
            AnyNodeRef::ExprBytesLiteral(bytes) => {
                self.strings += 1;
                Some((hash(&bytes.value.bytes().collect::<Vec<u8>>()), true))
            }
            // A part of a string or bytes expression belongs to it; a plain
            // part of an f-string or t-string stands alone.
            AnyNodeRef::StringLiteral(part) if self.strings == 0 => {
                Some((hash(part.value.as_bytes()), false))
            }
            AnyNodeRef::InterpolatedStringLiteralElement(text) => {
                Some((hash(text.value.as_bytes()), false))
            }
            AnyNodeRef::ExprNumberLiteral(number) => {
                Some((hash(&number_value(&number.value)), false))
            }
            AnyNodeRef::ExprBooleanLiteral(boolean) => {
                Some((hash(&[u8::from(boolean.value)]), false))
            }
            AnyNodeRef::ExprNoneLiteral(_) | AnyNodeRef::ExprEllipsisLiteral(_) => {
                Some((hash(&[]), false))
            }
            _ => None,
        }
    }
}

impl<'a> SourceOrderVisitor<'a> for Functions<'_> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.too_deep = true;
            return TraversalSignal::Skip;
        }
        let opened = match node {
            AnyNodeRef::StmtFunctionDef(f) => {
                let doc = docstring(f);
                if let Some(doc) = doc {
                    self.docstrings.insert(doc.start().to_usize());
                }
                let opens = self.frames.len() < MAX_OPEN_FUNCTIONS && !is_stub(f);
                if opens {
                    self.frames.push(Frame {
                        name: f.name.to_string(),
                        start: self.def_start(f),
                        end: f.range.end().to_usize(),
                        docstring: doc,
                        kinds: Vec::new(),
                        names: Vec::new(),
                        literals: Vec::new(),
                        open_string: None,
                    });
                }
                self.defs.push(opens);
                opens
            }
            AnyNodeRef::StmtClassDef(class) => {
                if let Some(doc) = body_docstring(&class.body) {
                    self.docstrings.insert(doc.start().to_usize());
                }
                false
            }
            _ => false,
        };
        let start = node.start().to_usize();
        let kind = node.kind() as u16;
        let called = match node {
            AnyNodeRef::ExprCall(call) => called_attribute(&call.func),
            _ => None,
        };
        let literal = self.literal(node, kind);
        let own = self.frames.len().saturating_sub(1);
        for (i, frame) in self.frames.iter_mut().enumerate() {
            // A decorator sits before `def`, outside the function. The
            // function's own node starts at its decorators too, and counts.
            if start < frame.start && !(opened && i == own) {
                continue;
            }
            frame.kinds.push(kind);
            let at = (frame.kinds.len() - 1) as u32;
            if let Some(hash) = called {
                frame.names.push(RoleName { after: at, hash });
            }
            if let Some((value, string)) = literal {
                if string {
                    frame.open_string = Some(frame.literals.len());
                }
                frame.literals.push(LiteralLeaf { at, len: 1, value });
            }
        }
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, node: AnyNodeRef<'a>) {
        let skipped = self.depth > MAX_DEPTH;
        self.depth -= 1;
        if skipped {
            return;
        }
        if matches!(
            node,
            AnyNodeRef::ExprStringLiteral(_) | AnyNodeRef::ExprBytesLiteral(_)
        ) {
            self.strings -= 1;
            // The literal is its own node and the parts walked under it.
            for frame in &mut self.frames {
                if let Some(index) = frame.open_string.take() {
                    let literal = &mut frame.literals[index];
                    literal.len = frame.kinds.len() as u32 - literal.at;
                }
            }
            return;
        }
        if !matches!(node, AnyNodeRef::StmtFunctionDef(_)) {
            return;
        }
        if !self.defs.pop().unwrap_or(false) {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let code_size = self.code_size(&frame);
        let start = self.line_index.location(frame.start);
        self.out.push(RawFunction {
            grammar: "python",
            name: frame.name,
            head: start.clone(),
            start,
            end: self.line_index.location(frame.end),
            kinds: frame.kinds,
            names: frame.names,
            literals: frame.literals,
            code_size: Some(code_size),
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

    /// The literals of the first function of `src`: the node types each
    /// spans, and its value.
    fn literals_of(src: &str) -> Vec<(Vec<ruff_python_ast::NodeKind>, u64)> {
        use ruff_python_ast::NodeKind;
        let f = extract_functions(src, "python").remove(0);
        let kind = |id: u16| {
            [
                NodeKind::ExprStringLiteral,
                NodeKind::StringLiteral,
                NodeKind::ExprBytesLiteral,
                NodeKind::BytesLiteral,
                NodeKind::ExprNumberLiteral,
                NodeKind::ExprBooleanLiteral,
                NodeKind::ExprNoneLiteral,
                NodeKind::ExprEllipsisLiteral,
                NodeKind::InterpolatedStringLiteralElement,
            ]
            .into_iter()
            .find(|k| *k as u16 == id)
            .unwrap_or_else(|| panic!("node type {id} is no literal's"))
        };
        f.literals
            .iter()
            .map(|l| {
                let nodes = &f.kinds[l.at as usize..(l.at + l.len) as usize];
                (nodes.iter().map(|&id| kind(id)).collect(), l.value)
            })
            .collect()
    }

    #[test]
    fn literals_are_recorded_with_their_parts_and_parsed_values() {
        use ruff_python_ast::NodeKind::*;
        let src = "\
def f(state: Literal[\"active\"], key) -> bytes:
    \"\"\"Return the payload for a state.\"\"\"
    a = \"x\" \"y\"
    b = 0x10, 16, 1.0, 1.00, 1, 2j
    c = True, None, ...
    d = f\"pre{key:>10}post\" \"tail\"
    return b\"raw\" b\"more\"
";
        let found = literals_of(src);
        let nodes: Vec<&[_]> = found.iter().map(|(n, _)| n.as_slice()).collect();
        assert_eq!(
            nodes,
            vec![
                &[ExprStringLiteral, StringLiteral][..],
                &[ExprStringLiteral, StringLiteral, StringLiteral],
                &[ExprNumberLiteral],
                &[ExprNumberLiteral],
                &[ExprNumberLiteral],
                &[ExprNumberLiteral],
                &[ExprNumberLiteral],
                &[ExprNumberLiteral],
                &[ExprBooleanLiteral],
                &[ExprNoneLiteral],
                &[ExprEllipsisLiteral],
                &[InterpolatedStringLiteralElement],
                &[InterpolatedStringLiteralElement],
                &[InterpolatedStringLiteralElement],
                &[StringLiteral],
                &[ExprBytesLiteral, BytesLiteral, BytesLiteral],
            ],
            "Literal[\"active\"], the joined string, six numbers, three constants, \
             the f-string's text, format spec and tail, the joined bytes; no docstring"
        );
        let value = |i: usize| found[i].1;
        assert_eq!(value(2), value(3), "0x10 is 16");
        assert_eq!(value(4), value(5), "1.0 is 1.00");
        assert_ne!(value(5), value(6), "the float 1.0 is not the int 1");
        let renamed = literals_of(&src.replace("\"active\"", "\"blocked\""));
        assert_ne!(
            renamed[0].1,
            value(0),
            "the value inside an annotation counts"
        );
        assert_eq!(renamed[1..], found[1..]);
    }

    #[test]
    fn a_docstring_is_documentation_and_no_literal() {
        let src = "\
def outer(x):
    \"\"\"Outer docstring.\"\"\"

    class Inner:
        \"\"\"Class docstring.\"\"\"

    def inner(y):
        \"\"\"Inner docstring.\"\"\"
        return y + 1

    return Inner, inner(x), \"text\"
";
        let fns = extract_functions(src, "python");
        let outer = fns.iter().find(|f| f.name == "outer").unwrap();
        assert_eq!(
            outer.literals.len(),
            2,
            "the 1 of inner and the text; no docstring of outer, Inner or inner"
        );
        let inner = fns.iter().find(|f| f.name == "inner").unwrap();
        assert_eq!(inner.literals.len(), 1);
    }

    #[test]
    fn a_source_that_does_not_parse_yields_nothing() {
        assert!(extract_functions("def broken(:\n    pass\n", "python").is_empty());
    }

    #[test]
    fn declarations_without_code_are_not_functions() {
        let src = "\
from typing import Protocol, overload


class Store(Protocol):
    def load(self, key: str) -> bytes: ...

    def save(self, key: str, data: bytes) -> None:
        \"\"\"Store the data under the key.\"\"\"


@overload
def fetch(key: int) -> bytes: ...
@overload
def fetch(key: str) -> bytes: ...
def fetch(key):
    return store.load(str(key))


def noop():
    pass
";
        let fns = extract_functions(src, "python");
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["fetch", "noop"],
            "stubs declare, `pass` is code"
        );
        assert_eq!(fns[0].start.line, 15, "the overloads before it are skipped");
    }

    #[test]
    fn the_size_limits_read_code_without_its_docstring_and_comments() {
        let src = "\
class Recipe:
    def title(self) -> str:
        \"\"\"The title of the recipe.

        It comes from the profile, which keeps the original
        capitalization and the punctuation the author used,
        so a title is never rewritten on the way to the page.
        \"\"\"
        # the profile is loaded with the recipe
        return self.profile.title
";
        let f = &extract_functions(src, "python")[0];
        let size = f.code_size.expect("python functions carry their code size");
        assert_eq!(
            size.lines, 2,
            "the def line to the return, docstring left out"
        );
        assert!(size.tokens <= 14, "{size:?}");
        assert_eq!((f.start.line, f.end.line), (2, 10), "the span is unchanged");
    }

    #[test]
    fn a_deep_expression_does_not_overflow_the_stack() {
        let src = format!("def f(a):\n    return {}a\n", "a + ".repeat(50_000));
        let fns = extract_functions(&src, "python");
        assert_eq!(fns.len(), 1);
        assert!(
            fns[0].kinds.len() < 4 * super::MAX_DEPTH,
            "the walk stops at the depth cap: {}",
            fns[0].kinds.len()
        );
    }

    #[test]
    fn defs_nested_past_the_cap_get_no_function_of_their_own() {
        let mut src = String::new();
        for depth in 0..40 {
            let indent = "    ".repeat(depth);
            src.push_str(&format!("{indent}def f{depth}(a):\n{indent}    a.run()\n"));
        }
        let fns = extract_functions(&src, "python");
        assert_eq!(fns.len(), super::MAX_OPEN_FUNCTIONS);
        assert_eq!(fns[0].name, "f0");
    }
}
