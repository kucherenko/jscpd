//! Python functions through the ruff parser.

use super::{FunctionExtractor, MAX_OPEN_FUNCTIONS, RawFunction, normalize_newlines};
use crate::{
    CodeSize, DecoratorLeaf, LiteralLeaf, RoleName, UnitKind, decorator_hash, literal_hash,
    name_hash,
};
use cpd_tokenizer::line_index::LineIndex;
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{
    AnyNodeRef, BytesLiteralValue, Decorator, Expr, ExprFString, FStringPartRef, Identifier,
    NodeKind, Number, Singleton, Stmt, StmtClassDef, StmtFunctionDef, TypeParam,
};
use ruff_text_size::{Ranged, TextRange};
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

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
/// its body, methods and nested functions included. Next to the functions it
/// finds the other units `--similarity` compares (issue #1132): every
/// `class`, and the assignments and type aliases at module level or in a
/// class body, the constants and fields. An assignment inside a function is
/// part of that function, not a unit. A function starts at `def` (or the
/// `async` before it) and a class at `class`; their decorators follow them
/// in the node sequence, each a [`DecoratorLeaf`] that
/// `--similarity-decorators` leaves out, names or keeps. Type annotations,
/// type parameters and docstrings keep their place in the sequence, while
/// the size limits read the code alone: a docstring, a decorator (of the
/// unit or of a unit in it) and comments add no tokens or lines there.
/// A declaration whose body is `...` or a docstring, such as an `@overload`
/// signature, a `.pyi` stub or a `Protocol` member, is no function to
/// compare, as with bodyless TypeScript functions.
///
/// Literals are strings with their parts (`"a" "b"` is one), bytes, numbers,
/// booleans, `None`, `...`, the values of `case` patterns, and the text of an
/// f-string or t-string outside its replacement fields, format specs
/// included; the fields stand as nodes of their own. The plain parts of an
/// f-string that follow each other, as in `"a" "b" f"{x}"`, are one literal.
/// A docstring is documentation and a string in a type annotation is a type,
/// as in `-> "User"`, so both keep their node types whatever
/// `--similarity-literals` says; the values in `Literal[...]` are literals.
pub struct PythonExtractor;

impl FunctionExtractor for PythonExtractor {
    fn grammar(&self) -> &'static str {
        "python"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["python"]
    }

    fn extract(&self, source: &str, _format: &str) -> Vec<RawFunction> {
        Self::walk(source, None)
    }

    fn extract_units(&self, source: &str, _format: &str, min: CodeSize) -> Vec<RawFunction> {
        Self::walk(source, Some(min))
    }
}

