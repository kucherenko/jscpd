//! JavaScript, TypeScript, JSX and TSX functions through the oxc parser:
//! every function, method and arrow function with a body, nested ones
//! included, named by its declaration, the variable, property or method it
//! is assigned to, or the title of the test case it runs.
//!
//! A function passed to a call of a test framework (`it`, `describe`,
//! `beforeEach` and the like) is test code, and so is everything in it.

use super::{FunctionExtractor, RawFunction};
use cpd_tokenizer::line_index::LineIndex;
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::GetSpan;

/// How many functions can be open around a node and still record it: a
/// file of functions nested a thousand deep would otherwise cost a thousand
/// times its size. A function nested deeper is left out.
const MAX_OPEN_FUNCTIONS: usize = 16;

pub struct ScriptExtractor;

impl FunctionExtractor for ScriptExtractor {
    fn grammar(&self) -> &'static str {
        "oxc"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["javascript", "typescript", "jsx", "tsx"]
    }

    fn extract(&self, source: &str, format: &str) -> Vec<RawFunction> {
        let allocator = Allocator::new();
        let source_type = cpd_tokenizer::javascript::source_type_for_format(format);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        // Recoverable diagnostics leave a usable (possibly partial) AST;
        // only a parser that gave up yields nothing (issue #1023).
        if parsed.fatal_error {
            return Vec::new();
        }
        let line_index = LineIndex::new(source.as_bytes());
        let mut extractor = Extractor {
            frames: Vec::new(),
            opened: Vec::new(),
            out: Vec::new(),
            pending_name: None,
            pending_head: None,
            pending_call: None,
            owners: Vec::new(),
            test_callbacks: Vec::new(),
            line_index: &line_index,
            source,
        };
        extractor.visit_program(&parsed.program);
        extractor.out
    }
}

struct Frame {
    name: String,
    head: u32,
    start: u32,
    end: u32,
    test: bool,
}

struct Extractor<'i> {
    frames: Vec<Frame>,
    /// For every function being walked, whether it opened a frame; one
    /// nested past [`MAX_OPEN_FUNCTIONS`] does not.
    opened: Vec<bool>,
    out: Vec<RawFunction>,
    /// Name from the enclosing declarator, property or method, consumed by
    /// the next function node.
    pending_name: Option<String>,
    /// Where the code that named `pending_name` starts, and where its value
    /// starts: the function that starts there takes that code as its head.
    pending_head: Option<(u32, u32)>,
    /// The call (by its span: `test.each(t)('title', fn)` and its inner
    /// `test.each(t)` start at the same byte) that set `pending_name` for
    /// its test-case callback; leaving that call drops a name no function
    /// took.
    pending_call: Option<(u32, u32)>,
    /// Whether each function and class being walked is test code,
    /// innermost last.
    owners: Vec<bool>,
    /// Where the functions passed to a call of a test framework start,
    /// until the walk reaches them: they are test code.
    test_callbacks: Vec<u32>,
    line_index: &'i LineIndex,
    source: &'i str,
}

