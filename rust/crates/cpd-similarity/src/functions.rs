//! Function extraction for similarity scoring (issue #999, stage 2).
//!
//! A [`FunctionExtractor`] turns a source file into its functions, each with
//! a name, a span and the pre-order sequence of syntax-tree node types
//! inside it. Node *types* only: identifiers and literal values are not part
//! of the sequence, so the summary describes structure. The scoring in the
//! crate root is grammar-agnostic; it only ever compares two functions that
//! carry the same [`FunctionExtractor::grammar`] id. An extractor can find
//! other units next to functions (classes, variables, type aliases) through
//! [`FunctionExtractor::extract_units`], each marked with its [`UnitKind`].
//!
//! Next to the sequence an extractor records the names whose role changes
//! what the code does, as [`RoleName`]s: the method each call invokes. The
//! role-aware mode of `--similarity-identifiers` puts them into the
//! sequence; the default leaves them out. It also records every literal as a
//! [`LiteralLeaf`] over its nodes, with a hash of its parsed value, for
//! `--similarity-literals`; the default keeps the literals' node types only.
//!
//! # Adding a language
//!
//! 1. Implement [`FunctionExtractor`]: pick a stable `grammar` id (for a
//!    tree-sitter grammar, its name), list the jscpd `formats` it serves,
//!    and in `extract` walk the tree, opening a [`RawFunction`] at every
//!    function-like node and appending each visited node's type id (any
//!    dense `u16`, e.g. tree-sitter's `node.kind_id()`) to every open
//!    function. For a call whose callee is a member, also append a
//!    [`RoleName`] with the member's name, after the call's node. For a
//!    literal, append a [`LiteralLeaf`] over its node and the nodes of its
//!    parts, with the [`literal_hash`] of its type and parsed value. A
//!    grammar without literal nodes records none.
//! 2. Add the extractor to [`EXTRACTORS`].
//!
//! Nothing else changes: the CLI, the MCP tool, the reporters and the
//! fixtures pick the new formats up through [`supports_functions`], and
//! code blocks embedded in Markdown and components through
//! [`extract_embedded_functions`].

mod python;

pub use python::PythonExtractor;

use crate::{
    CodeSize, DecoratorLeaf, FunctionSig, LiteralLeaf, RoleName, SignaturePolicy, Structure,
    UnitContext, UnitKind, literal_hash, name_hash,
};
use cpd_core::models::Location;
use cpd_tokenizer::line_index::LineIndex;
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::GetSpan;

/// A function found in a source, or another unit `--similarity` compares
/// (see [`UnitKind`]), before token ranges are attached.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFunction {
    /// Grammar that produced `kinds`; functions of different grammars are
    /// never compared.
    pub grammar: &'static str,
    /// What the unit is; units of different kinds are never compared.
    pub unit: UnitKind,
    pub name: String,
    pub start: Location,
    pub end: Location,
    /// Where the code that names the function starts: the key of a method
    /// or property, or the variable a function is assigned to, when that
    /// code precedes `start`; `start` otherwise.
    pub head: Location,
    /// Pre-order syntax-tree node types of the function, itself included.
    pub kinds: Vec<u16>,
    /// The method each call in the function invokes, placed after the
    /// call's node in `kinds`; only role-aware similarity reads them.
    pub names: Vec<RoleName>,
    /// The literals of the function, by their nodes in `kinds`; only the
    /// `--similarity-literals` modes other than the default read them.
    pub literals: Vec<LiteralLeaf>,
    /// The decorators in the function, its own and those of the units in
    /// it, by their nodes in `kinds`; `--similarity-decorators` decides how
    /// they count. The function's span starts after its own, in every mode.
    pub decorators: Vec<DecoratorLeaf>,
    /// The size of the function's code alone when its span also holds a
    /// docstring or comments that the tokenizer counts, as in Python; `None`
    /// when the span's tokens are all code.
    pub code_size: Option<CodeSize>,
    /// Where the function is declared, for [`crate::CandidatePolicy`].
    pub context: UnitContext,
}

/// How many functions can be open around a node and still record it.
/// Every node joins the sequence of each open function, so without a cap a
/// file of `def`s nested a thousand deep costs a thousand times its size. A
/// function nested deeper gets no sequence of its own; its code still counts
/// in the functions around it.
const MAX_OPEN_FUNCTIONS: usize = 16;

