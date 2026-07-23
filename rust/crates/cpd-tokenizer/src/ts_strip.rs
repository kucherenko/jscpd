// cpd-tokenizer: collect byte spans of erasable TypeScript-only syntax.
//
// Used by the --cross-formats feature: when a TypeScript file shares a
// detection pool with JavaScript files, tokens inside these spans are dropped
// so the remaining token stream (and therefore its hashes) matches the
// equivalent plain-JavaScript source. Spans reference the ORIGINAL source, so
// token locations — and reported clone positions — are unaffected.
//
// Non-erasable TypeScript syntax with runtime semantics is left intact and
// simply will not cross-match: enum, non-declare namespace, parameter
// properties (`constructor(private x)`), `import x = require()`, `export =`.

use oxc_ast::ast::{
    Class, ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportNamedDeclaration,
    Function, ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, MethodDefinition,
    Program, PropertyDefinition, TSAsExpression, TSEnumDeclaration, TSInterfaceDeclaration,
    TSModuleDeclaration, TSNonNullExpression, TSSatisfiesExpression, TSThisParameter,
    TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeAssertion, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation, VariableDeclaration, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;

/// TypeScript-only modifier keywords that may appear in a class-member head
/// (between member start and key start) or before `class`. Tokens inside a
/// modifier zone are dropped only when their text is one of these words, so
/// `static`, `async`, `get`, `set` and decorators survive.
const TS_MODIFIER_WORDS: &[&str] = &[
    "public",
    "private",
    "protected",
    "readonly",
    "abstract",
    "override",
    "declare",
];

pub fn is_ts_modifier_word(word: &str) -> bool {
    TS_MODIFIER_WORDS.contains(&word)
}

/// Byte spans of erasable TypeScript syntax in one source file.
#[derive(Debug, Default)]
pub struct ErasableSpans {
    /// Sorted, merged `[start, end)` byte ranges removed wholesale.
    pub ranges: Vec<[u32; 2]>,
    /// Sorted `[start, end)` zones where tokens are dropped only if their
    /// text is a TypeScript modifier word (see [`is_ts_modifier_word`]).
    pub modifier_zones: Vec<[u32; 2]>,
}

impl ErasableSpans {
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && self.modifier_zones.is_empty()
    }

    /// True when the `[start, end)` byte range intersects an erased range.
    pub fn intersects_range(&self, start: u32, end: u32) -> bool {
        // ranges are sorted and merged; find the first range ending after `start`.
        let idx = self.ranges.partition_point(|[_, re]| *re <= start);
        self.ranges.get(idx).is_some_and(|[rs, _]| *rs < end)
    }

    /// True when a token at `[start, end)` with text `value` should be
    /// dropped because it is a TS modifier word inside a modifier zone.
    pub fn is_modifier_token(&self, start: u32, end: u32, value: &str) -> bool {
        if !is_ts_modifier_word(value) {
            return false;
        }
        let idx = self.modifier_zones.partition_point(|[_, ze]| *ze <= start);
        self.modifier_zones
            .get(idx)
            .is_some_and(|[zs, _]| *zs < end)
    }
}

/// Walk `program` and collect the byte spans of all erasable
/// TypeScript-only syntax.
pub fn collect_erasable_spans(program: &Program, source: &str) -> ErasableSpans {
    let mut collector = Collector {
        source,
        ranges: Vec::new(),
        modifier_zones: Vec::new(),
    };
    collector.visit_program(program);

    collector.ranges.sort_unstable();
    let mut merged: Vec<[u32; 2]> = Vec::with_capacity(collector.ranges.len());
    for range in collector.ranges {
        match merged.last_mut() {
            Some(last) if range[0] <= last[1] => last[1] = last[1].max(range[1]),
            _ => merged.push(range),
        }
    }
    collector.modifier_zones.sort_unstable();

    ErasableSpans {
        ranges: merged,
        modifier_zones: collector.modifier_zones,
    }
}

struct Collector<'s> {
    source: &'s str,
    ranges: Vec<[u32; 2]>,
    modifier_zones: Vec<[u32; 2]>,
}