impl PythonExtractor {
    /// The functions of `source`; with `min`, its other units too, and only
    /// the units of at least that size.
    fn walk(source: &str, min: Option<CodeSize>) -> Vec<RawFunction> {
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
            min,
            frames: Vec::new(),
            openers: Vec::new(),
            docstring_spans: Vec::new(),
            decorator_spans: Vec::new(),
            depth: 0,
            too_deep: false,
            units: Vec::new(),
            docstrings: HashSet::new(),
            annotations: HashSet::new(),
            literal_types: HashSet::new(),
            contexts: Vec::new(),
            runs: HashMap::new(),
            crlf: OnceCell::new(),
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

/// Where the string of a body's docstring starts: past the `(` of a
/// docstring in parentheses, where the statement starts.
fn docstring_start(body: &[Stmt]) -> Option<usize> {
    match body.first()? {
        Stmt::Expr(stmt) if matches!(stmt.value.as_ref(), Expr::StringLiteral(_)) => {
            Some(stmt.value.start().to_usize())
        }
        _ => None,
    }
}

/// The type annotations of `f`: of its parameters, its return and the
/// bounds and defaults of its type parameters.
fn annotations_of(f: &StmtFunctionDef) -> impl Iterator<Item = &Expr> {
    let type_params = f
        .type_params
        .iter()
        .flat_map(|params| params.iter())
        .flat_map(|param| match param {
            TypeParam::TypeVar(t) => [t.bound.as_deref(), t.default.as_deref()],
            TypeParam::ParamSpec(p) => [p.default.as_deref(), None],
            TypeParam::TypeVarTuple(t) => [t.default.as_deref(), None],
        })
        .flatten();
    f.parameters
        .iter()
        .filter_map(|param| param.annotation())
        .chain(f.returns.as_deref())
        .chain(type_params)
}

/// The name `expr` gives, alone or as the attribute of a module: `Literal`
/// for `Literal` and for `typing.Literal`.
fn name_of(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(id) => Some(id.id.as_str()),
        Expr::Attribute(attribute) => Some(attribute.attr.as_str()),
        _ => None,
    }
}

/// Whether `expr` names `name`, alone or as the attribute of a module:
/// `Literal` or `typing.Literal`.
fn is_named(expr: &Expr, name: &str) -> bool {
    name_of(expr) == Some(name)
}

/// Whether `expr` names `typing.Literal`, whose subscript holds values.
fn is_literal_type(expr: &Expr) -> bool {
    is_named(expr, "Literal")
}

/// Whether `node` is a literal followed by nodes of its own in the walk: a
/// string or bytes expression and its parts, a `case` value with its
/// literal.
fn opens_literal(node: AnyNodeRef<'_>) -> bool {
    match node {
        AnyNodeRef::ExprStringLiteral(_) | AnyNodeRef::ExprBytesLiteral(_) => true,
        AnyNodeRef::PatternMatchValue(pattern) => matches!(
            pattern.value.as_ref(),
            Expr::NumberLiteral(_) | Expr::StringLiteral(_) | Expr::BytesLiteral(_)
        ),
        _ => false,
    }
}

/// The [`literal_hash`] of a number of type `kind` by its parsed value: an
/// int by its value, a float by its bits, a complex number by both of its
/// parts. So `0x10` equals `16` and `1.0` equals `1.00`, while the int `1`
/// and the float `1.0` differ.
fn number_hash(kind: u16, number: &Number) -> u64 {
    match number {
        Number::Int(int) => match int.as_u64() {
            Some(small) => {
                let mut value = [b'i'; 9];
                value[1..].copy_from_slice(&small.to_le_bytes());
                literal_hash(kind, &value)
            }
            None => literal_hash(kind, &big_int_value(&int.to_string())),
        },
        Number::Float(float) => {
            let mut value = [b'f'; 9];
            value[1..].copy_from_slice(&float.to_bits().to_le_bytes());
            literal_hash(kind, &value)
        }
        Number::Complex { real, imag } => {
            let mut value = [b'c'; 17];
            value[1..9].copy_from_slice(&real.to_bits().to_le_bytes());
            value[9..].copy_from_slice(&imag.to_bits().to_le_bytes());
            literal_hash(kind, &value)
        }
    }
}

/// The value of an int of 64 bits or more, from its token, which ruff keeps
/// as written: its digits in base 2^32, so the base, the case of the
/// letters and the underscores of the token do not count.
fn big_int_value(token: &str) -> Vec<u8> {
    let token = token.replace('_', "").to_ascii_lowercase();
    let (radix, digits) = match token.get(..2) {
        Some("0x") => (16, &token[2..]),
        Some("0o") => (8, &token[2..]),
        Some("0b") => (2, &token[2..]),
        _ => (10, token.as_str()),
    };
    // Least significant limb first.
    let mut limbs: Vec<u32> = Vec::new();
    for digit in digits.chars().filter_map(|c| c.to_digit(radix)) {
        let mut carry = u64::from(digit);
        for limb in &mut limbs {
            let next = u64::from(*limb) * u64::from(radix) + carry;
            *limb = next as u32;
            carry = next >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
    }
    let mut value = vec![b'I'];
    for limb in limbs {
        value.extend_from_slice(&limb.to_le_bytes());
    }
    value
}

/// Whether a `def` or `class` body only declares: it holds nothing but
/// `...` and a docstring.
fn is_stub(body: &[Stmt]) -> bool {
    let skip = usize::from(body_docstring(body).is_some());
    body.iter().skip(skip).all(|stmt| {
        matches!(stmt, Stmt::Expr(e) if matches!(e.value.as_ref(), Expr::EllipsisLiteral(_)))
    })
}

/// Whether a method declares more than it does: its body is `...`, `pass`,
/// a docstring or `raise NotImplementedError`, as in an abstract method.
fn declares_only(body: &[Stmt]) -> bool {
    body.iter().all(|stmt| match stmt {
        Stmt::Pass(_) => true,
        Stmt::Expr(e) => matches!(
            e.value.as_ref(),
            Expr::EllipsisLiteral(_) | Expr::StringLiteral(_)
        ),
        Stmt::Raise(raise) => raise.exc.as_deref().is_some_and(|exc| {
            let class = match exc {
                Expr::Call(call) => call.func.as_ref(),
                other => other,
            };
            is_named(class, "NotImplementedError")
        }),
        _ => false,
    })
}

/// Whether a class body holds code to compare: a method that does more than
/// declare, or a statement other than a field. A body of declarations alone,
/// such as the members of an `Enum`, the fields of a `TypedDict` or a
/// model, or the stub methods of a `Protocol`, has the shape of its length:
/// two of them with as many fields match whatever the fields are, since
/// names do not count. A body nested past `budget` levels counts as code.
fn has_code(body: &[Stmt], budget: usize) -> bool {
    let Some(budget) = budget.checked_sub(1) else {
        return true;
    };
    body.iter().any(|stmt| match stmt {
        Stmt::Expr(e) => !matches!(
            e.value.as_ref(),
            Expr::EllipsisLiteral(_) | Expr::StringLiteral(_)
        ),
        Stmt::Pass(_) | Stmt::Assign(_) | Stmt::AnnAssign(_) | Stmt::TypeAlias(_) => false,
        Stmt::FunctionDef(f) => !declares_only(&f.body),
        Stmt::ClassDef(class) => has_code(&class.body, budget),
        _ => true,
    })
}

/// The name an assignment gives: `x` in `x = 1`, `limit` in
/// `Config.limit = 10`, `a, b` in `a, b = pair`. Past `budget` levels of
/// nesting it is `<assignment>`.
fn target_name(target: &Expr, budget: usize) -> String {
    let Some(budget) = budget.checked_sub(1) else {
        return "<assignment>".to_string();
    };
    let names = |items: &mut dyn Iterator<Item = &Expr>| {
        items
            .map(|item| target_name(item, budget))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match target {
        Expr::Name(name) => name.id.to_string(),
        Expr::Attribute(attribute) => attribute.attr.to_string(),
        Expr::Tuple(tuple) => names(&mut tuple.iter()),
        Expr::List(list) => names(&mut list.iter()),
        Expr::Starred(starred) => target_name(&starred.value, budget),
        _ => "<assignment>".to_string(),
    }
}

/// Whether a value is data: a literal, a table (a list, tuple, set or dict,
/// whatever it holds), arithmetic over literals such as `60 * 60`, or the
/// values listed in `Literal[...]`. An assignment of data is no unit: two
/// tables of one length have one shape whatever their rows hold, since the
/// names in them do not count, and the token passes find copied data. A
/// value nested past `budget` levels is no data.
fn is_data(expr: &Expr, budget: usize) -> bool {
    let Some(budget) = budget.checked_sub(1) else {
        return false;
    };
    match expr {
        Expr::Tuple(_) | Expr::List(_) | Expr::Set(_) | Expr::Dict(_) => true,
        Expr::UnaryOp(unary) => is_data(&unary.operand, budget),
        Expr::BinOp(binary) => is_data(&binary.left, budget) && is_data(&binary.right, budget),
        Expr::Subscript(subscript) => {
            is_literal_type(&subscript.value) && is_data(&subscript.slice, budget)
        }
        _ => expr.is_literal_expr(),
    }
}

/// Whether an annotation is `TypeAlias`, which makes its assignment a type
/// alias: `Pair: TypeAlias = tuple[int, int]`, or the same with the
/// annotation in quotes.
fn is_type_alias(annotation: &Expr) -> bool {
    match annotation {
        Expr::StringLiteral(string) => {
            string.value.to_str().rsplit('.').next() == Some("TypeAlias")
        }
        other => is_named(other, "TypeAlias"),
    }
}

/// What a docstring or a decorator takes from the size of the units around
/// it: its tokens, and its lines when it stands on lines of its own.
struct DocSpan {
    tokens: u32,
    lines: u32,
}

struct Frame {
    unit: UnitKind,
    name: String,
    /// Where the unit's code starts: at `def` or `class`, after its
    /// decorators, which the walk visits after its own node.
    start: usize,
    end: usize,
    /// The docstrings walked inside the unit, its own included, from this
    /// index of [`Functions::docstring_spans`] on.
    docs: usize,
    /// The decorators walked since the unit opened, from this index of
    /// [`Functions::decorator_spans`] on: its own, before `start`, and
    /// those of the units in it.
    decos: usize,
    kinds: Vec<u16>,
    names: Vec<RoleName>,
    literals: Vec<LiteralLeaf>,
    /// The literal being walked that has nodes of its own, by its index in
    /// `literals`: its length is known when the walk leaves them.
    open: Option<usize>,
    /// The last run of plain f-string parts, by its index in `literals`.
    run: Option<usize>,
    /// The decorators walked in the unit, its own and those of the units in
    /// it.
    decorators: Vec<DecoratorLeaf>,
    /// The decorator being walked, by its index in `decorators`: its length
    /// is known when the walk leaves it.
    decorator: Option<usize>,
}

/// What a node adds to the literals of the functions around it.
#[derive(Clone, Copy)]
enum Literal {
    /// A literal of one node.
    Leaf(u64),
    /// A literal whose nodes follow it in the walk.
    Open(u64),
    /// The first plain part of an f-string, with the value of the parts that
    /// follow it.
    Run(u64),
    /// A plain part of an f-string after another one.
    RunPart(u64),
}

struct Functions<'s> {
    source: &'s str,
    line_index: &'s LineIndex,
    /// Where the file's code tokens start, in order.
    code_tokens: &'s [usize],
    /// The smallest units to find, when classes, variables and type aliases
    /// are units next to the functions; `None` for the functions alone. A
    /// unit too small for it gets no frame.
    min: Option<CodeSize>,
    frames: Vec<Frame>,
    /// For every node that can open a unit being walked (a `def`, a `class`,
    /// an assignment, a type alias): whether it opened a frame, which a stub,
    /// a unit nested past [`MAX_OPEN_FUNCTIONS`] and an assignment inside a
    /// function do not, and whether it is a `def`. An assignment is a unit
    /// when the innermost of them is no `def`.
    openers: Vec<(bool, bool)>,
    /// The docstrings of the `def`s and `class`es walked so far, in order:
    /// a unit's code leaves out its own and those of the units inside it.
    docstring_spans: Vec<DocSpan>,
    /// The decorators walked so far, in order, each with where it starts: a
    /// unit's code leaves out those of the units inside it.
    decorator_spans: Vec<(usize, DocSpan)>,
    depth: usize,
    /// Whether the walk met nodes deeper than [`MAX_DEPTH`].
    too_deep: bool,
    /// For every literal with nodes of its own being walked, whether it is
    /// recorded: the nodes inside one belong to it, and a string can be a
    /// docstring or a type.
    units: Vec<bool>,
    /// Where the strings of the docstrings of the `def`s and `class`es walked
    /// so far start.
    docstrings: HashSet<usize>,
    /// The ranges of the type annotations the walk has not reached yet.
    annotations: HashSet<(usize, usize)>,
    /// The ranges of the subscripts of `Literal[...]` the walk has not
    /// reached yet.
    literal_types: HashSet<(usize, usize)>,
    /// The annotations and `Literal[...]` subscripts being walked, innermost
    /// last: the depth each starts at, and whether it is an annotation.
    contexts: Vec<(usize, bool)>,
    /// The plain parts of the f-strings being walked, by where they start:
    /// the value of the run they belong to, and whether they start it.
    runs: HashMap<usize, (u64, bool)>,
    /// Whether the source has carriage returns, which Python reads as line
    /// breaks: `\r\n` in a string's text is then `\n`. Looked up the first
    /// time a text is hashed.
    crlf: OnceCell<bool>,
    out: Vec<RawFunction>,
}

impl Functions<'_> {
    /// Where the code of `f` starts: its `def`, or the `async` before it.
    /// The node's own range starts at its first decorator.
    fn def_start(&self, f: &StmtFunctionDef) -> usize {
        self.keyword_start(&f.name, "def", f.is_async, f.range.start().to_usize())
    }

    /// Where a keyword that starts a unit (`def`, `class`) stands before the
    /// unit's name, or the `async` before a `def`; `fallback` without one.
    fn keyword_start(
        &self,
        name: &Identifier,
        keyword: &str,
        is_async: bool,
        fallback: usize,
    ) -> usize {
        let head = &self.source[..name.range.start().to_usize()];
        let Some(at) = head.rfind(keyword) else {
            return fallback;
        };
        let before = head[..at].trim_end();
        match is_async && before.ends_with("async") {
            true => before.len() - "async".len(),
            false => at,
        }
    }

    /// Where the code of `class` starts: its `class` keyword. The node's
    /// own range starts at its first decorator.
    fn class_start(&self, class: &StmtClassDef) -> usize {
        self.keyword_start(&class.name, "class", false, class.range.start().to_usize())
    }

    /// The frame of the unit `node` opens, if it is one: a `def` with code, a
    /// `class` with code (see [`has_code`]), or an assignment or type alias
    /// at module level or in a class body, whose value is no data. Its
    /// docstrings start at `docs` of [`Self::docstring_spans`].
    fn opening(&self, node: AnyNodeRef<'_>, docs: usize) -> Option<Frame> {
        let decos = self.decorator_spans.len();
        if self.min.is_none() && !matches!(node, AnyNodeRef::StmtFunctionDef(_)) {
            return None;
        }
        // Statements are units outside functions only.
        let statement = !self.openers.last().is_some_and(|&(_, def)| def);
        let range = node.range();
        let (start, end) = (range.start().to_usize(), range.end().to_usize());
        let (unit, name, start) = match node {
            AnyNodeRef::StmtFunctionDef(f) if !is_stub(&f.body) => {
                (UnitKind::Function, f.name.to_string(), self.def_start(f))
            }
            AnyNodeRef::StmtClassDef(class) if has_code(&class.body, MAX_DEPTH) => (
                UnitKind::Class,
                class.name.to_string(),
                self.class_start(class),
            ),
            AnyNodeRef::StmtAssign(assign) if statement && !is_data(&assign.value, MAX_DEPTH) => {
                let target = assign.targets.first()?;
                (UnitKind::Variable, target_name(target, MAX_DEPTH), start)
            }
            AnyNodeRef::StmtAnnAssign(assign)
                if statement
                    && !assign
                        .value
                        .as_deref()
                        .is_some_and(|value| is_data(value, MAX_DEPTH)) =>
            {
                let unit = match is_type_alias(&assign.annotation) {
                    true => UnitKind::Type,
                    false => UnitKind::Variable,
                };
                (unit, target_name(&assign.target, MAX_DEPTH), start)
            }
            AnyNodeRef::StmtTypeAlias(alias) if statement && !is_data(&alias.value, MAX_DEPTH) => {
                (UnitKind::Type, target_name(&alias.name, MAX_DEPTH), start)
            }
            _ => return None,
        };
        // Its docstrings, the decorators in it and comments only make a unit
        // smaller.
        if let Some(min) = self.min
            && (self.tokens_in(start, end) < min.tokens || self.lines_in(start, end) < min.lines)
        {
            return None;
        }
        Some(Frame {
            unit,
            name,
            start,
            end,
            docs,
            decos,
            kinds: Vec::new(),
            names: Vec::new(),
            literals: Vec::new(),
            open: None,
            run: None,
            decorators: Vec::new(),
            decorator: None,
        })
    }

    /// Note the docstring of a `def` or `class` body whose head starts at
    /// `head`.
    fn note_docstring(&mut self, body: &[Stmt], head: usize) {
        let Some(doc) = body_docstring(body) else {
            return;
        };
        let (start, end) = (doc.start().to_usize(), doc.end().to_usize());
        let first = self.line_index.location(start).line;
        // A docstring on the line of the `def` shares it with code.
        let lines = match first > self.line_index.location(head).line {
            true => self.line_index.location(end).line - first + 1,
            false => 0,
        };
        self.docstring_spans.push(DocSpan {
            tokens: self.tokens_in(start, end),
            lines,
        });
    }

    /// Code tokens starting inside `start..end`.
    fn tokens_in(&self, start: usize, end: usize) -> u32 {
        let first = self.code_tokens.partition_point(|&t| t < start);
        let last = self.code_tokens.partition_point(|&t| t < end);
        (last - first) as u32
    }

    /// The line span of `start..end`, in jscpd's `end - start` convention.
    fn lines_in(&self, start: usize, end: usize) -> u32 {
        let first = self.line_index.location(start).line;
        self.line_index.location(end).line.saturating_sub(first)
    }

    /// The size of a unit's code: its tokens and line span without the
    /// docstrings in it, its own and its methods', without the decorators
    /// of the units in it, and without comments. Its own decorators stand
    /// before its code.
    fn code_size(&self, frame: &Frame) -> CodeSize {
        let docs = self.docstring_spans[frame.docs..].iter();
        let decorators = self.decorator_spans[frame.decos..]
            .iter()
            .filter(|(at, _)| *at >= frame.start)
            .map(|(_, span)| span);
        let spans: Vec<&DocSpan> = docs.chain(decorators).collect();
        let tokens = spans.iter().map(|span| span.tokens).sum();
        let lines = spans.iter().map(|span| span.lines).sum();
        CodeSize {
            tokens: self
                .tokens_in(frame.start, frame.end)
                .saturating_sub(tokens),
            lines: self.lines_in(frame.start, frame.end).saturating_sub(lines),
        }
    }

    /// Whether the walk is inside a type annotation, and not inside a
    /// `Literal[...]` in it.
    fn in_annotation(&self) -> bool {
        self.contexts
            .last()
            .is_some_and(|&(_, annotation)| annotation)
    }

    /// The [`literal_hash`] of a text of type `kind`, with its line breaks
    /// read as Python reads them.
    fn text_hash(&self, kind: u16, text: &[u8]) -> u64 {
        let crlf = *self.crlf.get_or_init(|| self.source.contains('\r'));
        match crlf && text.contains(&b'\r') {
            true => literal_hash(kind, &normalize_newlines(text)),
            false => literal_hash(kind, text),
        }
    }

    fn bytes_hash(&self, kind: u16, value: &BytesLiteralValue) -> u64 {
        let mut parts = value.iter();
        match (parts.next(), parts.next()) {
            (Some(only), None) => self.text_hash(kind, only.as_slice()),
            _ => self.text_hash(kind, &value.bytes().collect::<Vec<u8>>()),
        }
    }

    /// The value of a literal expression of a `case` pattern.
    fn expr_hash(&self, expr: &Expr) -> Option<u64> {
        let kind = AnyNodeRef::from(expr).kind() as u16;
        match expr {
            Expr::NumberLiteral(number) => Some(number_hash(kind, &number.value)),
            Expr::StringLiteral(string) => {
                Some(self.text_hash(kind, string.value.to_str().as_bytes()))
            }
            Expr::BytesLiteral(bytes) => Some(self.bytes_hash(kind, &bytes.value)),
            _ => None,
        }
    }

    /// Note the runs of plain parts in `f`: `"a" "b" f"{x}"` has one, `"a"`
    /// then `"b"`, with the value of `"ab"`.
    fn note_runs(&mut self, f: &ExprFString) {
        if !f.value.is_implicit_concatenated() {
            return;
        }
        let parts: Vec<_> = f
            .value
            .iter()
            .map(|part| match part {
                FStringPartRef::Literal(plain) => Some(plain),
                FStringPartRef::FString(_) => None,
            })
            .collect();
        let kind = NodeKind::StringLiteral as u16;
        for run in parts.split(Option::is_none).filter(|run| !run.is_empty()) {
            let text: Vec<u8> = run.iter().flatten().flat_map(|p| p.value.bytes()).collect();
            let value = self.text_hash(kind, &text);
            for (i, part) in run.iter().flatten().enumerate() {
                self.runs.insert(part.start().to_usize(), (value, i == 0));
            }
        }
    }

    /// What `node`, of type `kind`, adds to the literals of the functions
    /// around it. Without such a function (`hash` false) it only keeps the
    /// state of the walk.
    fn literal(&mut self, node: AnyNodeRef<'_>, kind: u16, hash: bool) -> Option<Literal> {
        // The nodes inside a literal belong to it.
        let inside = !self.units.is_empty();
        if opens_literal(node) {
            let own = !inside
                && match node {
                    // A docstring is documentation, and a string in an
                    // annotation is a type.
                    AnyNodeRef::ExprStringLiteral(string) => {
                        !self.docstrings.contains(&string.start().to_usize())
                            && !self.in_annotation()
                    }
                    _ => true,
                };
            self.units.push(own);
            if !(own && hash) {
                return None;
            }
            let value = match node {
                AnyNodeRef::ExprStringLiteral(string) => {
                    self.text_hash(kind, string.value.to_str().as_bytes())
                }
                AnyNodeRef::ExprBytesLiteral(bytes) => self.bytes_hash(kind, &bytes.value),
                AnyNodeRef::PatternMatchValue(pattern) => self.expr_hash(&pattern.value)?,
                _ => return None,
            };
            return Some(Literal::Open(value));
        }
        if inside || !hash {
            return None;
        }
        match node {
            AnyNodeRef::StringLiteral(part) => {
                let own = self.text_hash(kind, part.value.as_bytes());
                let run = match self.runs.is_empty() {
                    true => None,
                    false => self.runs.remove(&part.start().to_usize()),
                };
                Some(match run {
                    Some((run, true)) => Literal::Run(run),
                    Some((_, false)) => Literal::RunPart(own),
                    None => Literal::Leaf(own),
                })
            }
            AnyNodeRef::InterpolatedStringLiteralElement(text) => {
                Some(Literal::Leaf(self.text_hash(kind, text.value.as_bytes())))
            }
            AnyNodeRef::ExprNumberLiteral(number) => {
                Some(Literal::Leaf(number_hash(kind, &number.value)))
            }
            AnyNodeRef::ExprBooleanLiteral(boolean) => Some(Literal::Leaf(literal_hash(
                kind,
                &[u8::from(boolean.value)],
            ))),
            AnyNodeRef::ExprNoneLiteral(_) | AnyNodeRef::ExprEllipsisLiteral(_) => {
                Some(Literal::Leaf(literal_hash(kind, &[])))
            }
            AnyNodeRef::PatternMatchSingleton(pattern) => {
                let value = match pattern.value {
                    Singleton::None => 0,
                    Singleton::True => 1,
                    Singleton::False => 2,
                };
                Some(Literal::Leaf(literal_hash(kind, &[value])))
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
        let docs = self.docstring_spans.len();
        match node {
            AnyNodeRef::StmtFunctionDef(f) => {
                self.docstrings.extend(docstring_start(&f.body));
                self.note_docstring(&f.body, self.def_start(f));
                for annotation in annotations_of(f) {
                    let range = annotation.range();
                    self.annotations
                        .insert((range.start().to_usize(), range.end().to_usize()));
                }
            }
            AnyNodeRef::StmtClassDef(class) => {
                self.docstrings.extend(docstring_start(&class.body));
                self.note_docstring(&class.body, self.class_start(class));
            }
            AnyNodeRef::StmtAnnAssign(assign) => {
                let range = assign.annotation.range();
                self.annotations
                    .insert((range.start().to_usize(), range.end().to_usize()));
            }
            _ => {}
        }
        if is_opener(node) {
            let frame = match self.frames.len() < MAX_OPEN_FUNCTIONS {
                true => self.opening(node, docs),
                false => None,
            };
            let def = matches!(node, AnyNodeRef::StmtFunctionDef(_));
            self.openers.push((frame.is_some(), def));
            self.frames.extend(frame);
        }
        if !self.annotations.is_empty() || !self.literal_types.is_empty() {
            let range = (node.start().to_usize(), node.end().to_usize());
            if self.annotations.remove(&range) {
                self.contexts.push((self.depth, true));
            } else if self.literal_types.remove(&range) {
                self.contexts.push((self.depth, false));
            }
        }
        // Literals outside every function would be thrown away.
        let hash = !self.frames.is_empty();
        match node {
            AnyNodeRef::ExprSubscript(subscript)
                if self.in_annotation() && is_literal_type(&subscript.value) =>
            {
                let slice = subscript.slice.range();
                self.literal_types
                    .insert((slice.start().to_usize(), slice.end().to_usize()));
            }
            AnyNodeRef::ExprFString(f) if hash => self.note_runs(f),
            _ => {}
        }
        let start = node.start().to_usize();
        let kind = node.kind() as u16;
        let called = match node {
            AnyNodeRef::ExprCall(call) => called_attribute(&call.func),
            _ => None,
        };
        let literal = self.literal(node, kind, hash);
        let decorator = match node {
            AnyNodeRef::Decorator(decorator) => {
                let (from, to) = (start, decorator.end().to_usize());
                let lines =
                    self.line_index.location(to).line - self.line_index.location(from).line + 1;
                let tokens = self.tokens_in(from, to);
                self.decorator_spans.push((from, DocSpan { tokens, lines }));
                Some(decorator_hash(&decorator_name(decorator)))
            }
            _ => None,
        };
        for frame in &mut self.frames {
            frame.kinds.push(kind);
            let at = (frame.kinds.len() - 1) as u32;
            if let Some(name) = decorator {
                frame.decorator = Some(frame.decorators.len());
                frame.decorators.push(DecoratorLeaf { at, len: 1, name });
            }
            if let Some(hash) = called {
                frame.names.push(RoleName { after: at, hash });
            }
            let leaf = LiteralLeaf {
                at,
                len: 1,
                value: 0,
            };
            match literal {
                Some(Literal::Leaf(value)) => frame.literals.push(LiteralLeaf { value, ..leaf }),
                Some(Literal::Open(value)) => {
                    frame.open = Some(frame.literals.len());
                    frame.literals.push(LiteralLeaf { value, ..leaf });
                }
                Some(Literal::Run(value)) => {
                    frame.run = Some(frame.literals.len());
                    frame.literals.push(LiteralLeaf { value, ..leaf });
                }
                Some(Literal::RunPart(value)) => match frame.run {
                    Some(index) if frame.literals[index].at + frame.literals[index].len == at => {
                        frame.literals[index].len += 1;
                    }
                    _ => frame.literals.push(LiteralLeaf { value, ..leaf }),
                },
                None => {}
            }
        }
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, node: AnyNodeRef<'a>) {
        let depth = self.depth;
        self.depth -= 1;
        if depth > MAX_DEPTH {
            return;
        }
        if self.contexts.last().is_some_and(|&(at, _)| at == depth) {
            self.contexts.pop();
        }
        if matches!(node, AnyNodeRef::Decorator(_)) {
            // The decorator is its own node and its expression.
            for frame in &mut self.frames {
                if let Some(index) = frame.decorator.take() {
                    let decorator = &mut frame.decorators[index];
                    decorator.len = frame.kinds.len() as u32 - decorator.at;
                }
            }
            return;
        }
        if opens_literal(node) {
            // The literal is its own node and the nodes walked under it.
            if self.units.pop() == Some(true) {
                for frame in &mut self.frames {
                    if let Some(index) = frame.open.take() {
                        let literal = &mut frame.literals[index];
                        literal.len = frame.kinds.len() as u32 - literal.at;
                    }
                }
            }
            return;
        }
        if !is_opener(node) {
            return;
        }
        if !self.openers.pop().is_some_and(|(opened, _)| opened) {
            return;
        }
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let code_size = self.code_size(&frame);
        if self
            .min
            .is_some_and(|min| code_size.tokens < min.tokens || code_size.lines < min.lines)
        {
            return;
        }
        let start = self.line_index.location(frame.start);
        self.out.push(RawFunction {
            grammar: "python",
            unit: frame.unit,
            name: frame.name,
            head: start.clone(),
            start,
            end: self.line_index.location(frame.end),
            kinds: frame.kinds,
            names: frame.names,
            literals: frame.literals,
            decorators: frame.decorators,
            code_size: Some(code_size),
        });
    }
}

/// Whether `node` is of a type that can open a unit; [`Functions::opening`]
/// decides whether it does.
fn is_opener(node: AnyNodeRef<'_>) -> bool {
    matches!(
        node,
        AnyNodeRef::StmtFunctionDef(_)
            | AnyNodeRef::StmtClassDef(_)
            | AnyNodeRef::StmtAssign(_)
            | AnyNodeRef::StmtAnnAssign(_)
            | AnyNodeRef::StmtTypeAlias(_)
    )
}

/// The name a decorator is known by: the name or attribute it calls, past
/// any call or subscript of it, as `get` in `@app.get("/users")`, `retry`
/// in `@retry(times=3)(log)` and `routes` in `@routes["get"]`; for any
/// other expression, such as a lambda, the kind of its node.
fn decorator_name(decorator: &Decorator) -> String {
    let mut expr = &decorator.expression;
    loop {
        match expr {
            Expr::Call(call) => expr = &call.func,
            Expr::Subscript(subscript) => expr = &subscript.value,
            other => {
                return match name_of(other) {
                    Some(name) => name.to_string(),
                    None => format!("<{:?}>", AnyNodeRef::from(other).kind()),
                };
            }
        }
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
    use super::super::{
        extract_embedded_functions, extract_embedded_units, extract_functions, extract_units,
        signatures, supports_functions,
    };
    use crate::{
        CodeSize, SignaturePolicy, SimilarityDecorators, SimilarityLiterals, UnitKind, bag_jaccard,
    };

    /// No size limit: every unit, however small.
    const ANY_SIZE: CodeSize = CodeSize {
        tokens: 0,
        lines: 0,
    };

    /// The values of the literals of the first function of `src`.
    fn values_of(src: &str) -> Vec<u64> {
        let f = extract_functions(src, "python").remove(0);
        f.literals.iter().map(|l| l.value).collect()
    }

    /// The similarity of the first units of `a` and `b` under `literals`.
    fn score(a: &str, b: &str, literals: SimilarityLiterals) -> f32 {
        let policy = SignaturePolicy {
            literals,
            ..SignaturePolicy::default()
        };
        score_with(a, b, policy)
    }

    /// The default policy with the decorators of `mode`.
    fn decorators(mode: SimilarityDecorators) -> SignaturePolicy {
        SignaturePolicy {
            decorators: mode,
            ..SignaturePolicy::default()
        }
    }

    /// The signature of the first unit of `src` under `policy`, whatever
    /// its size.
    fn signature(src: &str, policy: SignaturePolicy) -> crate::FunctionSig {
        use cpd_tokenizer::tokenizer::{Mode, TokenizeOptions, tokenize_to_detection};
        let options = TokenizeOptions::new(Mode::Mild);
        let spans: Vec<_> = tokenize_to_detection("python", src, &options)
            .iter()
            .map(|t| (t.start.clone(), t.end.clone()))
            .collect();
        signatures(extract_units(src, "python", ANY_SIZE), &spans, policy).remove(0)
    }

    /// The similarity of the first units of `a` and `b` under `policy`.
    fn score_with(a: &str, b: &str, policy: SignaturePolicy) -> f32 {
        bag_jaccard(
            &signature(a, policy).shingles,
            &signature(b, policy).shingles,
        )
    }

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
    fn decorators_count_as_the_mode_says() {
        use SimilarityDecorators::*;
        let body = "def f(a):\n    total = a.run(1)\n    return total + a.size\n";
        let plain = body.to_string();
        let routed = format!("@app.get(\"/users\")\n{body}");
        let more_args = format!("@app.get(\"/orders\", status_code=201)\n{body}");
        let posted = format!("@app.post(\"/users\")\n{body}");
        let score = |a: &str, b: &str, mode| score_with(a, b, decorators(mode));
        assert_eq!(score(&plain, &routed, Omit), 1.0);
        assert_eq!(score(&routed, &more_args, Omit), 1.0);
        assert!(score(&plain, &routed, Names) < 1.0, "the decorator counts");
        assert_eq!(
            score(&routed, &more_args, Names),
            1.0,
            "by its name, without its arguments"
        );
        assert!(score(&routed, &posted, Names) < 1.0, "get is no post");
        assert!(
            score(&routed, &more_args, Full) < 1.0,
            "its arguments count"
        );
        assert!(
            score(&routed, &posted, Full) < 1.0,
            "and its name, as in the names mode"
        );
        // The fragment starts at `def` and the size limits read the code
        // alone in every mode.
        let [omit, names, full] =
            [Omit, Names, Full].map(|mode| signature(&routed, decorators(mode)));
        for sig in [&names, &full] {
            assert_eq!(
                (sig.start.line, sig.token_count, sig.code_lines),
                (omit.start.line, omit.token_count, omit.code_lines)
            );
        }
        assert_eq!(omit.start.line, 2);
    }

    #[test]
    fn the_decorators_of_methods_count_in_their_class_as_the_mode_says() {
        use SimilarityDecorators::*;
        let class = |decorator: &str| {
            format!(
                "class Cart:\n    def __init__(self, items):\n        self.items = items\n\n    {decorator}def total(self):\n        return sum(item.price for item in self.items)\n"
            )
        };
        let (plain, property) = (class(""), class("@property\n    "));
        assert_eq!(score_with(&plain, &property, decorators(Omit)), 1.0);
        assert!(score_with(&plain, &property, decorators(Names)) < 1.0);
        let size = |src: &str| extract_units(src, "python", ANY_SIZE).remove(0).code_size;
        assert_eq!(
            size(&plain),
            size(&property),
            "a method's decorator adds nothing to the size of its class"
        );
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
    fn case_patterns_hold_literals() {
        let handler = |pattern: &str| {
            format!(
                "def handle(flag, store):\n    match flag:\n        case {pattern}:\n            store.enable(flag)\n        case _:\n            store.disable(flag)\n    return store.state()\n"
            )
        };
        let [yes, no, none] = ["True", "False", "None"].map(|p| values_of(&handler(p)));
        assert_eq!(yes.len(), 1, "case True: is a literal");
        assert_ne!(yes, no);
        assert_ne!(yes, none);
        let zero = extract_functions(&handler("0"), "python").remove(0);
        assert_eq!(
            zero.literals.len(),
            1,
            "a value pattern and its number are one literal"
        );
        assert_eq!(zero.literals[0].len, 2);
        let (a, b) = (handler("True"), handler("False"));
        assert!(score(&a, &b, SimilarityLiterals::Values) < 1.0);
        let (a, b) = (handler("None"), handler("0"));
        assert_eq!(score(&a, &b, SimilarityLiterals::Generic), 1.0);
        assert_eq!(score(&a, &b, SimilarityLiterals::Omit), 1.0);
    }

    #[test]
    fn strings_in_annotations_are_types_and_literal_values_are_literals() {
        let typed = |model: &str| {
            format!(
                "def load(session, keys: List[\"{model}\"], mode: Literal[\"fast\"]) -> \"Result\":\n    cache: \"Cache\" = session.cache\n    return session.get(keys, mode)\n"
            )
        };
        let f = extract_functions(&typed("User"), "python").remove(0);
        assert_eq!(
            f.literals.len(),
            1,
            "only the value in Literal[...] is a literal"
        );
        assert_eq!(values_of(&typed("User")), values_of(&typed("Order")));
        let (user, order) = (typed("User"), typed("Order"));
        assert_eq!(score(&user, &order, SimilarityLiterals::Values), 1.0);
    }

    #[test]
    fn crlf_line_ends_do_not_change_the_value_of_a_string() {
        let lf = "def query(db):\n    sql = \"\"\"\n        SELECT id\n        FROM users\n    \"\"\"\n    note = f\"\"\"rows:\n{db.count()}\"\"\"\n    return db.run(sql, b\"\"\"\nraw\"\"\", note)\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(values_of(lf), values_of(&crlf));
        assert_eq!(
            values_of(lf).len(),
            3,
            "the string, the f-string's text and the bytes"
        );
    }

    #[test]
    fn a_docstring_in_parentheses_is_documentation() {
        let src = "def f(x):\n    (\"\"\"Add one.\"\"\"\n    )\n    return x + 1\n";
        assert_eq!(values_of(src).len(), 1, "only the 1");
        let joined = "def f(x):\n    (\"Add \" \"one.\")\n    return x + 1\n";
        assert_eq!(values_of(joined).len(), 1);
    }

    #[test]
    fn big_ints_are_compared_by_value() {
        let value = |number: &str| values_of(&format!("def f():\n    return {number}\n"));
        let big = value("18446744073709551616");
        for same in [
            "0x10000000000000000",
            "0X1_0000_0000_0000_0000",
            "0o2000000000000000000000",
            "18_446_744_073_709_551_616",
        ] {
            assert_eq!(value(same), big, "{same}");
        }
        assert_ne!(value("18446744073709551617"), big);
        assert_eq!(value("0xFFFFFFFFFFFFFFFFFF"), value("0xffffffffffffffffff"));
        assert_ne!(value("16"), value("0x10000000000000000"));
    }

    #[test]
    fn plain_parts_of_an_fstring_that_follow_each_other_are_one_literal() {
        let split = "def check(value):\n    if value < 0:\n        raise ValueError(\"Invalid configuration: \" \"the value must be positive, \" f\"got {value}\")\n    return value\n";
        let joined = "def check(value):\n    if value < 0:\n        raise ValueError(\"Invalid configuration: the value must be positive, \" f\"got {value}\")\n    return value\n";
        let f = extract_functions(split, "python").remove(0);
        let run = f
            .literals
            .iter()
            .find(|l| l.len == 2)
            .expect("the two plain parts are one literal");
        assert_eq!(
            Some(&run.value),
            values_of(joined).iter().find(|&&v| v == run.value)
        );
        assert_eq!(score(split, joined, SimilarityLiterals::Generic), 1.0);
    }

    const MODELS: &str = "\
import re
from typing import Literal, TypeAlias

Handler: TypeAlias = Callable[[Request], Response]
type Pair[T] = tuple[T, T]
PATTERN = re.compile(r\"^[a-z]+$\")
first, second = make_pair()
VERSION = \"1.0\"
LEVELS = {\"debug\": 10, \"info\": 20, \"error\": -1}
__all__ = [\"User\", \"Pair\"]
Mode = Literal[\"fast\", \"slow\"]
Flag: TypeAlias = Literal[True]


@dataclass(frozen=True)
class User:
    \"\"\"A user of the shop.\"\"\"

    name: str
    role: Literal[\"admin\", \"user\"] = \"user\"
    tags: list[str] = field(default_factory=list)

    def label(self) -> str:
        cache = dict(self.extra)
        return f\"{self.name} ({self.role})\"
";

    /// The kind, name and first line of every unit of `src`.
    fn units_of(src: &str) -> Vec<(UnitKind, String, u32)> {
        extract_units(src, "python", ANY_SIZE)
            .into_iter()
            .map(|u| (u.unit, u.name, u.start.line))
            .collect()
    }

    #[test]
    fn classes_variables_and_type_aliases_are_units() {
        use UnitKind::*;
        let unit = |kind, name: &str, line| (kind, name.to_string(), line);
        assert_eq!(
            units_of(MODELS),
            vec![
                unit(Type, "Handler", 4),
                unit(Type, "Pair", 5),
                unit(Variable, "PATTERN", 6),
                unit(Variable, "first, second", 7),
                unit(Class, "User", 16),
                unit(Variable, "name", 19),
                unit(Variable, "tags", 21),
                unit(Function, "label", 23),
            ],
            "data, `Literal[...]` included, is no unit, and an assignment in a \
             function is part of it"
        );
        let functions = extract_functions(MODELS, "python");
        assert_eq!(functions.len(), 1, "extract_functions keeps to functions");
        let label = extract_units(MODELS, "python", ANY_SIZE).remove(7);
        assert_eq!(
            functions[0], label,
            "the units around a function do not change it"
        );
    }

    #[test]
    fn a_class_starts_at_class_and_its_docstring_is_no_code() {
        let units = extract_units(MODELS, "python", ANY_SIZE);
        let user = &units[4];
        assert!(MODELS[user.start.offset as usize..].starts_with("class User:"));
        assert_eq!(user.end.line, 25, "the class ends with its last method");
        let plain = MODELS.replace("@dataclass(frozen=True)\n", "");
        let plain = &extract_units(&plain, "python", ANY_SIZE)[4];
        assert_eq!(plain.code_size, user.code_size, "its decorator is no code");
        let documented = MODELS.replace(
            "\"\"\"A user of the shop.\"\"\"",
            "\"\"\"A user of the shop.\n\n    Users sign in with a name and get a role.\n    \"\"\"",
        );
        let documented = &extract_units(&documented, "python", ANY_SIZE)[4];
        assert_eq!(documented.code_size, user.code_size);
        assert_ne!(documented.end.line, user.end.line);
    }

    #[test]
    fn classes_that_only_declare_are_no_units() {
        let src = "\
class Store(Protocol):
    def load(self, key: str) -> bytes: ...


class NotFound(LookupError):
    \"\"\"No value under the key.\"\"\"


class Empty: ...


class Base:
    pass


class Color(Enum):
    RED = 1
    GREEN = auto()


class Point(TypedDict, total=False):
    x: Required[int]
    y: int = 0


class Hero(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    team = Relationship(back_populates=\"heroes\")


class Repository(ABC):
    @abstractmethod
    def get(self, key):
        \"\"\"The value under `key`.\"\"\"

    @abstractmethod
    def put(self, key, value):
        raise NotImplementedError(key)

    class Meta:
        ordering = [\"key\"]


class Cache(Repository):
    def get(self, key):
        return self.store.get(key)
";
        let classes: Vec<String> = units_of(src)
            .into_iter()
            .filter(|u| u.0 == UnitKind::Class)
            .map(|u| u.1)
            .collect();
        assert_eq!(
            classes,
            vec!["Cache"],
            "fields, stubs, `pass`, abstract methods and nested declarations \
             only declare; a method with code makes a class"
        );
    }

    #[test]
    fn a_class_in_a_function_has_fields_of_its_own() {
        use UnitKind::*;
        let src = "\
def build(store):
    class Model(Base):
        table = store.table(\"users\")

        def save(self):
            return store.save(self)

    return Model
";
        let unit = |kind, name: &str, line| (kind, name.to_string(), line);
        assert_eq!(
            units_of(src),
            vec![
                unit(Function, "build", 1),
                unit(Class, "Model", 2),
                unit(Variable, "table", 3),
                unit(Function, "save", 5),
            ]
        );
    }

    #[test]
    fn units_nested_past_the_cap_get_no_unit_of_their_own() {
        let mut src = String::new();
        for depth in 0..20 {
            src.push_str(&format!("{}class C{depth}:\n", "    ".repeat(depth)));
        }
        let indent = "    ".repeat(20);
        src.push_str(&format!("{indent}def f(a):\n{indent}    a.run()\n"));
        let units = extract_units(&src, "python", ANY_SIZE);
        assert_eq!(units.len(), super::MAX_OPEN_FUNCTIONS);
        assert!(units.iter().all(|u| u.unit == UnitKind::Class));
        let functions = extract_functions(&src, "python");
        assert_eq!(
            functions.len(),
            1,
            "the classes around a function do not count for extract_functions"
        );
    }

    #[test]
    fn code_blocks_of_markdown_hold_units() {
        let md = "# Models\n\n```python\nclass User(Base):\n    name = Column(String)\n\n    def greet(self):\n        return self.name.title()\n```\n";
        let units: Vec<(String, UnitKind, u32)> = extract_embedded_units(md, "markdown", ANY_SIZE)
            .into_iter()
            .map(|(format, u)| (format, u.unit, u.start.line))
            .collect();
        assert_eq!(
            units,
            vec![
                ("python".to_string(), UnitKind::Class, 4),
                ("python".to_string(), UnitKind::Variable, 5),
                ("python".to_string(), UnitKind::Function, 7),
            ]
        );
        let functions = extract_embedded_functions(md, "markdown");
        assert_eq!(functions.len(), 1, "the method alone");
    }

    #[test]
    fn tables_and_arithmetic_are_data() {
        let src = "\
urlpatterns = [
    path(\"\", views.index, name=\"index\"),
    path(\"<int:pk>/\", views.detail, name=\"detail\"),
]
COMMANDS = {\"add\": commands.add, \"drop\": commands.drop}
TIMEOUT = 60 * 60
SIZES = frozenset({1, 2})
app = FastAPI(title=\"Shop\", version=\"1.0\")
";
        let names: Vec<String> = units_of(src).into_iter().map(|u| u.1).collect();
        assert_eq!(
            names,
            vec!["SIZES", "app"],
            "lists, dicts and arithmetic are data; calls are code"
        );
    }

    #[test]
    fn the_docstrings_of_methods_are_no_code_of_their_class() {
        let class = |docs: bool| {
            let doc = |indent: &str, text: &str| match docs {
                true => format!("{indent}\"\"\"{text}\n\n{indent}More words.\n{indent}\"\"\"\n"),
                false => String::new(),
            };
            format!(
                "class Vector:\n{}    def norm(self):\n{}        return self.x * self.x + self.y * self.y\n",
                doc("    ", "A vector."),
                doc("        ", "Its length squared."),
            )
        };
        let size = |src: &str| {
            extract_units(src, "python", ANY_SIZE)
                .into_iter()
                .find(|u| u.unit == UnitKind::Class)
                .and_then(|u| u.code_size)
                .unwrap()
        };
        assert_eq!(size(&class(true)), size(&class(false)));
    }

    #[test]
    fn units_too_small_for_the_limits_are_left_out() {
        let limits = |tokens, lines| super::super::CodeSize { tokens, lines };
        let names = |min| -> Vec<String> {
            extract_units(MODELS, "python", min)
                .into_iter()
                .map(|u| u.name)
                .collect()
        };
        assert_eq!(names(limits(0, 0)).len(), 8);
        assert_eq!(
            names(limits(30, 5)),
            vec!["User"],
            "the class alone has 30 tokens and 5 lines of code"
        );
        assert!(names(limits(1000, 0)).is_empty());
        assert_eq!(
            extract_functions(MODELS, "python").len(),
            1,
            "extract_functions has no limits"
        );
    }

    #[test]
    fn an_alias_annotated_in_quotes_is_a_type_alias() {
        let src = "Handler: \"TypeAlias\" = Callable[[Request], Response]\nOther: \"typing.TypeAlias\" = Union[int, str]\n";
        let kinds: Vec<UnitKind> = units_of(src).into_iter().map(|u| u.0).collect();
        assert_eq!(kinds, vec![UnitKind::Type, UnitKind::Type]);
    }

    #[test]
    fn names_and_data_stop_at_their_depth_budget() {
        use ruff_python_ast::Stmt;
        let parsed = ruff_python_parser::parse_module("[[[a]]] = load()\nX = - - - 1\n").unwrap();
        let body = &parsed.syntax().body;
        let (Stmt::Assign(target), Stmt::Assign(value)) = (&body[0], &body[1]) else {
            panic!("two assignments");
        };
        assert_eq!(super::target_name(&target.targets[0], 4), "a");
        assert_eq!(super::target_name(&target.targets[0], 3), "<assignment>");
        assert!(super::is_data(&value.value, 4));
        assert!(!super::is_data(&value.value, 3), "too deep to be data");
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