/// Language plug-in for function extraction.
pub trait FunctionExtractor: Send + Sync {
    /// Stable identifier of the grammar behind the node-type ids.
    fn grammar(&self) -> &'static str;
    /// jscpd format names this extractor serves.
    fn formats(&self) -> &'static [&'static str];
    /// All functions of `source`; empty when the source does not parse.
    fn extract(&self, source: &str, format: &str) -> Vec<RawFunction>;
    /// Every unit `--similarity` compares in `source` (see [`UnitKind`]):
    /// its functions, and its classes, variables and type aliases in the
    /// languages whose extractor finds them. An extractor that knows the
    /// size of a unit's code leaves out the units smaller than `min`, which
    /// `--min-tokens` and `--min-lines` would drop. The code in the byte
    /// ranges `ignored`, sorted and disjoint, is no part of any unit. The
    /// functions alone by default.
    fn extract_units(
        &self,
        source: &str,
        format: &str,
        min: CodeSize,
        ignored: &[[usize; 2]],
    ) -> Vec<RawFunction> {
        let _ = (min, ignored);
        self.extract(source, format)
    }
}

/// Registered extractors, consulted in order. Add new languages here.
pub static EXTRACTORS: &[&dyn FunctionExtractor] = &[&OxcExtractor, &PythonExtractor];

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

/// Extract every unit `--similarity` compares in a source: its functions,
/// and its classes, variables and type aliases where the extractor finds
/// them, of at least the size `min`. Like the token passes, the units leave
/// out the code in `jscpd:ignore` blocks and in the byte ranges `ignored`,
/// the matches of `--ignore-pattern`: it adds nothing to their summaries
/// and sizes.
pub fn extract_units(
    source: &str,
    format: &str,
    min: CodeSize,
    ignored: &[[usize; 2]],
) -> Vec<RawFunction> {
    extract_units_with(extractor_for(format), source, format, min, ignored)
}

/// Every function of `source` as `extractor` finds them; empty without an
/// extractor and for an empty source. For callers that pick extractors from
/// a registry of their own, like `--semantic`'s.
pub fn extract_with(
    extractor: Option<&dyn FunctionExtractor>,
    source: &str,
    format: &str,
) -> Vec<RawFunction> {
    run_extractor(extractor, source, |extractor| {
        extractor.extract(source, format)
    })
}

/// [`extract_with`] with every unit the extractor finds of at least the
/// size `min`, not only functions, without the code [`extract_units`]
/// leaves out.
pub fn extract_units_with(
    extractor: Option<&dyn FunctionExtractor>,
    source: &str,
    format: &str,
    min: CodeSize,
    ignored: &[[usize; 2]],
) -> Vec<RawFunction> {
    run_extractor(extractor, source, |extractor| {
        let ignored = left_out(source, format, ignored);
        extractor.extract_units(source, format, min, &ignored)
    })
}

/// The byte ranges of `source` that no unit holds, sorted and disjoint: its
/// `jscpd:ignore` blocks, as the tokenizer of `format` finds them, and the
/// ranges `ignored`.
fn left_out(source: &str, format: &str, ignored: &[[usize; 2]]) -> Vec<[usize; 2]> {
    let mut ranges = cpd_tokenizer::tokenizer::ignored_ranges(format, source);
    ranges.extend(ignored.iter().filter(|[start, end]| start < end));
    ranges.sort_unstable();
    let mut merged: Vec<[usize; 2]> = Vec::with_capacity(ranges.len());
    for [start, end] in ranges {
        match merged.last_mut() {
            Some(last) if start <= last[1] => last[1] = last[1].max(end),
            _ => merged.push([start, end]),
        }
    }
    merged
}

/// Whether `start..end` lies inside one of the sorted, disjoint `ranges`.
fn inside(ranges: &[[usize; 2]], start: usize, end: usize) -> bool {
    let at = ranges.partition_point(|range| range[0] <= start);
    at > 0 && end <= ranges[at - 1][1]
}

/// What `extract` finds with `extractor`; nothing without one and in an
/// empty source.
fn run_extractor(
    extractor: Option<&dyn FunctionExtractor>,
    source: &str,
    extract: impl FnOnce(&dyn FunctionExtractor) -> Vec<RawFunction>,
) -> Vec<RawFunction> {
    match extractor {
        Some(extractor) if !source.is_empty() => extract(extractor),
        _ => Vec::new(),
    }
}

/// Signatures of `functions` over the detection-token spans of the source
/// they are in, with the names and literals `policy` keeps and the size
/// limits read on each function's code. A function without a token inside
/// is left out.
pub fn signatures(
    functions: impl IntoIterator<Item = RawFunction>,
    spans: &[(Location, Location)],
    policy: SignaturePolicy,
) -> Vec<FunctionSig> {
    functions
        .into_iter()
        .filter_map(|f| {
            let structure = Structure {
                kinds: &f.kinds,
                names: &f.names,
                literals: &f.literals,
                decorators: &f.decorators,
            };
            FunctionSig::build_with(f.grammar, f.name, f.start, f.end, structure, policy, spans)
                .map(|sig| sig.with_code_size(f.code_size).with_unit(f.unit))
        })
        .collect()
}

