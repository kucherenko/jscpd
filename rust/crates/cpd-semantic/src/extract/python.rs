//! Python functions through the ruff parser: every `def` and `async def`
//! with code in its body, methods and nested functions included. A function
//! starts at `def` (or the `async` before it), after its decorators. A
//! declaration whose body is `...` or a docstring, such as an `@overload`
//! signature, a `.pyi` stub or a `Protocol` member, is no function.

use super::{FunctionExtractor, RawFunction};
use cpd_tokenizer::line_index::LineIndex;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{AnyNodeRef, Expr, Identifier, Stmt, StmtFunctionDef};

/// How deep the walk goes into the syntax tree; deeper nodes are left out.
/// ruff's visitor recurses once per level, and generated code such as a sum
/// of a hundred thousand terms would otherwise overflow the thread's stack.
/// Hand-written code stays far below it.
const MAX_DEPTH: usize = 1000;

/// How many functions can be open around a `def` and still record it.
const MAX_OPEN_FUNCTIONS: usize = 16;

/// The stack for dropping a tree deeper than [`MAX_DEPTH`]: a base, plus a
/// generous allowance per byte of source for a debug build's drop frames.
/// It is reserved address space; only the pages the drop touches are used.
const DROP_STACK_BASE: usize = 1 << 20;
const DROP_STACK_PER_BYTE: usize = 256;

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
        let mut walker = Functions {
            source,
            line_index: &line_index,
            open: Vec::new(),
            depth: 0,
            too_deep: false,
            out: Vec::new(),
        };
        walker.visit_body(&parsed.syntax().body);
        let (mut out, too_deep) = (walker.out, walker.too_deep);
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

struct Functions<'s> {
    source: &'s str,
    line_index: &'s LineIndex,
    /// For every `def` being walked, the function it records: none for a
    /// stub or one nested past [`MAX_OPEN_FUNCTIONS`].
    open: Vec<Option<(String, usize, usize)>>,
    depth: usize,
    /// Whether the walk met nodes deeper than [`MAX_DEPTH`].
    too_deep: bool,
    out: Vec<RawFunction>,
}

impl Functions<'_> {
    /// Where the code of `f` starts: its `def`, or the `async` before it.
    /// The node's own range starts at its first decorator.
    fn def_start(&self, f: &StmtFunctionDef) -> usize {
        let fallback = f.range.start().to_usize();
        let head = &self.source[..f.name.range.start().to_usize()];
        let Some(at) = head.rfind("def") else {
            return fallback;
        };
        let before = head[..at].trim_end();
        match f.is_async && before.ends_with("async") {
            true => before.len() - "async".len(),
            false => at,
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
        if let AnyNodeRef::StmtFunctionDef(f) = node {
            let recorded = self.open.iter().filter(|f| f.is_some()).count();
            let function = (recorded < MAX_OPEN_FUNCTIONS && !is_stub(&f.body)).then(|| {
                (
                    name_of(&f.name),
                    self.def_start(f),
                    f.range.end().to_usize(),
                )
            });
            self.open.push(function);
        }
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, node: AnyNodeRef<'a>) {
        let depth = self.depth;
        self.depth -= 1;
        if depth > MAX_DEPTH || !matches!(node, AnyNodeRef::StmtFunctionDef(_)) {
            return;
        }
        let Some(Some((name, start, end))) = self.open.pop() else {
            return;
        };
        let start = self.line_index.location(start);
        self.out.push(RawFunction {
            grammar: "python",
            name,
            head: start.clone(),
            start,
            end: self.line_index.location(end),
            test: false,
        });
    }
}

fn name_of(name: &Identifier) -> String {
    name.to_string()
}

/// Whether a `def` body only declares: it holds nothing but `...` and a
/// docstring.
fn is_stub(body: &[Stmt]) -> bool {
    let docstring = matches!(
        body.first(),
        Some(Stmt::Expr(stmt)) if matches!(stmt.value.as_ref(), Expr::StringLiteral(_))
    );
    body.iter().skip(usize::from(docstring)).all(|stmt| {
        matches!(stmt, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::EllipsisLiteral(_)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defs_methods_and_nested_functions_in_source_order_without_stubs() {
        let source = "@app.get('/')\nasync def index():\n    return 1\n\nclass A:\n    def m(self):\n        def inner():\n            return 2\n        return inner\n\n    def stub(self):\n        \"\"\"Doc.\"\"\"\n        ...\n";
        let found = PythonExtractor.extract(source, "python");
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["index", "m", "inner"]);
        assert_eq!(
            found[0].start.offset as usize,
            source.find("async").unwrap()
        );
        assert_eq!(found[0].head, found[0].start);
    }
}
