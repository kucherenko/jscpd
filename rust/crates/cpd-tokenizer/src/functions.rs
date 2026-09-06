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
    if parsed.panicked {
        return Vec::new();
    }
    let line_index = LineIndex::new(source.as_bytes());
    let mut extractor = Extractor {
        frames: Vec::new(),
        out: Vec::new(),
        pending_name: None,
        line_index: &line_index,
        len: source.len(),
    };
    extractor.visit_program(&parsed.program);
    extractor.out
}

struct Frame {
    name: String,
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
    line_index: &'i LineIndex,
    len: usize,
}

impl Extractor<'_> {
    fn open(&mut self, name: String, start: u32, end: u32) {
        self.frames.push(Frame {
            name,
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
        self.out.push(RawFunction {
            grammar: OxcExtractor.grammar(),
            name: frame.name,
            start: self.line_index.location(start),
            end: self.line_index.location(end),
            kinds: frame.kinds,
        });
    }
}

impl<'a> Visit<'a> for Extractor<'_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::VariableDeclarator(d) => {
                self.pending_name = d.id.get_identifier_name().map(|n| n.to_string());
            }
            AstKind::MethodDefinition(m) => {
                self.pending_name = m.key.static_name().map(|n| n.into_owned());
            }
            AstKind::PropertyDefinition(p) => {
                self.pending_name = p.key.static_name().map(|n| n.into_owned());
            }
            AstKind::ObjectProperty(p) => {
                self.pending_name = p.key.static_name().map(|n| n.into_owned());
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
            | AstKind::ObjectProperty(_) => self.pending_name = None,
            _ => {}
        }
        let _ = kind.span();
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
    fn renamed_copies_share_the_same_kind_sequence() {
        let a = extract_functions("function a(x) { return x + 1; }", "javascript");
        let b = extract_functions("function b(y) { return y + 1; }", "javascript");
        assert_eq!(a[0].kinds, b[0].kinds);
    }
}