/// Formats whose files embed code with functions in it: Markdown with its
/// code fences, and the components whose scripts [`extract_embedded_functions`]
/// reads.
pub fn embeds_functions(format: &str) -> bool {
    matches!(format, "markdown" | "md" | "vue" | "svelte" | "astro")
}

/// Every function in the code a host file embeds, paired with the format of
/// its block: the code fences of a Markdown file, the scripts and Astro
/// frontmatter of a Vue, Svelte or Astro component. Positions are the host
/// file's, so a function reports where it sits in the `.md` or `.vue` file.
/// Empty for other host formats and for blocks without an extractor.
pub fn extract_embedded_functions(source: &str, host_format: &str) -> Vec<(String, RawFunction)> {
    embedded(source, host_format, &[], |extractor, block, format, _| {
        extract_with(extractor, block, format)
    })
}

/// [`extract_embedded_functions`] with every unit `--similarity` compares,
/// of at least the size `min`, without the code [`extract_units`] leaves
/// out: the `jscpd:ignore` blocks of each block and the byte ranges
/// `ignored` of the host.
pub fn extract_embedded_units(
    source: &str,
    host_format: &str,
    min: CodeSize,
    ignored: &[[usize; 2]],
) -> Vec<(String, RawFunction)> {
    embedded(
        source,
        host_format,
        ignored,
        |extractor, block, format, ignored| {
            extract_units_with(extractor, block, format, min, ignored)
        },
    )
}

/// What `extract` finds in the blocks a host file embeds, placed in the host.
/// It gets the part of the host's byte ranges `ignored` in each block, by the
/// block's own offsets.
fn embedded(
    source: &str,
    host_format: &str,
    ignored: &[[usize; 2]],
    extract: impl Fn(Option<&dyn FunctionExtractor>, &str, &str, &[[usize; 2]]) -> Vec<RawFunction>,
) -> Vec<(String, RawFunction)> {
    let blocks = match host_format {
        "markdown" | "md" => cpd_tokenizer::markdown::code_blocks(source),
        "vue" | "svelte" | "astro" => cpd_tokenizer::sfc::script_blocks(source, host_format),
        _ => return Vec::new(),
    };
    let host = LineIndex::new(source.as_bytes());
    let mut out = Vec::new();
    for (format, range) in blocks {
        let Some(extractor) = extractor_for(&format) else {
            continue;
        };
        let place = |location: &Location| host.location(range.start + location.offset as usize);
        let local = cpd_tokenizer::tokenizer::ranges_in(ignored, range.clone());
        for mut function in extract(Some(extractor), &source[range.clone()], &format, &local) {
            function.start = place(&function.start);
            function.end = place(&function.end);
            function.head = place(&function.head);
            out.push((format.clone(), function));
        }
    }
    out
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
        extract_with_oxc(source, format, &[])
    }

    fn extract_units(
        &self,
        source: &str,
        format: &str,
        _min: CodeSize,
        ignored: &[[usize; 2]],
    ) -> Vec<RawFunction> {
        extract_with_oxc(source, format, ignored)
    }
}

/// The functions of `source`, without the code in the byte ranges
/// `ignored`, sorted and disjoint.
fn extract_with_oxc(source: &str, format: &str, ignored: &[[usize; 2]]) -> Vec<RawFunction> {
    let allocator = Allocator::new();
    let source_type = cpd_tokenizer::javascript::source_type_for_format(format);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    // Recoverable diagnostics leave a usable (possibly partial) AST; only a
    // parser that gave up yields nothing (issue #1023).
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
        templates: Vec::new(),
        tagged: Vec::new(),
        line_index: &line_index,
        len: source.len(),
        ignored,
    };
    extractor.visit_program(&parsed.program);
    extractor.out
}

struct Frame {
    name: String,
    head: u32,
    start: u32,
    end: u32,
    context: UnitContext,
    kinds: Vec<u16>,
    names: Vec<RoleName>,
    literals: Vec<LiteralLeaf>,
}

/// A function or class being walked, which the units in it are declared in.
struct Owner {
    function: bool,
    /// Test code, or inside test code.
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
    /// Where the code that named `pending_name` starts, and where its
    /// value starts: the function that starts there takes that code as its
    /// head.
    pending_head: Option<(u32, u32)>,
    /// The call (by its span: `test.each(t)('title', fn)` and its inner
    /// `test.each(t)` start at the same byte) that set `pending_name` for
    /// its test-case callback; leaving that call drops a name no function
    /// took.
    pending_call: Option<(u32, u32)>,
    /// The functions and classes being walked, innermost last.
    owners: Vec<Owner>,
    /// Where the functions passed to a call of a test framework start, until
    /// the walk reaches them: they are test code.
    test_callbacks: Vec<u32>,
    /// For every template being walked, whether a tag reads it: a tag sees
    /// the raw text, as `String.raw` does, and the others the cooked one.
    templates: Vec<bool>,
    /// Where the templates of the tagged templates being walked start, until
    /// the walk reaches them.
    tagged: Vec<u32>,
    line_index: &'i LineIndex,
    len: usize,
    /// The byte ranges whose code no function holds, sorted and disjoint:
    /// `jscpd:ignore` blocks and `--ignore-pattern` matches.
    ignored: &'i [[usize; 2]],
}