impl Extractor<'_> {
    fn open(&mut self, name: String, start: u32, end: u32) {
        let callback = take(&mut self.test_callbacks, start);
        let test = callback || self.owners.last().is_some_and(|&test| test);
        self.owners.push(test);
        // Only the function the naming code is about takes its head; one
        // that opens before it (an arrow in `test.each(table)`) leaves it.
        let head = match self.pending_head {
            Some((head, value)) if value == start => {
                self.pending_head = None;
                head
            }
            _ => start,
        };
        let opens = self.frames.len() < MAX_OPEN_FUNCTIONS;
        self.opened.push(opens);
        if opens {
            self.frames.push(Frame {
                name,
                head,
                start,
                end,
                test,
            });
        }
    }

    fn close(&mut self) {
        self.owners.pop();
        if !self.opened.pop().unwrap_or(false) {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let start = (frame.start as usize).min(self.source.len());
        let end = (frame.end as usize).min(self.source.len());
        let head = (frame.head as usize).min(start);
        self.out.push(RawFunction {
            grammar: "oxc",
            name: frame.name,
            start: self.line_index.location(start),
            end: self.line_index.location(end),
            head: self.line_index.location(head),
            test: frame.test,
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
                if test_call(call).is_some() {
                    self.test_callbacks.extend(callbacks(call));
                }
            }
            AstKind::Class(_) => {
                let test = self.owners.last().is_some_and(|&test| test);
                self.owners.push(test);
            }
            // A function without a body has no code: an overload signature,
            // `declare function`, an abstract method, every function of a
            // `.d.ts` file.
            AstKind::Function(f) if f.body.is_none() => {}
            AstKind::Function(f) => {
                let own = f.id.as_ref().map(|id| id.name.to_string());
                let name = match own {
                    Some(name) => name,
                    None => self
                        .take_name(f.span.start)
                        .unwrap_or_else(|| "<anonymous>".to_string()),
                };
                self.open(name, f.span.start, f.span.end);
            }
            AstKind::ArrowFunctionExpression(a) => {
                let name = self
                    .take_name(a.span.start)
                    .unwrap_or_else(|| "<arrow>".to_string());
                self.open(name, a.span.start, a.span.end);
            }
            _ => {}
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(f) if f.body.is_none() => {}
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => self.close(),
            AstKind::Class(_) => {
                self.owners.pop();
            }
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
    }
}

/// Functions that declare one test case in the JavaScript test frameworks
/// (Jest, Vitest, Mocha, Jasmine, node:test, Bun): `it('title', fn)`, with
/// `.only`, `.skip`, `.each(table)` and the like after it.
const TEST_CASE_CALLS: &[&str] = &["it", "test", "specify", "fit", "xit", "xtest", "bench"];

/// Functions that declare a suite of test cases: `describe('title', fn)`,
/// Mocha's `context` and the TDD `suite`.
const TEST_SUITE_CALLS: &[&str] = &["describe", "fdescribe", "xdescribe", "context", "suite"];

/// Functions that declare a hook, which sets tests up or cleans after them:
/// `beforeEach(fn)`, Mocha's `before` and `after`.
const TEST_HOOK_CALLS: &[&str] = &[
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "before",
    "after",
];

/// Members that change how a test call runs, not what it declares:
/// `it.only`, `test.each(table)`, `describe.skip`, `test.skipIf(cond)`.
const TEST_MODIFIERS: &[&str] = &[
    "only",
    "skip",
    "todo",
    "each",
    "concurrent",
    "sequential",
    "shuffle",
    "failing",
    "fails",
    "fail",
    "fixme",
    "slow",
    "serial",
    "parallel",
    "skipIf",
    "runIf",
];

/// Whether `starts` holds `start`, which it then gives up.
fn take(starts: &mut Vec<u32>, start: u32) -> bool {
    match starts.iter().rposition(|&at| at == start) {
        Some(at) => {
            starts.remove(at);
            true
        }
        None => false,
    }
}

/// What a call of a JavaScript test framework declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestCall {
    Case,
    /// A step inside a test case, Playwright's `test.step('title', fn)`.
    Step,
    Suite,
    Hook,
}

/// What `call` declares in a test framework: its callee starts with a test
/// function, every member after it is a modifier or names a suite, step or
/// hook (Playwright's `test.describe`, `test.step`, `test.beforeEach`), and
/// a case, step or suite takes a title first. `None` for any other call, as
/// `it.children.map(fn)` on a variable named `it`.
fn test_call(call: &oxc_ast::ast::CallExpression<'_>) -> Option<TestCall> {
    use oxc_ast::ast::Expression;
    let mut members = Vec::new();
    let mut callee = call.callee.get_inner_expression();
    let root = loop {
        match callee {
            Expression::Identifier(id) => break id.name.as_str(),
            Expression::StaticMemberExpression(member) => {
                members.push(member.property.name.as_str());
                callee = member.object.get_inner_expression();
            }
            // `test.each(table)(title, fn)` and `test.skipIf(cond)(title, fn)`.
            Expression::CallExpression(inner) => callee = inner.callee.get_inner_expression(),
            // ``describe.each`table`(title, fn)``.
            Expression::TaggedTemplateExpression(tagged) => {
                callee = tagged.tag.get_inner_expression();
            }
            _ => return None,
        }
    };
    let mut kind = match root {
        root if TEST_CASE_CALLS.contains(&root) => TestCall::Case,
        root if TEST_SUITE_CALLS.contains(&root) => TestCall::Suite,
        root if TEST_HOOK_CALLS.contains(&root) => TestCall::Hook,
        _ => return None,
    };
    for &member in members.iter().rev() {
        kind = match member {
            member if TEST_MODIFIERS.contains(&member) => kind,
            "describe" => TestCall::Suite,
            "step" => TestCall::Step,
            member if TEST_HOOK_CALLS.contains(&member) => TestCall::Hook,
            _ => return None,
        };
    }
    let titled = kind != TestCall::Hook;
    let title = call.arguments.first().and_then(|a| a.as_expression());
    let has_title = title.is_some_and(|title| {
        matches!(
            title.get_inner_expression(),
            Expression::StringLiteral(_)
                | Expression::TemplateLiteral(_)
                | Expression::Identifier(_)
                | Expression::StaticMemberExpression(_)
                | Expression::BinaryExpression(_)
        )
    });
    let has_callback = !callbacks(call).is_empty();
    (has_callback && (has_title || !titled)).then_some(kind)
}

