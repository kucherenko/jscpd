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
//!
//! An extractor that finds function boundaries but no node sequence (it
//! returns empty `kinds` and says so through
//! [`FunctionExtractor::structural`]) serves `--semantic` only, which embeds
//! the function's code and needs nothing else.

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
    /// Whether `kinds` carries the syntax-tree node sequence that
    /// `--similarity` compares. An extractor that only finds where functions
    /// are returns empty `kinds` and serves `--semantic` alone.
    fn structural(&self) -> bool {
        true
    }
}

/// Registered extractors, consulted in order. Add new languages here.
pub static EXTRACTORS: &[&dyn FunctionExtractor] =
    &[&OxcExtractor, &RustExtractor, &PythonExtractor];

/// The extractor serving `format`, if any.
pub fn extractor_for(format: &str) -> Option<&'static dyn FunctionExtractor> {
    EXTRACTORS
        .iter()
        .copied()
        .find(|e| e.formats().contains(&format))
}

/// Formats whose functions `--similarity` can compare: those with a
/// structural extractor.
pub fn supports_functions(format: &str) -> bool {
    extractor_for(format).is_some_and(|e| e.structural())
}

/// Every format `--similarity` supports, for messages and docs.
pub fn supported_function_formats() -> Vec<&'static str> {
    EXTRACTORS
        .iter()
        .filter(|e| e.structural())
        .flat_map(|e| e.formats().iter().copied())
        .collect()
}

/// Extract every function of a source. Returns an empty vector for formats
/// without an extractor and for sources that fail to parse.
pub fn extract_functions(source: &str, format: &str) -> Vec<RawFunction> {
    match extractor_for(format) {
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
    line_index: &'i LineIndex,
    len: usize,
}

impl Extractor<'_> {
    fn open(&mut self, name: String, start: u32, end: u32) {
        let head = self
            .pending_head
            .take()
            .filter(|&(_, value)| value == start)
            .map_or(start, |(head, _)| head);
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
            AstKind::Function(f) => {
                let name =
                    f.id.as_ref()
                        .map(|id| id.name.to_string())
                        .or_else(|| self.pending_name.take())
                        .unwrap_or_else(|| "<anonymous>".to_string());
                let span = f.span;
                self.open(name, span.start, span.end);
            }
            AstKind::ArrowFunctionExpression(a) => {
                let name = self
                    .pending_name
                    .take()
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
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => self.close(),
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

/// Python through the ruff parser: every `def` and `async def`, methods and
/// nested functions included. The span starts at `def` (or `async`), so
/// decorators stay out as Rust attributes do. Finds boundaries only (see
/// [`FunctionExtractor::structural`]).
pub struct PythonExtractor;

impl FunctionExtractor for PythonExtractor {
    fn grammar(&self) -> &'static str {
        "python"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["python"]
    }

    fn extract(&self, source: &str, _format: &str) -> Vec<RawFunction> {
        use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
        let Ok(parsed) = ruff_python_parser::parse_module(source) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.as_bytes());
        let mut visitor = PythonFunctions {
            source,
            line_index: &line_index,
            out: Vec::new(),
        };
        visitor.visit_body(&parsed.syntax().body);
        visitor.out.sort_by_key(|f| f.start.offset);
        visitor.out
    }

    fn structural(&self) -> bool {
        false
    }
}

struct PythonFunctions<'s> {
    source: &'s str,
    line_index: &'s LineIndex,
    out: Vec<RawFunction>,
}

impl<'a> ruff_python_ast::visitor::source_order::SourceOrderVisitor<'a> for PythonFunctions<'_> {
    fn visit_stmt(&mut self, stmt: &'a ruff_python_ast::Stmt) {
        if let ruff_python_ast::Stmt::FunctionDef(f) = stmt {
            let name_start = f.name.range.start().to_usize();
            let end = f.range.end().to_usize();
            // `def` is the last keyword before the name; `async` precedes it.
            let head = &self.source[..name_start];
            if let Some(def) = head.rfind("def") {
                let before = head[..def].trim_end();
                let start = match f.is_async && before.ends_with("async") {
                    true => before.len() - "async".len(),
                    false => def,
                };
                self.out.push(RawFunction {
                    grammar: "python",
                    name: f.name.to_string(),
                    start: self.line_index.location(start),
                    end: self.line_index.location(end),
                    head: self.line_index.location(start),
                    kinds: Vec::new(),
                });
            }
        }
        ruff_python_ast::visitor::source_order::walk_stmt(self, stmt);
    }
}