impl Extractor<'_> {
    /// Where the function that starts at `start` is declared, and what its
    /// own units are declared in.
    fn declare(&mut self, start: u32) -> UnitContext {
        let owner = self.owners.last();
        let callback = match self.test_callbacks.iter().rposition(|&at| at == start) {
            Some(at) => {
                self.test_callbacks.remove(at);
                true
            }
            None => false,
        };
        let context = UnitContext {
            local: owner.is_some_and(|owner| owner.function),
            test: callback || owner.is_some_and(|owner| owner.test),
        };
        self.owners.push(Owner {
            function: true,
            test: context.test,
        });
        context
    }

    fn open(&mut self, name: String, start: u32, end: u32) {
        let context = self.declare(start);
        // Only the function the naming code is about takes its head; one
        // that opens before it (an arrow in `test.each(table)`) leaves it.
        let head = match self.pending_head {
            Some((head, value)) if value == start => {
                self.pending_head = None;
                head
            }
            _ => start,
        };
        // A function the ignored code holds whole is no unit.
        let opens = self.frames.len() < MAX_OPEN_FUNCTIONS
            && !inside(self.ignored, start as usize, end as usize);
        self.opened.push(opens);
        if !opens {
            return;
        }
        self.frames.push(Frame {
            name,
            head,
            start,
            end,
            context,
            kinds: Vec::new(),
            names: Vec::new(),
            literals: Vec::new(),
        });
    }

    fn close(&mut self) {
        if !self.opened.pop().unwrap_or(false) {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let start = (frame.start as usize).min(self.len);
        let end = (frame.end as usize).min(self.len);
        let head = (frame.head as usize).min(start);
        self.out.push(RawFunction {
            grammar: OxcExtractor.grammar(),
            unit: UnitKind::Function,
            name: frame.name,
            start: self.line_index.location(start),
            end: self.line_index.location(end),
            head: self.line_index.location(head),
            kinds: frame.kinds,
            names: frame.names,
            literals: frame.literals,
            decorators: Vec::new(),
            code_size: None,
            context: frame.context,
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
                if is_test_call(call) {
                    self.test_callbacks.extend(callbacks(call));
                }
            }
            AstKind::Class(_) => {
                let test = self.owners.last().is_some_and(|owner| owner.test);
                self.owners.push(Owner {
                    function: false,
                    test,
                });
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
            AstKind::TaggedTemplateExpression(t) => self.tagged.push(t.quasi.span.start),
            AstKind::TemplateLiteral(t) => {
                let at = self.tagged.iter().rposition(|&start| start == t.span.start);
                if let Some(at) = at {
                    self.tagged.remove(at);
                }
                self.templates.push(at.is_some());
            }
            AstKind::TSTemplateLiteralType(_) => self.templates.push(false),
            _ => {}
        }
        let ty = kind.ty() as u16;
        let called = match kind {
            AstKind::CallExpression(call) => called_member(&call.callee),
            _ => None,
        };
        // Literals outside every function would be thrown away.
        let raw = self.templates.last() == Some(&true);
        let literal = match self.frames.is_empty() {
            true => None,
            false => literal_value(&kind, ty, raw),
        };
        // Ignored code adds nothing to the functions around it; the walk
        // goes on through it to keep its stacks.
        let span = kind.span();
        if inside(self.ignored, span.start as usize, span.end as usize) {
            return;
        }
        for frame in &mut self.frames {
            frame.kinds.push(ty);
            let at = (frame.kinds.len() - 1) as u32;
            if let Some(hash) = called {
                frame.names.push(RoleName { after: at, hash });
            }
            if let Some(value) = literal {
                frame.literals.push(LiteralLeaf { at, len: 1, value });
            }
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(f) if f.body.is_none() => {}
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                self.close();
                self.owners.pop();
            }
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
            AstKind::TemplateLiteral(_) | AstKind::TSTemplateLiteralType(_) => {
                self.templates.pop();
            }
            _ => {}
        }
        let _ = kind.span();
    }
}

/// The [`name_hash`] of the method a call invokes when its callee is a
/// member: `load` in `store.load(x)`, `store?.load(x)`, `this.#load()`,
/// `store['load'](x)` and ``store[`load`](x)``, also behind parentheses and
/// TypeScript's `!`, `as`, `satisfies` and `<T>`, as in `store.load!(x)`. A
/// plain call such as `load(x)` keeps no name, so a copy that calls a
/// renamed helper still matches.
fn called_member(callee: &oxc_ast::ast::Expression<'_>) -> Option<u64> {
    use oxc_ast::ast::Expression;
    let callee = callee.get_inner_expression();
    if let Expression::PrivateFieldExpression(member) = callee {
        return Some(name_hash(&member.field.name));
    }
    callee
        .as_member_expression()?
        .static_property_name()
        .map(name_hash)
}