/// Where the functions passed to `call` start, also behind parentheses and
/// inside a wrapper call, as Angular's `fakeAsync(() => …)` and
/// `inject([Service], (service) => …)`.
fn callbacks(call: &oxc_ast::ast::CallExpression<'_>) -> Vec<u32> {
    use oxc_ast::ast::{Argument, Expression};
    let function = |argument: &Argument<'_>| match argument.as_expression()?.get_inner_expression()
    {
        Expression::ArrowFunctionExpression(f) => Some(f.span.start),
        Expression::FunctionExpression(f) => Some(f.span.start),
        _ => None,
    };
    let mut starts = Vec::new();
    for argument in &call.arguments {
        match function(argument) {
            Some(start) => starts.push(start),
            None => {
                if let Some(Expression::CallExpression(wrapper)) =
                    argument.as_expression().map(|e| e.get_inner_expression())
                {
                    starts.extend(wrapper.arguments.iter().filter_map(function));
                }
            }
        }
    }
    starts
}

/// The title of the test case `call` declares and where its callback
/// starts, when `call` is `it('rounds cents', () => …)` or one of its
/// variants and the title is a plain string. The callback then goes by the
/// title, which is what names a test in these frameworks, where it would
/// otherwise be an anonymous arrow; its text starts at the call, so the
/// title is part of what a model sees. Suites, steps and hooks keep their
/// callbacks' names.
fn test_case(call: &oxc_ast::ast::CallExpression<'_>) -> Option<(String, u32)> {
    use oxc_ast::ast::Expression;
    if test_call(call)? != TestCall::Case {
        return None;
    }
    let title = match call.arguments.first()?.as_expression()? {
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
    let callback = *callbacks(call).first()?;
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then_some((title, callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn functions(source: &str) -> Vec<RawFunction> {
        ScriptExtractor.extract(source, "typescript")
    }

    #[test]
    fn functions_take_the_names_of_what_they_are_assigned_to() {
        let found = functions(
            "export function total(items) { return items.length; }\nconst double = (x) => x * 2;\nclass Cart { add(item) { this.items.push(item); } }\nconst obj = { run() { return 1; }, cb: function () { return 2; } };\n",
        );
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["total", "double", "add", "run", "cb"]);
        let add = &found[2];
        // A method's text starts at its key.
        assert_eq!(add.head.offset as usize, "export function total(items) { return items.length; }\nconst double = (x) => x * 2;\nclass Cart { ".len());
    }

    #[test]
    fn test_cases_go_by_their_titles_and_are_test_code() {
        let found = functions(
            "describe('cart', () => {\n  beforeEach(() => setup());\n  it('adds   items', () => { add(1); });\n});\nfunction helper() { return 1; }\n",
        );
        let named: Vec<(&str, bool)> = found.iter().map(|f| (f.name.as_str(), f.test)).collect();
        assert_eq!(
            named,
            [
                ("<arrow>", true),
                ("adds items", true),
                ("<arrow>", true),
                ("helper", false)
            ]
        );
    }
}