/// Rust by scanning the source: every `fn` with a body — free functions,
/// methods, trait methods with a default body, and functions nested in
/// them. Finds boundaries only (see [`FunctionExtractor::structural`]).
///
/// A scan, not a parse: it knows Rust's comments (nested block comments
/// included), strings, raw strings and character literals — so a brace or a
/// `fn` inside one of them is not code, and a lifetime `'a` is not the start
/// of a character — and that is all it needs to find where a function's
/// item starts (its first keyword, after doc comments and attributes) and
/// where its body's braces close. `fn` followed by anything but a name (a
/// function pointer type, a macro fragment) opens nothing.
pub struct RustExtractor;

impl FunctionExtractor for RustExtractor {
    fn grammar(&self) -> &'static str {
        "rust"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["rust"]
    }

    fn extract(&self, source: &str, _format: &str) -> Vec<RawFunction> {
        let line_index = LineIndex::new(source.as_bytes());
        let mut out: Vec<RawFunction> = scan_rust_functions(source)
            .into_iter()
            .map(|(name, start, end)| RawFunction {
                grammar: "rust",
                name,
                start: line_index.location(start),
                end: line_index.location(end),
                head: line_index.location(start),
                kinds: Vec::new(),
            })
            .collect();
        out.sort_by_key(|f| f.start.offset);
        out
    }

    fn structural(&self) -> bool {
        false
    }
}

/// `(name, item start, end after the closing brace)` of every Rust function
/// with a body.
fn scan_rust_functions(source: &str) -> Vec<(String, usize, usize)> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    // Functions whose body is open: (name, item start, depth inside body).
    let mut open: Vec<(String, usize, usize)> = Vec::new();
    let mut depth = 0usize;
    // Start of the item being read: the first code byte after the last
    // `;`, `{`, `}` or attribute `]`.
    let mut item_start: Option<usize> = None;
    // A `fn NAME` whose body has not started: (name, item start).
    let mut pending: Option<(String, usize)> = None;
    // Parentheses and brackets inside a pending signature.
    let mut nesting = 0usize;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if let Some(next) = skip_rust_trivia(b, i) {
            i = next;
            continue;
        }
        if item_start.is_none() {
            item_start = Some(i);
        }
        if let Some(next) = skip_rust_literal(b, i) {
            i = next;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            let word_end = word_end(b, i);
            if &b[i..word_end] == b"fn" && pending.is_none() {
                let name_start = skip_space_and_trivia(b, word_end);
                let name_end = word_end_if_ident(b, name_start);
                if name_end > name_start {
                    let name = source[name_start..name_end].to_string();
                    pending = Some((name, item_start.unwrap_or(i)));
                    nesting = 0;
                    i = name_end;
                    continue;
                }
            }
            i = word_end;
            continue;
        }
        match c {
            b'(' | b'[' if pending.is_some() => nesting += 1,
            b')' | b']' if pending.is_some() => nesting = nesting.saturating_sub(1),
            b';' if pending.is_some() && nesting == 0 => {
                // A declaration without a body.
                pending = None;
                item_start = None;
            }
            b'{' => {
                depth += 1;
                if nesting == 0
                    && let Some((name, start)) = pending.take()
                {
                    open.push((name, start, depth));
                }
                if pending.is_none() {
                    item_start = None;
                }
            }
            b'}' => {
                if let Some((_, _, d)) = open.last()
                    && *d == depth
                {
                    let (name, start, _) = open.pop().unwrap_or_default();
                    out.push((name, start, i + 1));
                }
                depth = depth.saturating_sub(1);
                if pending.is_none() {
                    item_start = None;
                }
            }
            b';' | b']' if pending.is_none() => item_start = None,
            _ => {}
        }
        i += 1;
    }
    out
}