/// `text` with its line breaks as a parser reads them: `\r\n` and a lone
/// `\r` are `\n`, so a file with CRLF line ends gives a multi-line string
/// the value it has in a file with LF ones.
pub(crate) fn normalize_newlines(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        if byte == b'\r' {
            bytes.next_if_eq(&b'\n');
            out.push(b'\n');
        } else {
            out.push(byte);
        }
    }
    out
}

/// The [`literal_hash`] of a literal node of type `ty`: a string, a number,
/// a bigint, a boolean, `null`, a regular expression, or the text of a
/// template between its substitutions, which stand as nodes of their own.
/// A number is its parsed value, so `0x10` equals `16`; a bigint is its
/// value in base 10. A template's text is the string it makes, or its raw
/// text when a tag reads it (`raw`), as `String.raw` does. JSX text is
/// markup and no literal.
fn literal_value(kind: &AstKind<'_>, ty: u16, raw: bool) -> Option<u64> {
    let hash = |value: &[u8]| Some(literal_hash(ty, value));
    match kind {
        AstKind::StringLiteral(s) => hash(s.value.as_bytes()),
        AstKind::NumericLiteral(n) => hash(&n.value.to_bits().to_le_bytes()),
        AstKind::BigIntLiteral(b) => hash(b.value.as_bytes()),
        AstKind::BooleanLiteral(b) => hash(&[u8::from(b.value)]),
        AstKind::NullLiteral(_) => hash(&[]),
        // The flags join the pattern's hash below its top bit.
        AstKind::RegExpLiteral(r) => hash(r.regex.pattern.text.as_bytes())
            .map(|pattern| pattern ^ (u64::from(r.regex.flags.bits()) << 1)),
        AstKind::TemplateElement(t) => match (raw, &t.value.cooked) {
            (false, Some(cooked)) => hash(cooked.as_bytes()),
            // Raw text keeps the line ends of the file, which the language
            // reads as `\n`.
            _ => match t.value.raw.as_bytes() {
                raw if raw.contains(&b'\r') => hash(&normalize_newlines(raw)),
                raw => hash(raw),
            },
        },
        _ => None,
    }
}

/// Functions that declare one test case in the JavaScript test frameworks
/// (Jest, Vitest, Mocha, Jasmine, node:test, Bun): `it('title', fn)`, with
/// `.only`, `.skip`, `.each(table)` and the like after it. Suites
/// (`describe`) and hooks (`beforeEach`) are left out: they group or set up
/// tests, while a test case is what a port carries over one by one.
pub const TEST_CASE_CALLS: &[&str] = &["it", "test", "specify", "fit", "xit", "xtest", "bench"];

/// The calls of the JavaScript test frameworks that group or set up test
/// cases: suites and hooks. The functions passed to them and to the test
/// cases of [`TEST_CASE_CALLS`], and everything in those, are test code.
const TEST_SUITE_CALLS: &[&str] = &[
    "describe",
    "fdescribe",
    "xdescribe",
    "suite",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
];

/// Whether `call` runs test code: `it(...)`, `describe.each(table)(...)`,
/// `test.step(...)`, `beforeEach(...)`.
fn is_test_call(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    use oxc_ast::ast::Expression;
    let mut callee = &call.callee;
    loop {
        match callee {
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                return TEST_CASE_CALLS.contains(&name) || TEST_SUITE_CALLS.contains(&name);
            }
            Expression::StaticMemberExpression(member) => callee = &member.object,
            Expression::CallExpression(inner) => callee = &inner.callee,
            _ => return false,
        }
    }
}