impl Collector<'_> {
    fn push(&mut self, start: u32, end: u32) {
        if start < end {
            self.ranges.push([start, end]);
        }
    }

    /// Push the 1-byte span of `needle` if it occurs in `[start, end)`.
    /// Used for markers stored as bools in the AST (`?`, `!`).
    fn push_char_in(&mut self, needle: u8, start: u32, end: u32) {
        let (start, end) = (start as usize, end.min(self.source.len() as u32) as usize);
        if start >= end {
            return;
        }
        if let Some(pos) = self.source.as_bytes()[start..end]
            .iter()
            .position(|&b| b == needle)
        {
            let at = (start + pos) as u32;
            self.push(at, at + 1);
        }
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'a>) {
        // Span starts at the `:` token — dropping it removes the colon too.
        self.push(it.span.start, it.span.end);
    }

    fn visit_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'a>) {
        self.push(it.span.start, it.span.end);
    }

    fn visit_ts_type_parameter_instantiation(&mut self, it: &TSTypeParameterInstantiation<'a>) {
        self.push(it.span.start, it.span.end);
    }

    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'a>) {
        self.push(it.span.start, it.span.end);
    }

    fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'a>) {
        self.push(it.span.start, it.span.end);
    }

    fn visit_ts_as_expression(&mut self, it: &TSAsExpression<'a>) {
        // Drop the ` as T` tail, keep the expression.
        self.push(it.expression.span().end, it.span.end);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'a>) {
        self.push(it.expression.span().end, it.span.end);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_type_assertion(&mut self, it: &TSTypeAssertion<'a>) {
        // `<T>expr` — drop the leading `<T>`.
        self.push(it.span.start, it.expression.span().start);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_non_null_expression(&mut self, it: &TSNonNullExpression<'a>) {
        // The trailing `!`.
        self.push(it.expression.span().end, it.span.end);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_this_parameter(&mut self, it: &TSThisParameter<'a>) {
        // Drop `this: T` and, when present, the comma separating it from the
        // next parameter.
        let mut end = it.span.end;
        let bytes = self.source.as_bytes();
        let mut i = end as usize;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b',' {
            end = (i + 1) as u32;
        }
        self.push(it.span.start, end);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        if it.import_kind == ImportOrExportKind::Type {
            self.push(it.span.start, it.span.end);
            return;
        }
        if let Some(specifiers) = &it.specifiers {
            for specifier in specifiers {
                if let ImportDeclarationSpecifier::ImportSpecifier(s) = specifier {
                    if s.import_kind == ImportOrExportKind::Type {
                        // Leaves a stray `,` behind — documented limitation.
                        self.push(s.span.start, s.span.end);
                    }
                }
            }
        }
    }

    fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
        if it.export_kind == ImportOrExportKind::Type {
            self.push(it.span.start, it.span.end);
            return;
        }
        for specifier in &it.specifiers {
            if specifier.export_kind == ImportOrExportKind::Type {
                self.push(specifier.span.start, specifier.span.end);
            }
        }
        walk::walk_export_named_declaration(self, it);
    }

    fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'a>) {
        if matches!(
            it.declaration,
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        ) {
            self.push(it.span.start, it.span.end);
            return;
        }
        walk::walk_export_default_declaration(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        // Ambient declarations and overload signatures have no body and are
        // fully erasable. (Overload signatures of class methods are handled
        // in visit_method_definition, which owns the surrounding span.)
        if it.declare || it.body.is_none() {
            self.push(it.span.start, it.span.end);
            return;
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        if it.declare {
            self.push(it.span.start, it.span.end);
            return;
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if it.definite {
            // `let x!: T` — the `!` sits between the binding and the annotation.
            let bound = it
                .type_annotation
                .as_ref()
                .map(|t| t.span.start)
                .or_else(|| it.init.as_ref().map(|i| i.span().start))
                .unwrap_or(it.span.end);
            self.push_char_in(b'!', it.id.span().end, bound);
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        if it.declare {
            self.push(it.span.start, it.span.end);
            return;
        }
        if it.r#abstract {
            // `abstract` appears between the class span start (which includes
            // decorators and `export` is outside the span) and the `class`
            // keyword; word-filtering keeps everything else in the zone.
            let zone_end = it
                .id
                .as_ref()
                .map(|id| id.span.start)
                .unwrap_or(it.body.span.start);
            self.modifier_zones.push([it.span.start, zone_end]);
        }
        if let (Some(first), Some(last)) = (it.implements.first(), it.implements.last()) {
            // Scan back from the first heritage type for the `implements`
            // keyword and drop the whole clause.
            let clause_start = self.source[..first.span.start as usize]
                .rfind("implements")
                .map(|p| p as u32)
                .unwrap_or(first.span.start);
            self.push(clause_start, last.span.end);
        }
        walk::walk_class(self, it);
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        // Overload signatures and abstract methods have no body: erasable.
        if it.value.body.is_none() {
            self.push(it.span.start, it.span.end);
            return;
        }
        self.modifier_zones
            .push([it.span.start, it.key.span().start]);
        if it.optional {
            self.push_char_in(b'?', it.key.span().end, it.value.span.start);
        }
        walk::walk_method_definition(self, it);
    }

    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        if it.declare {
            self.push(it.span.start, it.span.end);
            return;
        }
        self.modifier_zones
            .push([it.span.start, it.key.span().start]);
        if it.optional || it.definite {
            let bound = it
                .type_annotation
                .as_ref()
                .map(|t| t.span.start)
                .or_else(|| it.value.as_ref().map(|v| v.span().start))
                .unwrap_or(it.span.end);
            let marker = if it.optional { b'?' } else { b'!' };
            self.push_char_in(marker, it.key.span().end, bound);
        }
        walk::walk_property_definition(self, it);
    }

    fn visit_formal_parameter(&mut self, it: &oxc_ast::ast::FormalParameter<'a>) {
        // Parameter properties (`constructor(private x)`) have runtime
        // semantics: keep the modifiers (and the `?`), but still walk so the
        // type annotation is stripped like everywhere else.
        let is_parameter_property = it.accessibility.is_some() || it.readonly || it.r#override;
        if !is_parameter_property && it.optional {
            let bound = it
                .type_annotation
                .as_ref()
                .map(|t| t.span.start)
                .or_else(|| it.initializer.as_ref().map(|i| i.span().start))
                .unwrap_or(it.span.end);
            self.push_char_in(b'?', it.pattern.span().end, bound);
        }
        walk::walk_formal_parameter(self, it);
    }

    fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
        // Non-declare enums generate runtime code: left intact (non-goal).
        if it.declare {
            self.push(it.span.start, it.span.end);
        }
    }

    fn visit_ts_module_declaration(&mut self, it: &TSModuleDeclaration<'a>) {
        if it.declare {
            self.push(it.span.start, it.span.end);
            return;
        }
        // Non-declare namespace: keep the header (runtime semantics) but
        // still strip annotations inside the body.
        walk::walk_ts_module_declaration(self, it);
    }
}

#[cfg(test)]
mod tests {
    use crate::tokenizer::{Mode, TokenizeOptions, tokenize_to_detection};

    /// Detection hash sequence with TS stripping enabled for typescript/tsx.
    fn ts_hashes(source: &str, format: &str) -> Vec<u64> {
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.strip_types_formats = ["typescript".to_string(), "tsx".to_string()]
            .into_iter()
            .collect();
        tokenize_to_detection(format, source, &opts)
            .into_iter()
            .map(|t| t.hash)
            .collect()
    }

    fn js_hashes(source: &str) -> Vec<u64> {
        let opts = TokenizeOptions::new(Mode::Mild);
        tokenize_to_detection("javascript", source, &opts)
            .into_iter()
            .map(|t| t.hash)
            .collect()
    }

    #[track_caller]
    fn assert_ts_matches_js(ts: &str, js: &str) {
        let ts_h = ts_hashes(ts, "typescript");
        let js_h = js_hashes(js);
        assert!(!js_h.is_empty(), "JS twin produced no tokens");
        assert_eq!(
            ts_h, js_h,
            "stripped TS must hash-match its JS twin\nTS: {ts}\nJS: {js}"
        );
    }

    #[test]
    fn strips_annotations_and_return_types() {
        assert_ts_matches_js(
            "function add(a: number, b: number): number { return a + b; }",
            "function add(a, b) { return a + b; }",
        );
    }

    #[test]
    fn strips_variable_annotations() {
        assert_ts_matches_js("const x: number = 5;", "const x = 5;");
    }

    #[test]
    fn strips_generics_declaration_and_instantiation() {
        assert_ts_matches_js(
            "function id<T>(x: T): T { return x; }\nconst y = id<number>(1);",
            "function id(x) { return x; }\nconst y = id(1);",
        );
    }

    #[test]
    fn strips_interface_and_type_alias() {
        assert_ts_matches_js(
            "interface P { x: number }\ntype Q = string;\nconst a = 1;",
            "const a = 1;",
        );
    }

    #[test]
    fn strips_as_and_satisfies_tails() {
        assert_ts_matches_js(
            "const a = foo as unknown as string; const b = bar satisfies object;",
            "const a = foo; const b = bar;",
        );
    }

    #[test]
    fn strips_non_null_and_definite_assignment() {
        assert_ts_matches_js(
            "let v!: number;\nconst w = obj!.prop!;",
            "let v;\nconst w = obj.prop;",
        );
    }

    #[test]
    fn strips_optional_markers() {
        assert_ts_matches_js(
            "function f(a?: number) { return a; }\nclass C { p?: string; m?() { return 1; } }",
            "function f(a) { return a; }\nclass C { p; m() { return 1; } }",
        );
    }

    #[test]
    fn strips_class_modifiers_and_implements() {
        assert_ts_matches_js(
            "abstract class C implements A, B { private readonly x = 1; protected m(): void {} }",
            "class C { x = 1; m() {} }",
        );
    }

    #[test]
    fn keeps_static_async_get_set_in_modifier_zones() {
        assert_ts_matches_js(
            "class C { private static async m(): Promise<void> {} public get x(): number { return 1; } }",
            "class C { static async m() {} get x() { return 1; } }",
        );
    }

    #[test]
    fn strips_type_only_imports_and_exports() {
        assert_ts_matches_js(
            "import type { A } from './a';\nimport { b } from './b';\nexport type { A };\nconst c = b;",
            "import { b } from './b';\nconst c = b;",
        );
    }

    #[test]
    fn strips_overload_signatures_and_declares() {
        assert_ts_matches_js(
            "declare const g: number;\nfunction f(a: string): string;\nfunction f(a: unknown) { return a; }",
            "function f(a) { return a; }",
        );
    }

    #[test]
    fn strips_this_parameter_with_comma() {
        assert_ts_matches_js(
            "function f(this: Window, a: number) { return a; }",
            "function f(a) { return a; }",
        );
    }

    #[test]
    fn strips_type_assertion_prefix() {
        // `<T>expr` assertions are only valid in .ts (not .tsx).
        assert_ts_matches_js("const a = <string>foo;", "const a = foo;");
    }

    #[test]
    fn tsx_annotations_stripped() {
        let ts_h = ts_hashes(
            "const f = (x: number): JSX.Element => <div>{x}</div>;",
            "tsx",
        );
        let mut opts = TokenizeOptions::new(Mode::Mild);
        opts.strip_types_formats = std::collections::HashSet::new();
        let jsx_h: Vec<u64> =
            tokenize_to_detection("jsx", "const f = (x) => <div>{x}</div>;", &opts)
                .into_iter()
                .map(|t| t.hash)
                .collect();
        assert_eq!(ts_h, jsx_h);
    }

    #[test]
    fn non_erasable_enum_left_intact() {
        let stripped = ts_hashes("enum E { A, B }", "typescript");
        let opts = TokenizeOptions::new(Mode::Mild);
        let unstripped: Vec<u64> = tokenize_to_detection("typescript", "enum E { A, B }", &opts)
            .into_iter()
            .map(|t| t.hash)
            .collect();
        assert_eq!(stripped, unstripped, "enums must not be stripped");
    }

    #[test]
    fn non_erasable_parameter_properties_left_intact() {
        let ts = "class C { constructor(private x: number) {} }";
        let stripped = ts_hashes(ts, "typescript");
        // `private` survives; only the type annotation is dropped.
        let expected = ts_hashes("class C { constructor(private x) {} }", "typescript");
        assert_eq!(
            stripped, expected,
            "annotation must be dropped, modifier kept"
        );
        assert_ne!(stripped, js_hashes("class C { constructor(x) {} }"));
    }

    #[test]
    fn no_strip_when_format_not_in_set() {
        let opts = TokenizeOptions::new(Mode::Mild);
        let ts = "const x: number = 5;";
        let plain: Vec<u64> = tokenize_to_detection("typescript", ts, &opts)
            .into_iter()
            .map(|t| t.hash)
            .collect();
        assert_ne!(
            plain,
            js_hashes("const x = 5;"),
            "default path must not strip"
        );
    }
}
