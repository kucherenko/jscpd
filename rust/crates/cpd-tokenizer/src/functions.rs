//! Function extraction for similarity scoring (issue #999, stage 2).
//!
//! A [`FunctionExtractor`] turns a source file into its functions, each with
//! a name, a span and the pre-order sequence of syntax-tree node types
//! inside it. Node *types* only: identifiers and literal values are not part
//! of the sequence, so the summary describes structure. The scoring in
//! `cpd_core::similarity` is grammar-agnostic; it only ever compares two
//! functions that carry the same [`FunctionExtractor::grammar`] id.
//!
//! # Adding a language
//!
//! 1. Implement [`FunctionExtractor`]: pick a stable `grammar` id (for a
//!    tree-sitter grammar, its name), list the jscpd `formats` it serves,
//!    and in `extract` walk the tree, opening a [`RawFunction`] at every
//!    function-like node and appending each visited node's type id (any
//!    dense `u16`, e.g. tree-sitter's `node.kind_id()`) to every open
//!    function.
//! 2. Add the extractor to [`EXTRACTORS`].
//!
//! Nothing else changes: the CLI, the MCP tool, the reporters and the
//! fixtures pick the new formats up through [`supports_functions`].

use crate::line_index::LineIndex;
use cpd_core::models::Location;
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::GetSpan;

/// A function found in a source, before token ranges are attached.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFunction {
    /// Grammar that produced `kinds`; functions of different grammars are
    /// never compared.
    pub grammar: &'static str,
    pub name: String,
    pub start: Location,
    pub end: Location,
    /// Where the code that names the function starts: the key of a method
    /// or property, or the variable a function is assigned to, when that
    /// code precedes `start`; `start` otherwise.
    pub head: Location,
    /// Pre-order syntax-tree node types of the function, itself included.
    pub kinds: Vec<u16>,
}

/// Language plug-in for function extraction.
pub trait FunctionExtractor: Send + Sync {
    /// Stable identifier of the grammar behind the node-type ids.
    fn grammar(&self) -> &'static str;
    /// jscpd format names this extractor serves.
    fn formats(&self) -> &'static [&'static str];
    /// All functions of `source`; empty when the source does not parse.
    fn extract(&self, source: &str, format: &str) -> Vec<RawFunction>;
}

/// Registered extractors, consulted in order. Add new languages here.
pub static EXTRACTORS: &[&dyn FunctionExtractor] = &[&OxcExtractor];

/// The extractor serving `format`, if any.
pub fn extractor_for(format: &str) -> Option<&'static dyn FunctionExtractor> {
    EXTRACTORS
        .iter()
        .copied()
        .find(|e| e.formats().contains(&format))
}

/// Formats handled by [`extract_functions`].
pub fn supports_functions(format: &str) -> bool {
    extractor_for(format).is_some()
}

/// Every format some extractor serves, for messages and docs.
pub fn supported_function_formats() -> Vec<&'static str> {
    EXTRACTORS
        .iter()
        .flat_map(|e| e.formats().iter().copied())
        .collect()
}

/// Extract every function of a source. Returns an empty vector for formats
/// without an extractor and for sources that fail to parse.
pub fn extract_functions(source: &str, format: &str) -> Vec<RawFunction> {
    extract_with(extractor_for(format), source, format)
}

/// Every function of `source` as `extractor` finds them; empty without an
/// extractor and for an empty source. For callers that pick extractors from
/// a registry of their own, like `--semantic`'s.
pub fn extract_with(
    extractor: Option<&dyn FunctionExtractor>,
    source: &str,
    format: &str,
) -> Vec<RawFunction> {
    match extractor {
        Some(extractor) if !source.is_empty() => extractor.extract(source, format),
        _ => Vec::new(),
    }
}

/// JavaScript, TypeScript, JSX and TSX through the oxc parser.
pub struct OxcExtractor;

impl FunctionExtractor for OxcExtractor {
    fn grammar(&self) -> &'static str {
        "oxc"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["javascript", "typescript", "jsx", "tsx"]
    }

    fn extract(&self, source: &str, format: &str) -> Vec<RawFunction> {
        extract_with_oxc(source, format)
    }
}