/// Where the functions passed to `call` start.
fn callbacks<'c>(call: &'c oxc_ast::ast::CallExpression<'_>) -> impl Iterator<Item = u32> + 'c {
    use oxc_ast::ast::Expression;
    call.arguments
        .iter()
        .filter_map(|argument| match argument.as_expression()? {
            Expression::ArrowFunctionExpression(f) => Some(f.span.start),
            Expression::FunctionExpression(f) => Some(f.span.start),
            _ => None,
        })
}

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
    fn functions_know_whether_they_are_local_or_test_code() {
        let src = "\
function load(store) {
  const pick = (rows) => rows[0];
  class Cache {
    get(key) { return this.rows[key]; }
  }
  return pick(store.rows);
}
describe('orders', () => {
  beforeEach(() => { reset(); });
  it('totals', () => {
    const sum = (a, b) => a + b;
    expect(sum(1, 2)).toBe(3);
  });
});
test('refund', function () { expect(refund()).toBe(0); });
app.get('/users', (req, res) => res.send(users));
const api = { list() { return users; } };
";
        let mut contexts: Vec<(u32, String, bool, bool)> = extract_functions(src, "javascript")
            .into_iter()
            .map(|f| (f.start.line, f.name, f.context.local, f.context.test))
            .collect();
        contexts.sort();
        let expected = [
            (1, "load", false, false),
            (2, "pick", true, false),
            (4, "get", false, false),
            (8, "<arrow>", false, true),
            (9, "<arrow>", true, true),
            (10, "totals", true, true),
            (11, "sum", true, true),
            (15, "refund", false, true),
            (16, "<arrow>", false, false),
            (17, "list", false, false),
        ];
        let expected: Vec<(u32, String, bool, bool)> = expected
            .iter()
            .map(|&(line, name, local, test)| (line, name.to_string(), local, test))
            .collect();
        assert_eq!(contexts, expected);
    }

    #[test]
    fn ignored_code_is_no_part_of_a_function() {
        let any = CodeSize {
            tokens: 0,
            lines: 0,
        };
        let units =
            |src: &str, ignored: &[[usize; 2]]| extract_units(src, "javascript", any, ignored);
        let plain = "function total(items) {\n  let sum = 0;\n  return sum;\n}\n";
        let marked = "function total(items) {\n  let sum = 0;\n  // jscpd:ignore-start\n  for (const item of items) { sum += item.price; }\n  // jscpd:ignore-end\n  return sum;\n}\n";
        let plain_kinds = units(plain, &[]).remove(0).kinds;
        assert_eq!(units(marked, &[]).remove(0).kinds, plain_kinds);
        // A range of `--ignore-pattern` leaves the loop out the same way.
        let looped = marked
            .replace("jscpd:ignore-start", "")
            .replace("jscpd:ignore-end", "");
        let at = looped.find("for (").unwrap();
        let end = at + looped[at..].find('}').unwrap() + 1;
        assert_eq!(units(&looped, &[[at, end]]).remove(0).kinds, plain_kinds);
        assert_ne!(units(&looped, &[]).remove(0).kinds, plain_kinds);
        let whole = format!("// jscpd:ignore-start\n{plain}// jscpd:ignore-end\n");
        assert!(
            units(&whole, &[]).is_empty(),
            "a function in ignored code is none"
        );
    }

    #[test]
    fn ignored_ranges_merge_and_hold_what_lies_inside_them() {
        let ranges = left_out("x = 1\n", "python", &[[10, 20], [4, 4], [15, 30], [40, 50]]);
        assert_eq!(
            ranges,
            vec![[10, 30], [40, 50]],
            "merged, without empty ones"
        );
        assert!(inside(&ranges, 12, 30));
        assert!(!inside(&ranges, 25, 41), "across two ranges");
        assert!(!inside(&ranges, 5, 12));
        assert!(!inside(&[], 1, 2));
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
        assert_eq!(extractor_for("python").unwrap().grammar(), "python");
        assert!(extractor_for("ruby").is_none());
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
        assert!(extract_functions("def f\n  1\nend\n", "ruby").is_empty());
        assert!(extract_functions("", "javascript").is_empty());
    }

    #[test]
    fn member_calls_record_the_called_method_and_plain_calls_nothing() {
        let first = |src: &str| extract_functions(src, "typescript").remove(0);
        let load = first("function a(store, x) { return store.load(x); }");
        let receiver = first("function b(repo, y) { return repo.load(y); }");
        let save = first("function c(store, x) { return store.save(x); }");
        assert_eq!(load.kinds, save.kinds, "the structure is the same");
        assert_eq!(
            load.names, receiver.names,
            "the receiver's name does not count"
        );
        assert_eq!(load.names.len(), 1);
        assert_ne!(load.names[0].hash, save.names[0].hash);
        let call = load.names[0].after as usize;
        assert_eq!(load.kinds[call], oxc_ast::AstType::CallExpression as u16);
        assert!(
            first("function d(x) { return loadItem(x); }")
                .names
                .is_empty()
        );
        let run = extract_functions(
            "class S { #load() {} run(s) { s?.load(); this.#load(); s['load'](); s[k](); } }",
            "typescript",
        )
        .into_iter()
        .find(|f| f.name == "run")
        .expect("the method run is extracted");
        let load_hash = load.names[0].hash;
        assert_eq!(
            run.names.iter().map(|n| n.hash).collect::<Vec<_>>(),
            vec![load_hash, load_hash, load_hash],
            "optional, private and string-keyed calls name their method; a computed key does not"
        );
    }

    #[test]
    fn wrapped_member_callees_still_name_their_method() {
        let names = |src: &str| -> Vec<u64> {
            extract_functions(src, "typescript")
                .remove(0)
                .names
                .iter()
                .map(|n| n.hash)
                .collect()
        };
        let load = name_hash("load");
        let src = "function f(s, x) { s.load!(x); (s.load)(x); (s.load as Fn)(x); (<Fn>s.load)(x); s[`load`](x); }";
        assert_eq!(names(src), vec![load; 5]);
    }

    #[test]
    fn functions_nested_past_the_cap_get_no_sequence_of_their_own() {
        let mut src = String::new();
        for depth in 0..40 {
            src.push_str(&format!("function f{depth}(a) {{ a.run(); "));
        }
        src.push_str(&"}".repeat(40));
        let fns = extract_functions(&src, "javascript");
        assert_eq!(fns.len(), MAX_OPEN_FUNCTIONS);
        assert!(
            fns.iter().any(|f| f.name == "f0"),
            "the outermost keep theirs"
        );
    }

    #[test]
    fn embedded_blocks_yield_functions_at_their_place_in_the_host_file() {
        let md = "# Guide\n\n```ts\nexport function a(x: number) {\n  return x + 1;\n}\n```\n\nText.\n\n```python\ndef b(y):\n    return y.run()\n```\n\n```ruby\ndef c\nend\n```\n";
        let fns = extract_embedded_functions(md, "markdown");
        let found: Vec<(&str, &str, u32)> = fns
            .iter()
            .map(|(format, f)| (format.as_str(), f.name.as_str(), f.start.line))
            .collect();
        assert_eq!(found, vec![("typescript", "a", 4), ("python", "b", 12)]);
        for (_, f) in &fns {
            let text = &md[f.start.offset as usize..f.end.offset as usize];
            assert!(text.contains(&format!(" {}(", f.name)), "{text}");
        }
        let vue = "<template>\n  <p>{{ x }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction total(items) {\n  return items.length;\n}\n</script>\n";
        let fns = extract_embedded_functions(vue, "vue");
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].0, "typescript");
        assert_eq!((fns[0].1.start.line, fns[0].1.end.line), (5, 7));
        assert!(extract_embedded_functions("const a = 1;", "javascript").is_empty());
        let ignored = "<!-- jscpd:ignore-start -->\n```js\nfunction f() { return 1; }\n```\n<!-- jscpd:ignore-end -->\n";
        assert!(extract_embedded_functions(ignored, "markdown").is_empty());
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

    /// The literals of the first function of `src`, as (node type, value).
    fn literals_of(src: &str, format: &str) -> Vec<(oxc_ast::AstType, u64)> {
        let f = extract_functions(src, format).remove(0);
        f.literals
            .iter()
            .map(|l| {
                assert_eq!(l.len, 1, "a JavaScript literal is one node");
                let ty = f.kinds[l.at as usize];
                let ty = [
                    oxc_ast::AstType::StringLiteral,
                    oxc_ast::AstType::NumericLiteral,
                    oxc_ast::AstType::BigIntLiteral,
                    oxc_ast::AstType::BooleanLiteral,
                    oxc_ast::AstType::NullLiteral,
                    oxc_ast::AstType::RegExpLiteral,
                    oxc_ast::AstType::TemplateElement,
                ]
                .into_iter()
                .find(|t| *t as u16 == ty)
                .expect("a literal sits at a literal's node");
                (ty, l.value)
            })
            .collect()
    }

    #[test]
    fn literals_are_recorded_by_category_and_parsed_value() {
        use oxc_ast::AstType::*;
        let found = literals_of(
            "function f(x) { return ['a', \"a\", 16, 0x10, 10n, true, null, /ab/g, `t${x}u`]; }",
            "javascript",
        );
        let types: Vec<_> = found.iter().map(|(ty, _)| *ty).collect();
        assert_eq!(
            types,
            vec![
                StringLiteral,
                StringLiteral,
                NumericLiteral,
                NumericLiteral,
                BigIntLiteral,
                BooleanLiteral,
                NullLiteral,
                RegExpLiteral,
                TemplateElement,
                TemplateElement
            ]
        );
        assert_eq!(found[0].1, found[1].1, "quotes do not change a string");
        assert_eq!(found[2].1, found[3].1, "0x10 is 16");
        assert_ne!(
            found[8].1, found[9].1,
            "each piece of a template has its text"
        );
        let other = literals_of("function g(y) { return ['b', 17]; }", "javascript");
        assert_ne!(other[0].1, found[0].1);
        assert_ne!(other[1].1, found[2].1);
    }

    #[test]
    fn a_tagged_template_is_read_by_its_raw_text() {
        let values = |src: &str| -> Vec<u64> {
            literals_of(src, "javascript")
                .into_iter()
                .map(|(_, value)| value)
                .collect()
        };
        let escaped =
            values("function f(s) { return new RegExp(String.raw`^\\d{3}\\.\\d{2}$`, s); }");
        let lost = values("function f(s) { return new RegExp(String.raw`^\\d{3}.\\d{2}$`, s); }");
        assert_ne!(
            escaped, lost,
            "the tag reads the raw text, where they differ"
        );
        assert_eq!(
            values("function f() { return `\\x41`; }"),
            values("function f() { return `A`; }"),
            "an untagged template is the string it makes"
        );
        assert_eq!(
            values("function f() { return String.raw`a\r\nb`; }"),
            values("function f() { return String.raw`a\nb`; }"),
            "line ends are read as the language reads them"
        );
        let cooked = values("function f() { return `A`; }")[0];
        assert!(
            values("function f(x) { return tag`a${`\\x41`}b`; }").contains(&cooked),
            "a template inside a tagged one is not tagged"
        );
    }

    #[test]
    fn typescript_literal_types_hold_literals_and_jsx_text_is_markup() {
        let typed = literals_of(
            "function f(s: 'on' | 'off'): 1 { return s === 'on' ? 1 : 0; }",
            "typescript",
        );
        assert_eq!(
            typed.len(),
            6,
            "two in the parameter type, one in the return type, three in the body"
        );
        let markup = literals_of(
            "const C = () => <p title=\"greeting\">Hello, world</p>;",
            "tsx",
        );
        assert_eq!(
            markup.iter().map(|(ty, _)| *ty).collect::<Vec<_>>(),
            vec![oxc_ast::AstType::StringLiteral],
            "the attribute is a string; the text between the tags is not a literal"
        );
    }

    #[test]
    fn literal_modes_change_signatures_only_where_literals_differ() {
        use crate::{SimilarityIdentifiers, SimilarityLiterals, bag_jaccard};
        let body = |a: &str, b: &str| {
            format!(
                "function retry(url, session) {{\n  const headers = {{ accept: {a} }};\n  let response = session.get(url, {{ headers, timeout: {b} }});\n  if (response.status === 503) {{\n    response = session.get(url, {{ headers, timeout: {b} }});\n  }}\n  return response;\n}}\n"
            )
        };
        let sig = |src: &str, literals| {
            let options = cpd_tokenizer::tokenizer::TokenizeOptions::new(
                cpd_tokenizer::tokenizer::Mode::Mild,
            );
            let spans =
                cpd_tokenizer::tokenizer::tokenize_to_detection("javascript", src, &options)
                    .iter()
                    .map(|t| (t.start.clone(), t.end.clone()))
                    .collect::<Vec<_>>();
            let policy = SignaturePolicy {
                identifiers: SimilarityIdentifiers::Ignore,
                literals,
                ..SignaturePolicy::default()
            };
            signatures(extract_functions(src, "javascript"), &spans, policy).remove(0)
        };
        let score =
            |a: &str, b: &str, mode| bag_jaccard(&sig(a, mode).shingles, &sig(b, mode).shingles);
        let json = body("'application/json'", "10.5");
        let csv = body("'text/csv'", "30");
        let numbered = body("1", "10.5");
        assert_eq!(score(&json, &csv, SimilarityLiterals::Categories), 1.0);
        assert!(score(&json, &csv, SimilarityLiterals::Values) < 1.0);
        assert!(score(&json, &numbered, SimilarityLiterals::Categories) < 1.0);
        assert_eq!(score(&json, &numbered, SimilarityLiterals::Generic), 1.0);
        assert_eq!(score(&json, &numbered, SimilarityLiterals::Omit), 1.0);
        // By category, the literals change nothing: the signature is the one
        // of the node types alone.
        let options =
            cpd_tokenizer::tokenizer::TokenizeOptions::new(cpd_tokenizer::tokenizer::Mode::Mild);
        let spans: Vec<_> =
            cpd_tokenizer::tokenizer::tokenize_to_detection("javascript", &json, &options)
                .iter()
                .map(|t| (t.start.clone(), t.end.clone()))
                .collect();
        let f = extract_functions(&json, "javascript").remove(0);
        let plain = FunctionSig::build(f.grammar, f.name, f.start, f.end, &f.kinds, &spans);
        assert_eq!(Some(sig(&json, SimilarityLiterals::Categories)), plain);
        // Without literals every mode gives that signature.
        let bare = "function f(a, b) {\n  const c = a.x + b.y;\n  if (c > a.z) {\n    return b.w(c);\n  }\n  return a.v(c);\n}\n";
        for mode in [
            SimilarityLiterals::Values,
            SimilarityLiterals::Generic,
            SimilarityLiterals::Omit,
        ] {
            assert_eq!(
                sig(bare, mode),
                sig(bare, SimilarityLiterals::Categories),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn renamed_copies_share_the_same_kind_sequence() {
        let a = extract_functions("function a(x) { return x + 1; }", "javascript");
        let b = extract_functions("function b(y) { return y + 1; }", "javascript");
        assert_eq!(a[0].kinds, b[0].kinds);
    }
}