/// The end of a comment starting at `i`, if one does.
fn skip_rust_trivia(b: &[u8], i: usize) -> Option<usize> {
    if b[i] != b'/' {
        return None;
    }
    match b.get(i + 1) {
        Some(b'/') => Some(
            b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |p| i + p + 1),
        ),
        Some(b'*') => {
            // Block comments nest in Rust.
            let mut level = 0usize;
            let mut j = i;
            while j < b.len() {
                if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                    level += 1;
                    j += 2;
                } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                    level -= 1;
                    j += 2;
                    if level == 0 {
                        return Some(j);
                    }
                } else {
                    j += 1;
                }
            }
            Some(b.len())
        }
        _ => None,
    }
}

/// The end of a string, raw string, byte string or character literal
/// starting at `i`, if one does. A lifetime or label (`'a`) is not one.
fn skip_rust_literal(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    // Prefixes: b"..", r"..", br"..", c"..", r#".."#.
    while j < b.len() && matches!(b[j], b'b' | b'r' | b'c') && j - i < 2 {
        j += 1;
    }
    let raw = b[i..j].contains(&b'r');
    let mut hashes = 0;
    if raw {
        while b.get(j) == Some(&b'#') {
            hashes += 1;
            j += 1;
        }
    }
    match b.get(j) {
        Some(b'"') if j == i || raw || b[i..j].iter().all(|&p| p == b'b' || p == b'c') => {
            j += 1;
            while j < b.len() {
                if !raw && b[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if b[j] == b'"'
                    && b.len() >= j + 1 + hashes
                    && b[j + 1..j + 1 + hashes].iter().all(|&h| h == b'#')
                {
                    return Some(j + 1 + hashes);
                }
                j += 1;
            }
            Some(b.len())
        }
        Some(b'\'') if !raw && (j == i || b[i..j] == *b"b") => {
            // `'x'`, `'\n'`, `'\u{..}'`, `'é'`; anything else is a lifetime.
            let k = j + 1;
            if b.get(k) == Some(&b'\\') {
                let close = b[k..].iter().position(|&c| c == b'\'').map(|p| k + p);
                // Skip the escaped quote of `'\''`.
                let close = match close {
                    Some(p) if p == k + 1 => b[k + 2..]
                        .iter()
                        .position(|&c| c == b'\'')
                        .map(|q| k + 2 + q),
                    other => other,
                };
                return close.map(|p| p + 1);
            }
            let ch_len = utf8_len(b.get(k).copied()?);
            (b.get(k + ch_len) == Some(&b'\'')).then_some(k + ch_len + 1)
        }
        _ => None,
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

fn word_end(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] >= 0x80) {
        j += 1;
    }
    j
}

/// End of the identifier at `i` (a raw identifier `r#name` included), or
/// `i` when none starts there.
fn word_end_if_ident(b: &[u8], i: usize) -> usize {
    match b.get(i) {
        Some(c) if c.is_ascii_alphabetic() || *c == b'_' || *c >= 0x80 => {
            if b[i] == b'r' && b.get(i + 1) == Some(&b'#') {
                word_end(b, i + 2)
            } else {
                word_end(b, i)
            }
        }
        _ => i,
    }
}

fn skip_space_and_trivia(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        match (i < b.len()).then(|| skip_rust_trivia(b, i)).flatten() {
            Some(next) => i = next,
            None => return i,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "export function total(items) {\n  let sum = 0;\n  for (const it of items) { sum += it.price; }\n  return sum;\n}\nconst double = (x) => x * 2;\nclass Cart {\n  add(item) { this.items.push(item); }\n}\nconst obj = { run() { return 1; }, cb: function () { return 2; } };\n";

    #[test]
    fn extracts_declarations_arrows_methods_and_properties_with_names() {
        let fns = extract_functions(SRC, "javascript");
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["total", "double", "add", "run", "cb"]);
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

    /// Name, first line and last line of each function.
    fn spans(fns: &[RawFunction]) -> Vec<(&str, u32, u32)> {
        fns.iter()
            .map(|f| (f.name.as_str(), f.start.line, f.end.line))
            .collect()
    }

    #[test]
    fn rust_functions_methods_and_default_trait_methods_are_found() {
        let src = "use std::fmt;\n\n/// Adds.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nimpl Cart {\n    pub(crate) async fn total(&self) -> i64 {\n        fn cents(x: i64) -> i64 { x * 100 }\n        cents(self.sum)\n    }\n}\n\ntrait Named {\n    fn name(&self) -> String { String::new() }\n    fn id(&self) -> u32;\n}\n";
        let fns = extract_functions(src, "rust");
        assert_eq!(
            spans(&fns),
            vec![
                ("add", 4, 6),
                ("total", 9, 12),
                ("cents", 10, 10),
                ("name", 16, 16)
            ]
        );
        // The span starts at the visibility, after the doc comment, and ends
        // on the closing brace.
        let add = &fns[0];
        assert_eq!(
            &src[add.start.offset as usize..add.end.offset as usize],
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}"
        );
        assert!(
            fns.iter()
                .all(|f| f.grammar == "rust" && f.kinds.is_empty())
        );
    }

    #[test]
    fn rust_braces_and_fn_inside_literals_and_comments_are_not_code() {
        let src = r####"fn lifetimes<'a>(x: &'a str) -> &'a str { if x == "{" { x } else { "}" } }
fn chars() -> [char; 3] { ['{', '\'', '}'] }
fn raw() -> &'static str { r##"fn fake() { "# }"## }
/* outer /* fn nested() { */ still comment */
fn pointer(f: fn(u32) -> u32) -> u32 { f(1) }
macro_rules! make { ($n:ident) => { fn $n() {} }; }
fn last() {}
"####;
        let names: Vec<(String, u32)> = extract_functions(src, "rust")
            .into_iter()
            .map(|f| (f.name, f.end.line))
            .collect();
        let expected = [
            ("lifetimes", 1),
            ("chars", 2),
            ("raw", 3),
            ("pointer", 5),
            ("last", 7),
        ];
        assert_eq!(
            names,
            expected
                .iter()
                .map(|(n, l)| (n.to_string(), *l))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn python_functions_methods_and_nested_ones_start_at_def() {
        let src = "import os\n\n@cache\ndef load(path):\n    return open(path).read()\n\nclass Cart:\n    async def total(self):\n        def cents(x):\n            return x * 100\n        return cents(self.sum)\n";
        let fns = extract_functions(src, "python");
        assert_eq!(
            spans(&fns),
            vec![("load", 4, 5), ("total", 8, 11), ("cents", 9, 10)]
        );
        assert_eq!(&src[fns[0].start.offset as usize..][..8], "def load");
        assert_eq!(&src[fns[1].start.offset as usize..][..9], "async def");
        assert!(
            fns.iter()
                .all(|f| f.grammar == "python" && f.kinds.is_empty())
        );
        assert!(!supports_functions("python"));
        assert!(extract_functions("def broken(:\n", "python").is_empty());
    }

    #[test]
    fn rust_that_does_not_parse_still_yields_what_closes() {
        assert!(extract_functions("fn broken( {", "rust").is_empty());
        let fns = extract_functions("fn ok() { 1 }\nfn open() {", "rust");
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "ok");
    }

    #[test]
    fn similarity_only_claims_structural_extractors() {
        assert!(extractor_for("rust").is_some());
        assert!(
            !supports_functions("rust"),
            "the scan finds boundaries, not structure"
        );
        assert!(!supported_function_formats().contains(&"rust"));
    }

    #[test]
    fn registry_dispatches_by_format_and_tags_the_grammar() {
        assert_eq!(extractor_for("typescript").unwrap().grammar(), "oxc");
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
    fn renamed_copies_share_the_same_kind_sequence() {
        let a = extract_functions("function a(x) { return x + 1; }", "javascript");
        let b = extract_functions("function b(y) { return y + 1; }", "javascript");
        assert_eq!(a[0].kinds, b[0].kinds);
    }
}