fn extract_with_oxc(source: &str, format: &str) -> Vec<RawFunction> {
    let allocator = Allocator::new();
    let source_type = crate::javascript::source_type_for_format(format);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    // Recoverable diagnostics leave a usable (possibly partial) AST; only a
    // parser that gave up yields nothing (issue #1023).
    if parsed.fatal_error {
        return Vec::new();
    }
    let line_index = LineIndex::new(source.as_bytes());
    let mut extractor = Extractor {
        frames: Vec::new(),
        out: Vec::new(),
        pending_name: None,
        pending_head: None,
        pending_call: None,
        line_index: &line_index,
        len: source.len(),
    };
    extractor.visit_program(&parsed.program);
    extractor.out
}

struct Frame {
    name: String,
    head: u32,
    start: u32,
    end: u32,
    kinds: Vec<u16>,
}

struct Extractor<'i> {
    frames: Vec<Frame>,
    out: Vec<RawFunction>,
    /// Name from the enclosing declarator, property or method, consumed by
    /// the next function node.
    pending_name: Option<String>,
    /// Where the code that named `pending_name` starts, and where its
    /// value starts: the function that starts there takes that code as its
    /// head.
    pending_head: Option<(u32, u32)>,
    /// The call (by its span: `test.each(t)('title', fn)` and its inner
    /// `test.each(t)` start at the same byte) that set `pending_name` for
    /// its test-case callback; leaving that call drops a name no function
    /// took.
    pending_call: Option<(u32, u32)>,
    line_index: &'i LineIndex,
    len: usize,
}

impl Extractor<'_> {
    fn open(&mut self, name: String, start: u32, end: u32) {
        // Only the function the naming code is about takes its head; one
        // that opens before it (an arrow in `test.each(table)`) leaves it.
        let head = match self.pending_head {
            Some((head, value)) if value == start => {
                self.pending_head = None;
                head
            }
            _ => start,
        };
        self.frames.push(Frame {
            name,
            head,
            start,
            end,
            kinds: Vec::new(),
        });
    }

    fn close(&mut self) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let start = (frame.start as usize).min(self.len);
        let end = (frame.end as usize).min(self.len);
        let head = (frame.head as usize).min(start);
        self.out.push(RawFunction {
            grammar: OxcExtractor.grammar(),
            name: frame.name,
            start: self.line_index.location(start),
            end: self.line_index.location(end),
            head: self.line_index.location(head),
            kinds: frame.kinds,
        });
    }

    /// The pending name, for the function that starts at `start`. A test
    /// title belongs to its callback alone: a function in `.each(table)`
    /// before it, or one nested in a named callback, does not take it.
    fn take_name(&mut self, start: u32) -> Option<String> {
        if self.pending_call.is_some() {
            if self.pending_head.map(|(_, callback)| callback) != Some(start) {
                return None;
            }
            self.pending_call = None;
        }
        self.pending_name.take()
    }

    /// Remember where the code naming the next function starts, for the
    /// function that is the named value itself (`value` starts there), not
    /// one nested in it.
    fn name_head(&mut self, head: u32, value: Option<u32>) {
        self.pending_head = match (&self.pending_name, value) {
            (Some(_), Some(value)) => Some((head, value)),
            _ => None,
        };
    }
}

impl<'a> Visit<'a> for Extractor<'_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::VariableDeclarator(d) => {
                self.pending_name = d.id.get_identifier_name().map(|n| n.to_string());
                self.name_head(d.span.start, d.init.as_ref().map(|v| v.span().start));
            }
            AstKind::MethodDefinition(m) => {
                self.pending_name = m.key.static_name().map(|n| n.into_owned());
                self.name_head(m.key.span().start, Some(m.value.span.start));
            }
            AstKind::PropertyDefinition(p) => {
                self.pending_name = p.key.static_name().map(|n| n.into_owned());
                self.name_head(p.key.span().start, p.value.as_ref().map(|v| v.span().start));
            }
            AstKind::ObjectProperty(p) => {
                self.pending_name = p.key.static_name().map(|n| n.into_owned());
                self.name_head(p.key.span().start, Some(p.value.span().start));
            }
            AstKind::CallExpression(call) => {
                if let Some((title, callback)) = test_case(call) {
                    self.pending_name = Some(title);
                    self.pending_head = Some((call.span.start, callback));
                    self.pending_call = Some((call.span.start, call.span.end));
                }
            }
            // A function without a body has no code to compare: an
            // overload signature, `declare function`, an abstract method,
            // every function of a `.d.ts` file.
            AstKind::Function(f) if f.body.is_none() => {}
            AstKind::Function(f) => {
                let own = f.id.as_ref().map(|id| id.name.to_string());
                let name = match own {
                    Some(name) => name,
                    None => self
                        .take_name(f.span.start)
                        .unwrap_or_else(|| "<anonymous>".to_string()),
                };
                let span = f.span;
                self.open(name, span.start, span.end);
            }
            AstKind::ArrowFunctionExpression(a) => {
                let name = self
                    .take_name(a.span.start)
                    .unwrap_or_else(|| "<arrow>".to_string());
                let span = a.span;
                self.open(name, span.start, span.end);
            }
            _ => {}
        }
        let ty = kind.ty() as u16;
        for frame in &mut self.frames {
            frame.kinds.push(ty);
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(f) if f.body.is_none() => {}
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => self.close(),
            AstKind::CallExpression(call)
                if self.pending_call == Some((call.span.start, call.span.end)) =>
            {
                self.pending_name = None;
                self.pending_head = None;
                self.pending_call = None;
            }
            AstKind::VariableDeclarator(_)
            | AstKind::MethodDefinition(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::ObjectProperty(_) => {
                self.pending_name = None;
                self.pending_head = None;
            }
            _ => {}
        }
        let _ = kind.span();
    }
}

/// Functions that declare one test case in the JavaScript test frameworks
/// (Jest, Vitest, Mocha, Jasmine, node:test, Bun): `it('title', fn)`, with
/// `.only`, `.skip`, `.each(table)` and the like after it. Suites
/// (`describe`) and hooks (`beforeEach`) are left out: they group or set up
/// tests, while a test case is what a port carries over one by one.
pub const TEST_CASE_CALLS: &[&str] = &["it", "test", "specify", "fit", "xit", "xtest", "bench"];

/// Members of a test function that declare something other than a test
/// case: a suite, a step inside a test, a hook, or configuration
/// (`test.describe`, `test.step`, `test.beforeEach`, `test.use`).
pub const NOT_TEST_CASES: &[&str] = &[
    "describe",
    "step",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "use",
    "extend",
];

/// The title of the test case `call` declares and where its callback
/// starts, when `call` is `it('rounds cents', () => …)` or one of its
/// variants and the title is a plain string. The callback then goes by the
/// title, which is what names a test in these frameworks, where it would
/// otherwise be an anonymous arrow; its text starts at the call, so the
/// title is part of what a model sees.
fn test_case(call: &oxc_ast::ast::CallExpression<'_>) -> Option<(String, u32)> {
    use oxc_ast::ast::Expression;
    // `it`, `it.only`, `test.each(table)`, `it.concurrent.each(table)`,
    // but not Playwright's `test.describe(…)` or `test.step(…)`.
    let mut callee = &call.callee;
    let root = loop {
        match callee {
            Expression::Identifier(id) => break id.name.as_str(),
            Expression::StaticMemberExpression(member) => {
                if NOT_TEST_CASES.contains(&member.property.name.as_str()) {
                    return None;
                }
                callee = &member.object;
            }
            Expression::CallExpression(inner) => callee = &inner.callee,
            _ => return None,
        }
    };
    if !TEST_CASE_CALLS.contains(&root) {
        return None;
    }
    let mut args = call.arguments.iter().filter_map(|a| a.as_expression());
    let title = match args.next()? {
        Expression::StringLiteral(literal) => literal.value.to_string(),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            let text = template.quasis.first()?;
            text.value
                .cooked
                .as_ref()
                .unwrap_or(&text.value.raw)
                .to_string()
        }
        _ => return None,
    };
    let callback = args.find_map(|a| match a {
        Expression::ArrowFunctionExpression(f) => Some(f.span.start),
        Expression::FunctionExpression(f) => Some(f.span.start),
        _ => None,
    })?;
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some((title, callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names of `fns`, in the order the extractor emits them.
    fn names(fns: &[RawFunction]) -> Vec<&str> {
        fns.iter().map(|f| f.name.as_str()).collect()
    }

    const SRC: &str = "export function total(items) {\n  let sum = 0;\n  for (const it of items) { sum += it.price; }\n  return sum;\n}\nconst double = (x) => x * 2;\nclass Cart {\n  add(item) { this.items.push(item); }\n}\nconst obj = { run() { return 1; }, cb: function () { return 2; } };\n";

    #[test]
    fn extracts_declarations_arrows_methods_and_properties_with_names() {
        let fns = extract_functions(SRC, "javascript");
        assert_eq!(names(&fns), vec!["total", "double", "add", "run", "cb"]);
        let total = &fns[0];
        assert_eq!((total.start.line, total.end.line), (1, 5));
        assert!(total.kinds.len() > 20, "{}", total.kinds.len());
        assert_eq!(total.kinds[0], oxc_ast::AstType::Function as u16);
    }

    #[test]
    fn nested_functions_are_emitted_separately_and_contribute_to_the_outer() {
        let src = "function outer() {\n  const inner = () => 1;\n  return inner();\n}\n";
        let fns = extract_functions(src, "typescript");
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "inner"); // closed first
        assert_eq!(fns[1].name, "outer");
        assert!(fns[1].kinds.len() > fns[0].kinds.len());
    }

    #[test]
    fn test_case_callbacks_go_by_their_titles() {
        let src = "describe('money', () => {\n  beforeEach(() => reset());\n  it('rounds  cents', () => {\n    expect(round(149)).toBe(100);\n  });\n  test.each([[1, 2]])('adds %i', (a, b) => {\n    expect(a + b).toBe(3);\n  });\n  it.only(`keeps ${'x'} dynamic`, () => {});\n  it('has no callback');\n  const later = () => 1;\n  test(\"async one\", async function () { await later(); });\n  it('named', function named() { [1].map((x) => x * 2); });\n  test.each([[() => 1]])('table', (f) => f());\n  test.describe('suite', () => {});\n  test.step('step', async () => {});\n});\n";
        let fns = extract_functions(src, "typescript");
        assert_eq!(
            names(&fns),
            vec![
                "<arrow>",
                "rounds cents",
                "adds %i",
                "<arrow>",
                "later",
                "async one",
                "<arrow>",
                "named",
                "<arrow>",
                "table",
                "<arrow>",
                "<arrow>",
                "<arrow>"
            ],
            "hooks, suites and dynamic titles stay anonymous; a test without a callback names nothing"
        );
        // The test's code starts at the call, so its title is part of it.
        let rounds = &fns[1];
        assert!(src[rounds.head.offset as usize..].starts_with("it('rounds  cents', () =>"));
        assert!(src[rounds.start.offset as usize..].starts_with("() =>"));
    }

    #[test]
    fn registry_dispatches_by_format_and_tags_the_grammar() {
        assert_eq!(extractor_for("typescript").unwrap().grammar(), "oxc");
        assert!(extractor_for("python").is_none());
        let formats = supported_function_formats();
        for f in ["javascript", "typescript", "jsx", "tsx"] {
            assert!(formats.contains(&f), "{f}");
            assert!(supports_functions(f));
        }
        let fns = extract_functions("const f = () => 1;", "jsx");
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].grammar, "oxc");
    }

    #[test]
    fn unsupported_or_empty_sources_yield_nothing() {
        assert!(extract_functions("def f():\n  pass\n", "python").is_empty());
        assert!(extract_functions("", "javascript").is_empty());
    }

    #[test]
    fn redeclared_functions_are_still_extracted() {
        let src = "function f(a) { return a + 1; }\nfunction f(b) { return b + 1; }\n";
        let fns = extract_functions(src, "javascript");
        assert_eq!(
            fns.len(),
            2,
            "a redeclaration diagnostic must not drop the file"
        );
        assert_eq!(fns[0].kinds, fns[1].kinds);
    }

    #[test]
    fn declarations_without_a_body_are_not_functions() {
        let src = "export function copy(src: string): void;\nexport function copy(src: string, dest: string): void;\nexport function copy(src: string, dest?: string): void {\n  write(src, dest);\n}\ndeclare function native(a: number): number;\nabstract class Base {\n  abstract run(): void;\n  go(a: string): void;\n  go(a: unknown) {\n    const twice = () => a;\n    return twice();\n  }\n}\n";
        let fns = extract_functions(src, "typescript");
        assert_eq!(names(&fns), vec!["copy", "twice", "go"]);
        assert_eq!((fns[0].start.line, fns[0].end.line), (3, 5));
        let declarations = "export declare function copy(\n  src: string,\n  dest: string,\n  options?: object,\n): Promise<void>;\nexport class Fs {\n  read(path: string): string;\n}\n";
        assert!(extract_functions(declarations, "typescript").is_empty());
    }

    #[test]
    fn renamed_copies_share_the_same_kind_sequence() {
        let a = extract_functions("function a(x) { return x + 1; }", "javascript");
        let b = extract_functions("function b(y) { return y + 1; }", "javascript");
        assert_eq!(a[0].kinds, b[0].kinds);
    }
}
