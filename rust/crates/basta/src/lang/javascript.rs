//! JavaScript, TypeScript, JSX and TSX, through the oxc parser.
//!
//! Two oxc products are combined. The semantic pass owns binding resolution:
//! it knows which declaration every identifier refers to, through shadowing,
//! hoisting and TypeScript's declaration merging, which no amount of
//! name matching would get right. The parser's module record owns the module
//! boundary: which specifier each import names and which local declaration
//! each export exposes.
//!
//! What neither provides is class members — methods and fields are not
//! bindings, so they never enter the symbol table — the evidence the
//! confidence model needs (decorators, string literals, computed access), and
//! **CommonJS**. The module record knows only `import` and `export`; a
//! `require("./x")` is a plain call and `module.exports = {…}` a plain
//! assignment as far as the parser is concerned, and a Node project that
//! never touched ESM would read as a pile of files nothing imports. One AST
//! walk collects all of that, and attributes every reference to the
//! declaration whose body encloses it, which is what lets dead code cascade:
//! a helper called only from a dead function is dead too.

use super::{AnalyzeInput, Analyzer, is_identifier_like};
use crate::entry::{collect_strings, looks_like_source_file, script_file_arguments};
use crate::model::{
    FileFacts, Import, ImportKind, ModuleId, ModuleTraits, Reference, ReferenceKind, Symbol,
    SymbolFlags, SymbolId, SymbolKind,
};
use crate::resolve::{ModuleIndex, PathAlias, append_extension, normalize};
use cpd_core::models::Location;
use cpd_tokenizer::line_index::LineIndex;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, PropertyKey};
use oxc_ast::{AstKind, ast};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::module_record::{
    ExportExportName, ExportLocalName, ImportImportName, ModuleRecord,
};
use oxc_syntax::symbol::SymbolFlags as OxcSymbolFlags;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

pub struct JsAnalyzer;

impl Analyzer for JsAnalyzer {
    fn language(&self) -> &'static str {
        "js"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["javascript", "jsx", "typescript", "tsx"]
    }

    fn analyze(&self, input: &AnalyzeInput<'_>) -> FileFacts {
        analyze_js(input)
    }

    fn resolve(&self, specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId> {
        resolve_js(specifier, importer, index)
    }

    fn entry_globs(&self) -> &'static [&'static str] {
        &[
            // Package and application roots.
            "index.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "src/index.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "main.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "src/main.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "cli.{js,mjs,cjs,ts,mts,cts}",
            "src/cli.{js,mjs,cjs,ts,mts,cts}",
            "server.{js,mjs,cjs,ts,mts,cts}",
            "src/server.{js,mjs,cjs,ts,mts,cts}",
            // An ambient declaration file is never imported by anything; the
            // compiler picks it up from the project configuration.
            "**/*.d.{ts,mts,cts}",
            // Anything a build tool loads by name rather than by import.
            "**/*.config.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "**/.*rc.{js,mjs,cjs,ts}",
            // Framework file-system routing: the framework imports these, no
            // file does.
            "**/pages/**/*.{js,jsx,ts,tsx}",
            "**/app/**/{page,layout,route,loading,error,not-found,template,default}.{js,jsx,ts,tsx}",
            "**/src/routes/**/*.{js,ts,svelte}",
            "**/routes/**/*.{js,jsx,ts,tsx}",
            "**/middleware.{js,ts}",
            "**/instrumentation.{js,ts}",
            "**/service-worker.{js,ts}",
            "**/*.stories.{js,jsx,ts,tsx}",
        ]
    }

    fn test_globs(&self) -> &'static [&'static str] {
        &[
            "**/*.{test,spec}.{js,jsx,mjs,cjs,ts,tsx,mts,cts}",
            "**/*.stories.{js,jsx,ts,tsx}",
        ]
    }

    fn manifests(&self) -> &'static [&'static str] {
        &["package.json"]
    }

    fn manifest_entries(&self, directory: &Path, _manifest: &str, text: &str) -> Vec<PathBuf> {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            return Vec::new();
        };
        package_json_entries(&json)
            .iter()
            .flat_map(|relative| {
                source_candidates(&directory.join(relative.trim_start_matches("./")))
            })
            .collect()
    }

    fn alias_configs(&self) -> &'static [&'static str] {
        &["tsconfig.json", "jsconfig.json"]
    }

    fn path_aliases(&self, directory: &Path, _config: &str, text: &str) -> Vec<PathAlias> {
        tsconfig_aliases(directory, text)
    }

    fn module_traits(&self, path: &str) -> ModuleTraits {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        ModuleTraits {
            entry_point: false,
            // `index.ts` exists to re-export a directory's surface.
            package_surface: name.starts_with("index.")
                && !name.contains(".test.")
                && !name.contains(".spec."),
            ambient_declarations: name.ends_with(".d.ts")
                || name.ends_with(".d.mts")
                || name.ends_with(".d.cts"),
            names_are_attributes: false,
        }
    }
}

// ── resolution ──────────────────────────────────────────────────────────────

/// Extensions tried, in order, when a specifier omits one.
///
/// TypeScript first: in a TS project `./x` almost always means `x.ts`, and a
/// stale `x.js` build artifact next to it must not win.
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "d.ts"];

/// `./x`, `../x/y`, a `paths` alias the project declared, and — as a fallback
/// for `baseUrl`-style projects — a bare `src/x` against each scan root.
fn resolve_js(specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId> {
    if specifier.is_empty() {
        return None;
    }
    let from_dir = importer.parent()?;
    if specifier.starts_with("./") || specifier.starts_with("../") || specifier == ".." {
        return candidates(index, &normalize(&from_dir.join(specifier)));
    }
    if let Some(rest) = specifier.strip_prefix('/') {
        // A root-absolute specifier is a bundler alias far more often than a
        // real filesystem path, so it is tried against the scan roots.
        return index.against_roots(rest, |base| candidates(index, base));
    }
    // What the project says about its own import paths beats any guess: an
    // alias is an explicit statement that this prefix is not a package.
    if let Some(id) = index.against_aliases(specifier, importer, |base| candidates(index, base)) {
        return Some(id);
    }
    // A bare specifier is a package unless a scan root makes it a path.
    // Anything with no separator is almost certainly a dependency, and trying
    // those would resolve `react` to a stray `react.ts` fixture.
    if specifier.contains('/') && !specifier.starts_with('@') {
        return index.against_roots(specifier, |base| candidates(index, base));
    }
    None
}

// ── tsconfig path aliases ───────────────────────────────────────────────────

/// How far an `extends` chain is followed before giving up.
const MAX_EXTENDS: usize = 8;

/// The aliases a `tsconfig.json` puts in effect for the files beside it.
///
/// `paths` is not merged along an `extends` chain: the nearest config that
/// declares it wins outright, and its targets resolve against *its own*
/// directory. That is what TypeScript does, and a monorepo whose packages
/// extend a shared base depends on it.
fn tsconfig_aliases(directory: &Path, text: &str) -> Vec<PathAlias> {
    let mut config_dir = directory.to_path_buf();
    let mut text = text.to_string();
    let mut seen: FxHashSet<PathBuf> = FxHashSet::default();
    for _ in 0..MAX_EXTENDS {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&strip_jsonc(&text)) else {
            return Vec::new();
        };
        let options = json.get("compilerOptions");
        if let Some(paths) = options
            .and_then(|o| o.get("paths"))
            .and_then(|p| p.as_object())
        {
            // Since TypeScript 4.1 `paths` needs no `baseUrl`, and then it is
            // read relative to the config file itself.
            let base = options
                .and_then(|o| o.get("baseUrl"))
                .and_then(|b| b.as_str())
                .unwrap_or(".");
            let base_dir = normalize(&config_dir.join(base));
            return build_aliases(directory, &base_dir, paths);
        }
        let Some(next) = json.get("extends").and_then(|e| e.as_str()) else {
            return Vec::new();
        };
        let Some((next_dir, next_text)) = read_extends(&config_dir, next, &mut seen) else {
            return Vec::new();
        };
        config_dir = next_dir;
        text = next_text;
    }
    Vec::new()
}

/// The config an `extends` names, when it is one basta can reach.
///
/// Only relative references are followed. A bare `extends` names a package,
/// which lives in `node_modules` — never scanned, and resolving it would mean
/// implementing node resolution for a file that rarely carries `paths`.
fn read_extends(
    config_dir: &Path,
    specifier: &str,
    seen: &mut FxHashSet<PathBuf>,
) -> Option<(PathBuf, String)> {
    if !specifier.starts_with('.') {
        return None;
    }
    let mut target = normalize(&config_dir.join(specifier));
    if target.extension().is_none() {
        target = append_extension(&target, "json");
    }
    if !seen.insert(target.clone()) {
        return None; // A cycle; stop rather than loop.
    }
    let text = std::fs::read_to_string(&target).ok()?;
    Some((target.parent()?.to_path_buf(), text))
}

/// Turn a `paths` table into aliases with absolute targets.
fn build_aliases(
    scope: &Path,
    base_dir: &Path,
    paths: &serde_json::Map<String, serde_json::Value>,
) -> Vec<PathAlias> {
    paths
        .iter()
        .filter_map(|(pattern, targets)| {
            let wildcard = pattern.ends_with('*');
            let targets: Vec<PathBuf> = targets
                .as_array()?
                .iter()
                .filter_map(|t| t.as_str())
                .map(|target| {
                    // For a wildcard the target is a prefix: `./src/*` means
                    // everything under `src`. Anything after the `*` is not
                    // representable as a base path and is dropped.
                    let head = match wildcard {
                        true => target.split('*').next().unwrap_or(target),
                        false => target,
                    };
                    normalize(&base_dir.join(head.trim_end_matches('/')))
                })
                .collect();
            (!targets.is_empty()).then(|| PathAlias {
                scope: scope.to_path_buf(),
                prefix: pattern.trim_end_matches('*').to_string(),
                wildcard,
                targets,
            })
        })
        .collect()
}

/// Strip comments and trailing commas from JSON with comments.
///
/// `tsconfig.json` is JSONC, and its path patterns look like `"@/*"`. A
/// comment strip that does not track string boundaries starts eating the file
/// at that `/*` and silently yields an empty alias table — which looks exactly
/// like a project that declared no aliases at all.
fn strip_jsonc(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(bytes.len());
                out.extend_from_slice(&bytes[start..i]);
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    drop_trailing_commas(&out)
}

/// Remove a comma that is followed only by whitespace and a closing bracket.
fn drop_trailing_commas(bytes: &[u8]) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(bytes.len());
            out.extend_from_slice(&bytes[start..i]);
            continue;
        }
        if bytes[i] == b',' {
            let next = bytes[i + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .copied();
            if matches!(next, Some(b'}') | Some(b']')) {
                i += 1;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Every file a specifier could name, in priority order.
fn candidates(index: &ModuleIndex, base: &Path) -> Option<ModuleId> {
    // An explicit extension wins outright.
    if let Some(id) = index.get(base) {
        return Some(id);
    }
    // `./x` → `x.ts`, `x.tsx`, ...
    for extension in EXTENSIONS {
        if let Some(id) = index.get(&append_extension(base, extension)) {
            return Some(id);
        }
    }
    // `./x` → `x/index.ts`, ...
    for extension in EXTENSIONS {
        if let Some(id) = index.get(&base.join(format!("index.{extension}"))) {
            return Some(id);
        }
    }
    // `./x.js` in an ESM TypeScript project means `x.ts` on disk.
    if let Some(rewritten) = rewrite_js_extension_to_ts(base)
        && let Some(id) = index.get(&rewritten)
    {
        return Some(id);
    }
    None
}

/// A string literal that *names* a source file rather than describing one.
///
/// A build script's entry, a worker spawned through `new URL('./w.ts', …)`, a
/// setup file listed in `vitest.config.ts`: the file is named but never
/// imported, so nothing else in the walk can see that it is alive.
///
/// Deliberately narrow. The string must carry an explicit source extension,
/// and either be relative or name a directory — prose, bare words and URLs
/// cannot become edges. A literal that survives this test still has to
/// resolve to a file the scan actually walked before it means anything.
fn module_path_literal(value: &str) -> Option<String> {
    if value.contains("://") || !value.contains('.') {
        return None;
    }
    let relative = value.starts_with("./") || value.starts_with("../");
    if !relative && !value.contains('/') {
        return None;
    }
    let extension = value.rsplit('.').next()?;
    EXTENSIONS
        .contains(&extension)
        .then(|| value.trim().to_string())
}

/// `./x.js` → `./x.ts` and friends. TypeScript's ESM output keeps the `.js`
/// specifier a project writes against a `.ts` file, so a literal match fails
/// on exactly the projects that follow the spec most carefully.
fn rewrite_js_extension_to_ts(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let (stem, replacement) = match () {
        _ if name.ends_with(".js") => (name.trim_end_matches(".js"), "ts"),
        _ if name.ends_with(".mjs") => (name.trim_end_matches(".mjs"), "mts"),
        _ if name.ends_with(".cjs") => (name.trim_end_matches(".cjs"), "cts"),
        _ if name.ends_with(".jsx") => (name.trim_end_matches(".jsx"), "tsx"),
        _ => return None,
    };
    Some(path.with_file_name(format!("{stem}.{replacement}")))
}

// ── manifests ───────────────────────────────────────────────────────────────

/// Every file path a `package.json` points at.
fn package_json_entries(json: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for field in ["main", "module", "browser", "types", "typings", "unpkg"] {
        if let Some(value) = json.get(field).and_then(serde_json::Value::as_str) {
            out.push(value.to_string());
        }
    }
    for field in ["bin", "exports", "imports"] {
        if let Some(value) = json.get(field) {
            collect_strings(value, &mut out);
        }
    }
    out.retain(|path| path.starts_with('.') || path.starts_with("dist") || path.contains('/'));

    // `files` is what the published tarball contains. A source file listed
    // there ships to every consumer, any of whom may `require('pkg/x.js')` it
    // directly, so it is a public surface whether or not the package's own
    // entry ever imports it. Directories are skipped: `"files": ["dist"]`
    // says nothing about which files in `dist` matter.
    if let Some(files) = json.get("files").and_then(serde_json::Value::as_array) {
        out.extend(
            files
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|entry| looks_like_source_file(entry))
                .map(|entry| entry.trim_start_matches("./").to_string()),
        );
    }

    if let Some(scripts) = json.get("scripts").and_then(serde_json::Value::as_object) {
        for command in scripts.values().filter_map(serde_json::Value::as_str) {
            out.extend(script_file_arguments(command));
        }
    }
    out
}

/// Directory names a build writes into. A manifest points at the built file;
/// the repository holds the source it was built from.
const OUTPUT_DIRS: &[&str] = &[
    "dist", "build", "lib", "out", "esm", "cjs", "output", ".output",
];

/// Source extensions a built `.js` file could have come from.
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

/// Every source file a manifest entry could correspond to.
///
/// `"main": "./dist/index.js"` names a file that only exists after a build,
/// which is exactly the file a repository does not contain. Without this the
/// published entry point of every TypeScript package looks unreachable, and
/// the whole package reads as dead.
fn source_candidates(named: &Path) -> Vec<PathBuf> {
    let mut bases = vec![named.to_path_buf()];
    // `dist/index.js` also means `src/index.js` and `index.js`.
    if let Some(rest) = strip_output_dir(named) {
        bases.push(rest.clone());
        if let (Some(parent), Some(name)) = (rest.parent(), rest.file_name()) {
            bases.push(parent.join("src").join(name));
        }
    }

    let mut out = Vec::with_capacity(bases.len() * (SOURCE_EXTENSIONS.len() + 1));
    for base in bases {
        out.push(base.clone());
        // `index.d.ts` was generated from `index.ts`, not `index.d.tsx`.
        let stem = base
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| {
                name.strip_suffix(".d.ts")
                    .or_else(|| name.rsplit_once('.').map(|(stem, _)| stem))
                    .unwrap_or(name)
                    .to_string()
            })
            .unwrap_or_default();
        if stem.is_empty() {
            continue;
        }
        let Some(parent) = base.parent() else {
            continue;
        };
        for extension in SOURCE_EXTENSIONS {
            out.push(parent.join(format!("{stem}.{extension}")));
            // A package whose entry is a directory resolves through its index.
            out.push(parent.join(&stem).join(format!("index.{extension}")));
        }
    }
    out
}

/// `<pkg>/dist/a/b.js` → `<pkg>/a/b.js`, for the first output directory in
/// the path. `None` when the path goes through none.
fn strip_output_dir(path: &Path) -> Option<PathBuf> {
    let components: Vec<_> = path.components().collect();
    let index = components.iter().position(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| OUTPUT_DIRS.contains(&name))
    })?;
    let mut out = PathBuf::new();
    for (position, component) in components.iter().enumerate() {
        if position != index {
            out.push(component.as_os_str());
        }
    }
    Some(out)
}

// ── parsing ─────────────────────────────────────────────────────────────────

/// The oxc source type for a jscpd format. The parser needs to know whether
/// angle brackets open a JSX element or a type assertion, which the format
/// name already decides.
fn source_type_for(format: &str, path: &str) -> SourceType {
    let base = match format {
        "typescript" => SourceType::ts(),
        "tsx" => SourceType::tsx(),
        "jsx" => SourceType::jsx(),
        _ => SourceType::jsx(),
    };
    // `.cjs` and `.cts` are CommonJS whatever the format says; everything else
    // is parsed as a module so that top-level `import` is legal.
    if path.ends_with(".cjs") || path.ends_with(".cts") {
        base.with_script(true)
    } else {
        base.with_module(true)
    }
}

fn analyze_js(input: &AnalyzeInput<'_>) -> FileFacts {
    let allocator = Allocator::new();
    let source_type = source_type_for(input.format, input.path);
    let mut parsed = Parser::new(&allocator, input.source, source_type).parse();
    // Recoverable diagnostics still leave a usable tree; only a parser that
    // gave up leaves nothing to analyze (matches the tokenizer, issue #1023).
    // A `.js` file is parsed as a module first, because that is what most of
    // them are now; one that is really a sloppy-mode script — `with`, octal
    // escapes, an HTML comment — gets a second chance as one.
    if parsed.panicked && source_type.is_module() && !source_type.is_typescript() {
        parsed = Parser::new(&allocator, input.source, source_type.with_script(true)).parse();
    }
    if parsed.panicked {
        return FileFacts::unparsed();
    }

    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let scoping = semantic.scoping();
    let lines = LineIndex::new(input.source.as_bytes());

    // Pass 1 — turn oxc's symbol table into basta symbols. Keyed by oxc's
    // SymbolId so exports and references can find them again.
    let mut builder = Builder::new(input, &lines);
    let mut by_oxc: FxHashMap<oxc_syntax::symbol::SymbolId, SymbolId> = FxHashMap::default();
    let root_scope = scoping.root_scope_id();

    // References that exist only because a name appears in an `export { x }`
    // clause are not uses of `x`: skipping them keeps "exported but never used
    // in its own file" honest. `module.exports = { x }` is the same statement
    // in CommonJS; the walk finds those, so reference counting waits for it.
    let export_clause_spans: FxHashSet<u32> = parsed
        .module_record
        .local_export_entries
        .iter()
        .filter_map(|e| match &e.local_name {
            // An `export { local as name }` entry points at the clause; an
            // `export function f` entry points at the declaration itself,
            // which is not a reference and never appears in the reference list.
            ExportLocalName::Name(n) => Some(n.span.start),
            _ => None,
        })
        .collect();

    for oxc_id in scoping.symbol_ids() {
        let name = scoping.symbol_name(oxc_id).to_string();
        let flags = scoping.symbol_flags(oxc_id);
        let decl_node = scoping.symbol_declaration(oxc_id);
        let decl_kind = semantic.nodes().get_node(decl_node).kind();
        let Some(kind) = symbol_kind_of(decl_kind, flags) else {
            // Parameters, catch bindings and destructured locals are not
            // declarations basta reports on.
            continue;
        };
        let binding_span = scoping.symbol_span(oxc_id);
        let decl_span = declaration_span(decl_kind, binding_span);

        let mut sym_flags = SymbolFlags::NONE;
        sym_flags.set(
            SymbolFlags::TOP_LEVEL,
            scoping.symbol_scope_id(oxc_id) == root_scope,
        );
        sym_flags.set(SymbolFlags::PRIVATE_NAME, name.starts_with('#'));
        sym_flags.set(SymbolFlags::TYPE_ONLY, kind.is_type_only());

        let id = builder.push(Symbol {
            id: SymbolId(0), // assigned by push
            module: input.module,
            name,
            kind,
            flags: sym_flags,
            start: lines.location(binding_span.start as usize),
            end: lines.location(decl_span.end as usize),
            exported_as: None,
            parent: None,
            local_refs: 0, // counted after the walk
            lines: 0,
        });
        by_oxc.insert(oxc_id, id);
    }

    // Pass 2 — the AST walk: class and enum members, member accesses,
    // references attributed to their enclosing declaration, and the evidence
    // the confidence model needs.
    let decl_starts: FxHashMap<u32, SymbolId> = builder
        .symbols
        .iter()
        .map(|s| (s.start.offset, s.id))
        .collect();
    let mut walker = Walk {
        builder: &mut builder,
        decl_starts: &decl_starts,
        export_clause_spans: &export_clause_spans,
        stack: Vec::new(),
        class_stack: Vec::new(),
        dynamic: false,
        strings: FxHashSet::default(),
        commonjs: CommonJs::default(),
        handled_requires: FxHashSet::default(),
        cjs_export_refs: FxHashSet::default(),
    };
    walker.visit_program(&parsed.program);
    let dynamic = walker.dynamic;
    let strings = std::mem::take(&mut walker.strings);
    let commonjs = std::mem::take(&mut walker.commonjs);
    let cjs_export_refs = std::mem::take(&mut walker.cjs_export_refs);
    drop(walker);

    // References from within the file, minus the ones that only export the
    // name — in either module system.
    for (oxc_id, id) in &by_oxc {
        builder.symbols[id.0 as usize].local_refs = scoping
            .get_resolved_references(*oxc_id)
            .filter(|r| {
                let start = semantic.nodes().get_node(r.node_id()).kind().span().start;
                !export_clause_spans.contains(&start) && !cjs_export_refs.contains(&start)
            })
            .count() as u32;
    }

    // Pass 3 — the module boundary, ESM from the module record and CommonJS
    // from the walk.
    let mut imports = collect_imports(&parsed.module_record, input, &lines, &builder);
    imports.extend(commonjs_imports(&commonjs, input, &lines, &mut builder));
    let mut facts = builder.finish();
    apply_exports(&mut facts, &parsed.module_record);
    apply_commonjs_exports(&mut facts, &commonjs);
    facts.imports = imports;
    facts.has_dynamic_access = dynamic;
    add_string_references(&mut facts, &strings, input.module);
    facts
}

/// Which basta symbol kind an oxc declaration node represents, or `None` for
/// declarations basta does not report on (parameters, destructured locals).
fn symbol_kind_of(kind: AstKind<'_>, flags: OxcSymbolFlags) -> Option<SymbolKind> {
    Some(match kind {
        AstKind::Function(_) => SymbolKind::Function,
        AstKind::Class(_) => SymbolKind::Class,
        AstKind::TSEnumDeclaration(_) => SymbolKind::Enum,
        AstKind::TSEnumMember(_) => SymbolKind::EnumMember,
        AstKind::TSTypeAliasDeclaration(_) => SymbolKind::TypeAlias,
        AstKind::TSInterfaceDeclaration(_) => SymbolKind::Interface,
        AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_) => SymbolKind::Import,
        AstKind::VariableDeclarator(d) => {
            // `const f = () => {}` is a function to a reader, and reporting it
            // as one makes the message match the code.
            match &d.init {
                Some(
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_),
                ) => SymbolKind::Function,
                Some(Expression::ClassExpression(_)) => SymbolKind::Class,
                _ => SymbolKind::Variable,
            }
        }
        _ if flags.is_function() => SymbolKind::Function,
        _ if flags.is_class() => SymbolKind::Class,
        _ => return None,
    })
}

/// The span of the whole declaration, which is what a reader would delete.
/// Falls back to the binding identifier for declaration forms whose node is
/// just the name.
fn declaration_span(kind: AstKind<'_>, binding: Span) -> Span {
    match kind {
        AstKind::Function(_)
        | AstKind::Class(_)
        | AstKind::TSEnumDeclaration(_)
        | AstKind::TSEnumMember(_)
        | AstKind::TSTypeAliasDeclaration(_)
        | AstKind::TSInterfaceDeclaration(_)
        | AstKind::VariableDeclarator(_) => kind.span(),
        _ => binding,
    }
}

/// Accumulates symbols and references while assigning ids and resolving byte
/// offsets, so no caller has to manage either.
struct Builder<'a> {
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
    module: crate::model::ModuleId,
    lines: &'a LineIndex,
    len: usize,
}

impl<'a> Builder<'a> {
    fn new(input: &'a AnalyzeInput<'a>, lines: &'a LineIndex) -> Self {
        Self {
            symbols: Vec::new(),
            references: Vec::new(),
            module: input.module,
            lines,
            len: input.source.len(),
        }
    }

    /// Byte offset to a line/column location, clamped to the source.
    fn loc(&self, offset: u32) -> Location {
        self.lines.location((offset as usize).min(self.len))
    }

    fn push(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        symbol.id = id;
        symbol.module = self.module;
        symbol.lines = symbol.end.line.saturating_sub(symbol.start.line) + 1;
        self.symbols.push(symbol);
        id
    }

    fn reference(&mut self, name: String, kind: ReferenceKind, from: Option<SymbolId>, at: u32) {
        let at = self.loc(at);
        self.references.push(Reference {
            module: self.module,
            name,
            kind,
            from,
            at,
        });
    }

    fn finish(self) -> FileFacts {
        FileFacts {
            symbols: self.symbols,
            imports: Vec::new(),
            references: self.references,
            has_dynamic_access: false,
            parse_failed: false,
        }
    }
}

struct Walk<'a, 'b> {
    builder: &'b mut Builder<'a>,
    /// Declaration byte offset → the symbol it declares, so the walker can
    /// tell when it enters a declaration pass 1 already recorded.
    decl_starts: &'b FxHashMap<u32, SymbolId>,
    /// Byte offsets of the names inside `export { … }` clauses.
    export_clause_spans: &'b FxHashSet<u32>,
    /// Declarations currently open, innermost last, each flagged with whether
    /// it is one a reference can be attributed to. See [`Walk::current`].
    stack: Vec<(SymbolId, bool)>,
    /// Enclosing class symbols, so members get a parent.
    class_stack: Vec<SymbolId>,
    dynamic: bool,
    strings: FxHashSet<String>,
    /// `require(…)` calls and `module.exports` assignments, resolved after the
    /// walk. See [`CommonJs`].
    commonjs: CommonJs,
    /// Byte offsets of `require` calls a declarator already accounted for, so
    /// the call itself is not recorded a second time as a bare side effect.
    handled_requires: FxHashSet<u32>,
    /// Byte offsets of the identifiers `module.exports = { x }` and
    /// `exports.x = x` name. Exporting a declaration is not using it, in
    /// CommonJS exactly as in ESM.
    cjs_export_refs: FxHashSet<u32>,
}

/// What the walk learned about a file's CommonJS module boundary.
#[derive(Default)]
struct CommonJs {
    /// Imports in the order they were seen. `local` is the byte offset of the
    /// binding identifier, turned into a symbol after the walk.
    imports: Vec<CjsImport>,
    /// `(exported name, local name)` pairs from `module.exports` / `exports.x`.
    /// A local of `None` means the value was not a plain identifier, so there
    /// is no declaration to mark.
    exports: Vec<(String, Option<String>)>,
}

struct CjsImport {
    specifier: String,
    kind: ImportKind,
    local: Option<u32>,
    at: u32,
}

impl Walk<'_, '_> {
    /// The declaration a reference at this point belongs to.
    ///
    /// The innermost *reportable* one, not simply the innermost. `const all =
    /// helper()` inside an exported function declares `all`, but `all` is a
    /// local basta never reports and therefore never reaches — attributing
    /// the call to it would strand `helper` behind a node with no way in, and
    /// a live helper would be reported as dead.
    fn current(&self) -> Option<SymbolId> {
        self.stack
            .iter()
            .rev()
            .find_map(|(id, reportable)| reportable.then_some(*id))
    }

    /// Open a declaration. `reportable` says whether references inside it
    /// belong to it or to whatever encloses it.
    fn open(&mut self, id: SymbolId, reportable: bool) {
        self.stack.push((id, reportable));
    }

    /// Record a class member (method, accessor or field) and an enum member.
    fn member(&mut self, name: String, kind: SymbolKind, span: Span, flags: SymbolFlags) {
        let parent = self.class_stack.last().copied();
        let mut flags = flags;
        flags.insert(SymbolFlags::MEMBER);
        flags.set(SymbolFlags::PRIVATE_NAME, name.starts_with('#'));
        flags.set(SymbolFlags::MAGIC, is_magic_member(&name));
        let start = self.builder.loc(span.start);
        let end = self.builder.loc(span.end);
        let id = self.builder.push(Symbol {
            id: SymbolId(0),
            module: self.builder.module,
            name,
            kind,
            flags,
            start,
            end,
            exported_as: None,
            parent,
            local_refs: 0,
            lines: 0,
        });
        // A member is reported in its own right, so references in its body
        // belong to it rather than to the class around it.
        self.open(id, true);
    }
}

impl<'a> Visit<'a> for Walk<'a, '_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            // A declaration pass 1 already recorded: open it so references in
            // its body are attributed to it.
            AstKind::Function(_)
            | AstKind::Class(_)
            | AstKind::VariableDeclarator(_)
            | AstKind::TSEnumDeclaration(_)
            | AstKind::TSTypeAliasDeclaration(_)
            | AstKind::TSInterfaceDeclaration(_) => {
                if let AstKind::VariableDeclarator(declarator) = kind {
                    self.commonjs_declarator(declarator);
                }
                if let Some(id) = self.decl_starts.get(&binding_start(kind)).copied() {
                    let reportable = self.builder.symbols[id.0 as usize]
                        .flags
                        .contains(SymbolFlags::TOP_LEVEL);
                    self.open(id, reportable);
                    if matches!(kind, AstKind::Class(_)) {
                        self.class_stack.push(id);
                    }
                }
            }
            AstKind::MethodDefinition(m) => {
                let name = member_name(&m.key);
                let mut flags = SymbolFlags::NONE;
                flags.set(SymbolFlags::DECORATED, !m.decorators.is_empty());
                flags.set(SymbolFlags::ABSTRACT, m.r#type.is_abstract());
                flags.set(SymbolFlags::OVERRIDE, m.r#override);
                if m.key.is_specific_id("constructor") || m.kind.is_constructor() {
                    flags.insert(SymbolFlags::MAGIC);
                }
                self.member(name, SymbolKind::Method, m.span, flags);
            }
            AstKind::PropertyDefinition(p) => {
                let name = member_name(&p.key);
                let mut flags = SymbolFlags::NONE;
                flags.set(SymbolFlags::DECORATED, !p.decorators.is_empty());
                flags.set(SymbolFlags::OVERRIDE, p.r#override);
                self.member(name, SymbolKind::Property, p.span, flags);
            }
            AstKind::AccessorProperty(p) => {
                let name = member_name(&p.key);
                self.member(name, SymbolKind::Property, p.span, SymbolFlags::NONE);
            }
            // An identifier read. oxc already decided which declaration it
            // refers to, including through shadowing.
            AstKind::IdentifierReference(ident) => {
                // The name in an `export { x }` clause is not a use of `x`.
                // Counting it as one would make every export its own reason
                // to stay alive, and no unused export would ever be found.
                if self.export_clause_spans.contains(&ident.span.start)
                    || self.cjs_export_refs.contains(&ident.span.start)
                {
                    return;
                }
                let from = self.current();
                self.builder.reference(
                    ident.name.to_string(),
                    ReferenceKind::Binding,
                    from,
                    ident.span.start,
                );
            }
            // `obj.name` / `obj?.name`: matched against member declarations by
            // name, since basta infers no types.
            AstKind::StaticMemberExpression(m) => {
                let from = self.current();
                self.builder.reference(
                    m.property.name.to_string(),
                    ReferenceKind::Member,
                    from,
                    m.property.span.start,
                );
            }
            AstKind::PrivateFieldExpression(m) => {
                let from = self.current();
                self.builder.reference(
                    format!("#{}", m.field.name),
                    ReferenceKind::Member,
                    from,
                    m.field.span.start,
                );
            }
            // `obj[expr]` with a literal key is still a static access; with a
            // computed key the module can reach any member by name.
            AstKind::ComputedMemberExpression(m) => {
                let from = self.current();
                match &m.expression {
                    Expression::StringLiteral(s) => self.builder.reference(
                        s.value.to_string(),
                        ReferenceKind::Member,
                        from,
                        s.span.start,
                    ),
                    _ => self.dynamic = true,
                }
            }
            AstKind::StringLiteral(s) => {
                // Only identifier-shaped strings can name a declaration, and
                // keeping the rest would blow the set up on data-heavy files.
                if is_identifier_like(&s.value) {
                    self.strings.insert(s.value.to_string());
                } else if let Some(specifier) = module_path_literal(&s.value) {
                    self.commonjs.imports.push(CjsImport {
                        specifier,
                        kind: ImportKind::SideEffect,
                        local: None,
                        at: s.span.start,
                    });
                }
            }
            AstKind::TemplateLiteral(_) => {}
            AstKind::CallExpression(call) => {
                if let Expression::Identifier(callee) = &call.callee {
                    match callee.name.as_str() {
                        // A module that evaluates source, or resolves a
                        // specifier at runtime, can reach anything.
                        "eval" => self.dynamic = true,
                        "require" => match require_specifier(call) {
                            // A `require` some declarator already turned into
                            // a binding is done with; any other is a
                            // side-effect import, or a call whose result goes
                            // somewhere the walker cannot follow.
                            Some(specifier) => {
                                if !self.handled_requires.contains(&call.span.start) {
                                    self.commonjs.imports.push(CjsImport {
                                        specifier,
                                        kind: ImportKind::SideEffect,
                                        local: None,
                                        at: call.span.start,
                                    });
                                }
                            }
                            // `require(name)` names whatever the variable
                            // held at the time.
                            None => self.dynamic = true,
                        },
                        _ => {}
                    }
                }
                // `require.resolve('./x')` proves the file is wanted even
                // though nothing is bound.
                if let Expression::StaticMemberExpression(member) = &call.callee
                    && matches!(&member.object, Expression::Identifier(id) if id.name == "require")
                    && member.property.name == "resolve"
                    && let Some(specifier) = require_specifier(call)
                {
                    self.commonjs.imports.push(CjsImport {
                        specifier,
                        kind: ImportKind::SideEffect,
                        local: None,
                        at: call.span.start,
                    });
                }
            }
            AstKind::AssignmentExpression(assignment) => {
                self.commonjs_assignment(assignment);
            }
            AstKind::ImportExpression(expr) => {
                if !matches!(&expr.source, Expression::StringLiteral(_)) {
                    self.dynamic = true;
                }
            }
            AstKind::JSXIdentifier(ident) => {
                // `<Widget />` is a use of `Widget` that the semantic pass
                // records as an identifier reference only for the element
                // name; member expressions like `<Ns.Widget />` still need it.
                let from = self.current();
                if ident.name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    self.builder.reference(
                        ident.name.to_string(),
                        ReferenceKind::Binding,
                        from,
                        ident.span.start,
                    );
                }
            }
            _ => {}
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(_)
            | AstKind::VariableDeclarator(_)
            | AstKind::TSEnumDeclaration(_)
            | AstKind::TSTypeAliasDeclaration(_)
            | AstKind::TSInterfaceDeclaration(_) => {
                if self.decl_starts.contains_key(&binding_start(kind)) {
                    self.stack.pop();
                }
            }
            AstKind::Class(_) => {
                if self.decl_starts.contains_key(&binding_start(kind)) {
                    self.stack.pop();
                    self.class_stack.pop();
                }
            }
            AstKind::MethodDefinition(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_) => {
                self.stack.pop();
            }
            _ => {}
        }
    }
}

impl Walk<'_, '_> {
    /// `const x = require('./m')`, `const { a, b: c } = require('./m')` and
    /// `const x = require('./m').member` bind what a module exports.
    fn commonjs_declarator(&mut self, declarator: &ast::VariableDeclarator<'_>) {
        let Some(init) = &declarator.init else {
            return;
        };
        // `require('./m')` or `require('./m').name`.
        let (call, member) = match init {
            Expression::CallExpression(call) => (call, None),
            Expression::StaticMemberExpression(access) => match &access.object {
                Expression::CallExpression(call) => (call, Some(access.property.name.as_str())),
                _ => return,
            },
            _ => return,
        };
        if !matches!(&call.callee, Expression::Identifier(id) if id.name == "require") {
            return;
        }
        let Some(specifier) = require_specifier(call) else {
            return;
        };
        self.handled_requires.insert(call.span.start);
        let at = call.span.start;

        match (&declarator.id, member) {
            // `const x = require('./m').name` — one export, bound as `x`.
            (ast::BindingPattern::BindingIdentifier(id), Some(name)) => {
                self.commonjs.imports.push(CjsImport {
                    specifier,
                    kind: ImportKind::Named(name.to_string()),
                    local: Some(id.span.start),
                    at,
                });
            }
            // `const m = require('./m')` — the whole module object.
            (ast::BindingPattern::BindingIdentifier(id), None) => {
                self.commonjs.imports.push(CjsImport {
                    specifier,
                    kind: ImportKind::Namespace,
                    local: Some(id.span.start),
                    at,
                });
            }
            // `const { a, b: c } = require('./m')` — one named import each.
            (ast::BindingPattern::ObjectPattern(pattern), None) => {
                for property in &pattern.properties {
                    let Some(exported) = property.key.static_name() else {
                        continue;
                    };
                    let local = match &property.value {
                        ast::BindingPattern::BindingIdentifier(id) => Some(id.span.start),
                        // `const { a: { b } } = …` reaches `a`; what is bound
                        // inside it is a local basta does not track.
                        _ => None,
                    };
                    self.commonjs.imports.push(CjsImport {
                        specifier: specifier.clone(),
                        kind: ImportKind::Named(exported.into_owned()),
                        local,
                        at,
                    });
                }
                // `const { ...rest } = require('./m')` takes everything.
                if pattern.rest.is_some() {
                    self.commonjs.imports.push(CjsImport {
                        specifier,
                        kind: ImportKind::Namespace,
                        local: None,
                        at,
                    });
                }
            }
            // Anything else (`const [a] = require(…)`, a nested member chain)
            // still proves the module is wanted.
            _ => self.commonjs.imports.push(CjsImport {
                specifier,
                kind: ImportKind::Namespace,
                local: None,
                at,
            }),
        }
    }

    /// `module.exports = …` and `exports.name = …` are the file's exports.
    fn commonjs_assignment(&mut self, assignment: &ast::AssignmentExpression<'_>) {
        use ast::AssignmentTarget;
        // Only the module's own top level exports anything; a `module.exports`
        // inside a function is somebody's plugin API, not this file's surface.
        if !self.stack.is_empty() {
            return;
        }
        let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
            return;
        };
        let property = target.property.name.as_str();
        match &target.object {
            // `module.exports = …`
            Expression::Identifier(object) if object.name == "module" && property == "exports" => {
                self.commonjs_export_value(&assignment.right);
            }
            // `exports.name = …`
            Expression::Identifier(object) if object.name == "exports" => {
                self.commonjs_export_one(property, &assignment.right);
            }
            // `module.exports.name = …`
            Expression::StaticMemberExpression(inner)
                if matches!(&inner.object, Expression::Identifier(id) if id.name == "module")
                    && inner.property.name == "exports" =>
            {
                self.commonjs_export_one(property, &assignment.right);
            }
            _ => {}
        }
    }

    /// The right-hand side of `module.exports = …`.
    fn commonjs_export_value(&mut self, value: &Expression<'_>) {
        match value {
            // `module.exports = { a, b: c, d() {} }` — one export per property.
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    let ast::ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue; // `...spread` names nothing basta can see.
                    };
                    let Some(exported) = property.key.static_name() else {
                        continue;
                    };
                    self.commonjs_export_one(&exported, &property.value);
                }
            }
            // `module.exports = thing` — the module *is* the thing, which
            // importers reach as the default / the namespace.
            other => self.commonjs_export_one("default", other),
        }
    }

    /// One exported name and the expression it exposes. When that expression
    /// is a bare identifier, its span is remembered so the reference it makes
    /// is not mistaken for a use.
    fn commonjs_export_one(&mut self, exported: &str, value: &Expression<'_>) {
        if let Expression::Identifier(id) = value {
            self.cjs_export_refs.insert(id.span.start);
        }
        self.commonjs
            .exports
            .push((exported.to_string(), identifier_name(value)));
    }
}

/// The string a `require` call names, when it names one statically.
fn require_specifier(call: &ast::CallExpression<'_>) -> Option<String> {
    match call.arguments.first().and_then(|a| a.as_expression()) {
        Some(Expression::StringLiteral(literal)) => Some(literal.value.to_string()),
        _ => None,
    }
}

/// The name an expression reads, when it is a bare identifier.
fn identifier_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(id) => Some(id.name.to_string()),
        _ => None,
    }
}

/// Turn the walk's `require` findings into imports. Their locals were
/// declared by pass 1 as ordinary variables — `const { a } = require(…)` is a
/// destructuring declarator to the parser — and are re-kinded here: they are
/// imports to a reader and to the unused-import rule alike.
fn commonjs_imports(
    commonjs: &CommonJs,
    input: &AnalyzeInput<'_>,
    lines: &LineIndex,
    builder: &mut Builder<'_>,
) -> Vec<Import> {
    let by_offset: FxHashMap<u32, SymbolId> = builder
        .symbols
        .iter()
        .map(|s| (s.start.offset, s.id))
        .collect();
    commonjs
        .imports
        .iter()
        .map(|import| {
            let local = import
                .local
                .and_then(|offset| by_offset.get(&offset).copied());
            if let Some(id) = local {
                builder.symbols[id.0 as usize].kind = SymbolKind::Import;
            }
            Import {
                module: input.module,
                specifier: import.specifier.clone(),
                kind: import.kind.clone(),
                local,
                start: lines.location(import.at as usize),
                type_only: false,
            }
        })
        .collect()
}

/// Mark the declarations `module.exports` / `exports.x` expose.
fn apply_commonjs_exports(facts: &mut FileFacts, commonjs: &CommonJs) {
    for (exported, local) in &commonjs.exports {
        let Some(local) = local else {
            continue;
        };
        // A `module.exports = { x }` where `x` is itself an import binding
        // re-exports it; the symbol then carries both roles, like an ESM
        // `export { x }` of an imported name.
        if let Some(symbol) = facts.symbols.iter_mut().find(|s| {
            s.flags.contains(SymbolFlags::TOP_LEVEL) && &s.name == local && s.exported_as.is_none()
        }) {
            symbol.exported_as = Some(exported.clone());
            symbol
                .flags
                .set(SymbolFlags::DEFAULT_EXPORT, exported == "default");
        }
    }
}

/// The byte offset pass 1 keyed this declaration's symbol by: its binding
/// identifier, or the node itself when it has no name.
fn binding_start(kind: AstKind<'_>) -> u32 {
    match kind {
        AstKind::Function(f) => f.id.as_ref().map_or(f.span.start, |id| id.span.start),
        AstKind::Class(c) => c.id.as_ref().map_or(c.span.start, |id| id.span.start),
        AstKind::VariableDeclarator(d) => {
            d.id.get_binding_identifier()
                .map_or(d.span.start, |id| id.span.start)
        }
        AstKind::TSEnumDeclaration(e) => e.id.span.start,
        AstKind::TSTypeAliasDeclaration(t) => t.id.span.start,
        AstKind::TSInterfaceDeclaration(i) => i.id.span.start,
        other => other.span().start,
    }
}

/// The name a class member is accessed by. A `#private` member keeps its
/// sigil so it matches the `this.#x` references the walker records, and a
/// computed key gets a placeholder that no reference can ever match — which
/// is what makes such members unreportable rather than falsely dead.
fn member_name(key: &PropertyKey<'_>) -> String {
    match key {
        PropertyKey::PrivateIdentifier(id) => format!("#{}", id.name),
        other => other
            .static_name()
            .map(|n| n.into_owned())
            .unwrap_or_else(|| "<computed>".into()),
    }
}

/// Members the runtime calls without anyone naming them.
fn is_magic_member(name: &str) -> bool {
    matches!(
        name,
        "constructor"
            | "toString"
            | "toJSON"
            | "valueOf"
            | "then"
            | "catch"
            | "finally"
            | "next"
            | "return"
            | "throw"
            | "dispose"
    ) || name.starts_with("[Symbol.")
}

/// Imports, in module-record terms.
fn collect_imports(
    record: &ModuleRecord<'_>,
    input: &AnalyzeInput<'_>,
    lines: &LineIndex,
    builder: &Builder<'_>,
) -> Vec<Import> {
    let mut imports = Vec::new();
    // Local binding by declaration offset, so an import entry finds the symbol
    // pass 1 created for it.
    let by_offset: FxHashMap<u32, SymbolId> = builder
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Import)
        .map(|s| (s.start.offset, s.id))
        .collect();

    for entry in &record.import_entries {
        let kind = match &entry.import_name {
            ImportImportName::Name(n) => ImportKind::Named(n.name.to_string()),
            ImportImportName::NamespaceObject => ImportKind::Namespace,
            ImportImportName::Default(_) => ImportKind::Default,
        };
        imports.push(Import {
            module: input.module,
            specifier: entry.module_request.name.to_string(),
            kind,
            local: by_offset.get(&entry.local_name.span.start).copied(),
            start: lines.location(entry.module_request.span.start as usize),
            type_only: entry.is_type,
        });
    }

    // `export { x } from 'm'` — a name crosses two module boundaries without
    // ever binding locally.
    for entry in &record.indirect_export_entries {
        if let Some(request) = &entry.module_request {
            let name = match &entry.import_name {
                oxc_syntax::module_record::ExportImportName::Name(n) => {
                    ImportKind::Named(n.name.to_string())
                }
                oxc_syntax::module_record::ExportImportName::All
                | oxc_syntax::module_record::ExportImportName::AllButDefault => {
                    ImportKind::StarReExport
                }
                oxc_syntax::module_record::ExportImportName::Null => ImportKind::Namespace,
            };
            imports.push(Import {
                module: input.module,
                specifier: request.name.to_string(),
                kind: name,
                local: None,
                start: lines.location(request.span.start as usize),
                type_only: entry.is_type,
            });
        }
    }

    // `export * from 'm'`.
    for entry in &record.star_export_entries {
        if let Some(request) = &entry.module_request {
            imports.push(Import {
                module: input.module,
                specifier: request.name.to_string(),
                kind: ImportKind::StarReExport,
                local: None,
                start: lines.location(request.span.start as usize),
                type_only: entry.is_type,
            });
        }
    }

    // Bare `import './side-effect'`: requested but binding nothing.
    let bound: FxHashSet<&str> = imports.iter().map(|i| i.specifier.as_str()).collect();
    let bare: Vec<(String, u32)> = record
        .requested_modules
        .iter()
        .filter(|(specifier, _)| !bound.contains(specifier.as_str()))
        .filter_map(|(specifier, requests)| {
            requests
                .first()
                .map(|r| (specifier.to_string(), r.span.start))
        })
        .collect();
    for (specifier, at) in bare {
        imports.push(Import {
            module: input.module,
            specifier,
            kind: ImportKind::SideEffect,
            local: None,
            start: lines.location(at as usize),
            type_only: false,
        });
    }

    // `import('./x')` names a file as surely as a static import does; only
    // `import(expr)` is out of reach. The module record keeps the argument's
    // span, so the literal is read back out of the source.
    for dynamic in &record.dynamic_imports {
        let span = dynamic.module_request;
        let argument = input
            .source
            .get(span.start as usize..span.end as usize)
            .unwrap_or("");
        let literal = argument
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| argument.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
            .filter(|s| !s.contains(['\'', '"', '\\', '$', '`']));
        let (specifier, kind) = match literal {
            Some(literal) => (literal.to_string(), ImportKind::Namespace),
            None => (String::new(), ImportKind::Dynamic),
        };
        imports.push(Import {
            module: input.module,
            specifier,
            kind,
            local: None,
            start: lines.location(span.start as usize),
            type_only: false,
        });
    }

    imports
}

/// Mark the symbols the module exports with the name they are visible under.
///
/// An inline `export function f` names the declaration itself, so the entry's
/// local span is the declaration's. An `export { a as b }` clause names the
/// *clause*, which is somewhere else entirely, so those fall back to matching
/// the local name against the module's top-level declarations.
///
/// A declaration exported under several names (`export { a as b, a as c }`)
/// keeps the first; the others go unreported, which can hide a second dead
/// alias but can never invent one.
fn apply_exports(facts: &mut FileFacts, record: &ModuleRecord<'_>) {
    let mut by_offset: FxHashMap<u32, (String, bool)> = FxHashMap::default();
    let mut by_name: FxHashMap<String, (String, bool)> = FxHashMap::default();
    for entry in &record.local_export_entries {
        let export_name = match &entry.export_name {
            ExportExportName::Name(n) => n.name.to_string(),
            ExportExportName::Default(_) => "default".to_string(),
            ExportExportName::Null => continue,
        };
        let local = match &entry.local_name {
            ExportLocalName::Name(n) | ExportLocalName::Default(n) => n,
            ExportLocalName::Null => continue,
        };
        by_offset
            .entry(local.span.start)
            .or_insert_with(|| (export_name.clone(), entry.is_type));
        by_name
            .entry(local.name.to_string())
            .or_insert((export_name, entry.is_type));
    }

    for symbol in &mut facts.symbols {
        let export = by_offset.get(&symbol.start.offset).or_else(|| {
            symbol
                .flags
                .contains(SymbolFlags::TOP_LEVEL)
                .then(|| by_name.get(&symbol.name))
                .flatten()
        });
        let Some((name, is_type)) = export else {
            continue;
        };
        symbol
            .flags
            .set(SymbolFlags::DEFAULT_EXPORT, name == "default");
        if *is_type {
            symbol.flags.insert(SymbolFlags::TYPE_ONLY);
        }
        symbol.exported_as = Some(name.clone());
    }
}

/// Turn identifier-shaped string literals into weak references. They never
/// mark a symbol used, but the confidence model reads them as a hint that the
/// name may be looked up at runtime.
fn add_string_references(
    facts: &mut FileFacts,
    strings: &FxHashSet<String>,
    module: crate::model::ModuleId,
) {
    facts
        .references
        .extend(strings.iter().map(|name| Reference {
            name: name.clone(),
            module,
            kind: ReferenceKind::String,
            from: None,
            at: Location {
                line: 0,
                column: 0,
                offset: 0,
            },
        }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModuleId;

    fn facts(source: &str, format: &str) -> FileFacts {
        let input = AnalyzeInput {
            module: ModuleId(0),
            format,
            path: "src/a.ts",
            source,
        };
        JsAnalyzer.analyze(&input)
    }

    fn symbol<'a>(facts: &'a FileFacts, name: &str) -> &'a Symbol {
        facts
            .symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no symbol {name} in {:?}", names(facts)))
    }

    fn names(facts: &FileFacts) -> Vec<&str> {
        facts.symbols.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn records_declarations_with_their_kinds() {
        let f = facts(
            "export function a() {}\nclass B {}\nconst c = 1;\ntype D = string;\ninterface E {}\nenum F { X }\nconst g = () => 1;\n",
            "typescript",
        );
        assert_eq!(symbol(&f, "a").kind, SymbolKind::Function);
        assert_eq!(symbol(&f, "B").kind, SymbolKind::Class);
        assert_eq!(symbol(&f, "c").kind, SymbolKind::Variable);
        assert_eq!(symbol(&f, "D").kind, SymbolKind::TypeAlias);
        assert_eq!(symbol(&f, "E").kind, SymbolKind::Interface);
        assert_eq!(symbol(&f, "F").kind, SymbolKind::Enum);
        assert_eq!(symbol(&f, "X").kind, SymbolKind::EnumMember);
        assert_eq!(
            symbol(&f, "g").kind,
            SymbolKind::Function,
            "an arrow bound to a const reads as a function"
        );
    }

    #[test]
    fn exports_carry_the_name_they_are_imported_by() {
        let f = facts(
            "function a() {}\nexport { a as renamed };\nexport default class W {}\nexport const k = 1;\nconst hidden = 2;\n",
            "typescript",
        );
        assert_eq!(symbol(&f, "a").export_name(), Some("renamed"));
        assert_eq!(symbol(&f, "W").export_name(), Some("default"));
        assert!(symbol(&f, "W").flags.contains(SymbolFlags::DEFAULT_EXPORT));
        assert_eq!(symbol(&f, "k").export_name(), Some("k"));
        assert_eq!(symbol(&f, "hidden").export_name(), None);
    }

    #[test]
    fn an_export_clause_is_not_a_use_of_the_symbol() {
        let f = facts("function a() {}\nexport { a };\n", "typescript");
        assert_eq!(
            symbol(&f, "a").local_refs,
            0,
            "`export {{ a }}` must not count as a reference to a"
        );
    }

    #[test]
    fn local_references_are_counted_exactly() {
        let f = facts(
            "function used() {}\nfunction unused() {}\nused(); used();\n",
            "javascript",
        );
        assert_eq!(symbol(&f, "used").local_refs, 2);
        assert_eq!(symbol(&f, "unused").local_refs, 0);
    }

    #[test]
    fn imports_record_specifier_and_shape() {
        let f = facts(
            "import a from './d';\nimport { b } from './n';\nimport * as c from './ns';\nimport './side';\nimport type { T } from './t';\nexport * from './star';\n",
            "typescript",
        );
        let find = |spec: &str| {
            f.imports
                .iter()
                .find(|i| i.specifier == spec)
                .unwrap_or_else(|| panic!("no import of {spec}"))
        };
        assert_eq!(find("./d").kind, ImportKind::Default);
        assert_eq!(find("./n").kind, ImportKind::Named("b".into()));
        assert_eq!(find("./ns").kind, ImportKind::Namespace);
        assert_eq!(find("./side").kind, ImportKind::SideEffect);
        assert!(find("./t").type_only);
        assert_eq!(find("./star").kind, ImportKind::StarReExport);
    }

    #[test]
    fn commonjs_require_forms_become_imports() {
        let f = facts(
            "const path = require('path');\n\
             const { getPlatformKey, PLATFORM_MAP: map } = require('./platform-map');\n\
             const only = require('./util').helper;\n\
             require('./side-effect');\n\
             require.resolve('./resolved');\n\
             path.join(getPlatformKey(), map, only);\n",
            "javascript",
        );
        let find = |spec: &str, kind: &ImportKind| {
            f.imports
                .iter()
                .find(|i| i.specifier == spec && &i.kind == kind)
                .unwrap_or_else(|| panic!("no {kind:?} import of {spec} in {:?}", f.imports))
        };
        find("path", &ImportKind::Namespace);
        find(
            "./platform-map",
            &ImportKind::Named("getPlatformKey".into()),
        );
        find("./platform-map", &ImportKind::Named("PLATFORM_MAP".into()));
        find("./util", &ImportKind::Named("helper".into()));
        find("./side-effect", &ImportKind::SideEffect);
        find("./resolved", &ImportKind::SideEffect);
        assert_eq!(
            f.imports
                .iter()
                .filter(|i| i.specifier == "./platform-map")
                .count(),
            2,
            "a destructuring require is not also recorded as a bare one"
        );
        for name in ["path", "getPlatformKey", "map", "only"] {
            assert_eq!(
                symbol(&f, name).kind,
                SymbolKind::Import,
                "{name} is an import binding to a reader and to the unused-import rule"
            );
        }
        assert!(!f.has_dynamic_access, "every specifier here is a literal");
    }

    #[test]
    fn commonjs_export_forms_mark_the_declarations_they_expose() {
        let f = facts(
            "function getPlatformKey() {}\n\
             const PLATFORM_MAP = {};\n\
             function describeHost() {}\n\
             function hidden() {}\n\
             module.exports = { getPlatformKey, PLATFORM_MAP, describe: describeHost, inline() {} };\n",
            "javascript",
        );
        assert_eq!(
            symbol(&f, "getPlatformKey").export_name(),
            Some("getPlatformKey")
        );
        assert_eq!(
            symbol(&f, "PLATFORM_MAP").export_name(),
            Some("PLATFORM_MAP")
        );
        assert_eq!(
            symbol(&f, "describeHost").export_name(),
            Some("describe"),
            "`describe: describeHost` exports under the key"
        );
        assert_eq!(symbol(&f, "hidden").export_name(), None);

        let single = facts("function run() {}\nmodule.exports = run;\n", "javascript");
        assert_eq!(symbol(&single, "run").export_name(), Some("default"));
        assert!(
            symbol(&single, "run")
                .flags
                .contains(SymbolFlags::DEFAULT_EXPORT)
        );

        let dotted = facts(
            "function a() {}\nfunction b() {}\nexports.a = a;\nmodule.exports.b = b;\n",
            "javascript",
        );
        assert_eq!(symbol(&dotted, "a").export_name(), Some("a"));
        assert_eq!(symbol(&dotted, "b").export_name(), Some("b"));
    }

    #[test]
    fn a_module_exports_object_is_not_a_use_of_what_it_exports() {
        let f = facts(
            "function a() {}\nfunction b() {}\nmodule.exports = { a };\nexports.b = b;\n",
            "javascript",
        );
        assert_eq!(
            symbol(&f, "a").local_refs,
            0,
            "`module.exports = {{ a }}` is not a use of a"
        );
        assert_eq!(
            symbol(&f, "b").local_refs,
            0,
            "`exports.b = b` is not a use of b"
        );
        assert!(
            !f.references
                .iter()
                .any(|r| r.kind == ReferenceKind::Binding && (r.name == "a" || r.name == "b")),
            "and neither creates an edge"
        );
    }

    #[test]
    fn a_module_exports_inside_a_function_is_not_the_files_surface() {
        let f = facts(
            "function plugin(api) { function hook() {} module.exports = { hook }; }\nplugin();\n",
            "javascript",
        );
        assert_eq!(
            symbol(&f, "hook").export_name(),
            None,
            "only the top level decides what a file exports"
        );
    }

    #[test]
    fn a_literal_dynamic_import_is_a_real_edge() {
        let f = facts(
            "import('./lazy');\nimport(\"./also\");\nimport(name);\n",
            "javascript",
        );
        let kinds: Vec<(&str, &ImportKind)> = f
            .imports
            .iter()
            .map(|i| (i.specifier.as_str(), &i.kind))
            .collect();
        assert!(
            kinds.contains(&("./lazy", &ImportKind::Namespace)),
            "{kinds:?}"
        );
        assert!(
            kinds.contains(&("./also", &ImportKind::Namespace)),
            "{kinds:?}"
        );
        assert!(kinds.contains(&("", &ImportKind::Dynamic)), "{kinds:?}");
    }

    #[test]
    fn class_members_are_symbols_with_a_parent() {
        let f = facts(
            "class C {\n  field = 1;\n  method() {}\n  #secret() {}\n  constructor() {}\n}\n",
            "typescript",
        );
        let class = symbol(&f, "C");
        for (name, kind) in [
            ("field", SymbolKind::Property),
            ("method", SymbolKind::Method),
            ("#secret", SymbolKind::Method),
        ] {
            let m = symbol(&f, name);
            assert_eq!(m.kind, kind, "{name}");
            assert_eq!(m.parent, Some(class.id), "{name} must belong to C");
            assert!(m.flags.contains(SymbolFlags::MEMBER));
        }
        assert!(
            symbol(&f, "#secret")
                .flags
                .contains(SymbolFlags::PRIVATE_NAME)
        );
        assert!(
            symbol(&f, "constructor").flags.contains(SymbolFlags::MAGIC),
            "the runtime calls a constructor without anyone naming it"
        );
    }

    #[test]
    fn references_are_attributed_to_the_enclosing_declaration() {
        let f = facts(
            "function outer() { helper(); }\nfunction helper() {}\n",
            "javascript",
        );
        let outer = symbol(&f, "outer").id;
        let call = f
            .references
            .iter()
            .find(|r| r.name == "helper" && r.kind == ReferenceKind::Binding)
            .expect("reference to helper");
        assert_eq!(call.from, Some(outer));
    }

    #[test]
    fn member_accesses_are_recorded_by_name() {
        let f = facts(
            "obj.used(); other['alsoUsed']; obj?.optional;\n",
            "javascript",
        );
        let members: Vec<&str> = f
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::Member)
            .map(|r| r.name.as_str())
            .collect();
        for want in ["used", "alsoUsed", "optional"] {
            assert!(members.contains(&want), "{want} missing from {members:?}");
        }
    }

    #[test]
    fn dynamic_constructs_set_the_flag() {
        for src in [
            "eval('1');",
            "const k = key; obj[k];",
            "require(name);",
            "import(spec);",
        ] {
            assert!(facts(src, "javascript").has_dynamic_access, "{src}");
        }
        assert!(
            !facts("obj.static; require('./x'); import('./y');", "javascript").has_dynamic_access,
            "literal specifiers and static access are not dynamic"
        );
    }

    #[test]
    fn identifier_shaped_strings_become_weak_references() {
        let f = facts("const handlers = { run: 'handleRun' };\n", "javascript");
        assert!(
            f.references
                .iter()
                .any(|r| r.kind == ReferenceKind::String && r.name == "handleRun")
        );
    }

    #[test]
    fn decorated_members_are_flagged() {
        let f = facts(
            "class C {\n  @inject() service;\n  @route('/x') handle() {}\n}\n",
            "typescript",
        );
        assert!(symbol(&f, "service").flags.contains(SymbolFlags::DECORATED));
        assert!(symbol(&f, "handle").flags.contains(SymbolFlags::DECORATED));
    }

    #[test]
    fn jsx_component_names_count_as_uses() {
        let f = facts(
            "import { Widget } from './w';\nexport const App = () => <Widget />;\n",
            "tsx",
        );
        assert!(
            symbol(&f, "Widget").local_refs > 0,
            "a JSX element is a use of the component"
        );
    }

    #[test]
    fn an_unparsable_file_reports_the_failure_rather_than_lying() {
        let f = facts("function ( { { {", "javascript");
        assert!(f.parse_failed);
        assert!(f.symbols.is_empty());
    }

    #[test]
    fn a_sloppy_mode_script_is_parsed_on_the_second_try() {
        // `with` is a syntax error in a module and legal in a script.
        let f = facts(
            "var o = {}; with (o) { x = 1; }\nfunction used() {}\nused();\n",
            "javascript",
        );
        assert!(!f.parse_failed, "a legacy script is not an unparsable file");
        assert_eq!(symbol(&f, "used").local_refs, 1);
    }

    #[test]
    fn an_object_literal_key_is_not_a_member_read() {
        let f = facts("const config = { render: 1 };\n", "javascript");
        assert!(
            !f.references
                .iter()
                .any(|r| r.kind == ReferenceKind::Member && r.name == "render"),
            "`{{ render: 1 }}` defines a property; it does not read one"
        );
    }

    #[test]
    fn module_traits_know_index_files_and_ambient_declarations() {
        let traits = |p: &str| JsAnalyzer.module_traits(p);
        assert!(traits("src/index.ts").package_surface);
        assert!(!traits("src/index.test.ts").package_surface);
        assert!(!traits("src/thing.ts").package_surface);
        assert!(traits("types/global.d.ts").ambient_declarations);
        assert!(traits("x.d.mts").ambient_declarations);
        assert!(!traits("src/a.ts").ambient_declarations);
        assert!(
            !traits("src/index.ts").entry_point,
            "conventions are globs, not traits"
        );
        assert!(!traits("src/a.ts").names_are_attributes);
    }

    // ── resolution ──────────────────────────────────────────────────────

    fn index(files: &[&str]) -> ModuleIndex {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        for (i, f) in files.iter().enumerate() {
            index.insert(PathBuf::from(f), ModuleId(i as u32));
        }
        index
    }

    fn js(files: &[&str], importer: &str, specifier: &str) -> Option<usize> {
        JsAnalyzer
            .resolve(specifier, Path::new(importer), &index(files))
            .map(|m| m.0 as usize)
    }

    #[test]
    fn relative_specifiers_try_extensions_in_typescript_first_order() {
        let files = ["/p/src/a.ts", "/p/src/util.js", "/p/src/util.ts"];
        assert_eq!(js(&files, "/p/src/a.ts", "./util"), Some(2), "util.ts wins");
        assert_eq!(js(&files, "/p/src/a.ts", "./util.js"), Some(1));
    }

    #[test]
    fn a_directory_specifier_resolves_to_its_index() {
        let files = ["/p/src/a.ts", "/p/src/feature/index.tsx"];
        assert_eq!(js(&files, "/p/src/a.ts", "./feature"), Some(1));
    }

    #[test]
    fn parent_segments_are_resolved_without_the_filesystem() {
        let files = ["/p/src/deep/a.ts", "/p/src/shared.ts"];
        assert_eq!(js(&files, "/p/src/deep/a.ts", "../shared"), Some(1));
        assert_eq!(js(&files, "/p/src/deep/a.ts", "./../shared"), Some(1));
        assert_eq!(js(&["/p/a.ts"], "/p/a.ts", "../../../../x"), None);
    }

    #[test]
    fn an_esm_js_specifier_finds_the_typescript_file_it_was_written_against() {
        let files = ["/p/src/a.ts", "/p/src/b.ts"];
        assert_eq!(js(&files, "/p/src/a.ts", "./b.js"), Some(1));
    }

    // ── tsconfig path aliases ───────────────────────────────────────────

    fn with_aliases(files: &[&str], tsconfig: &str) -> ModuleIndex {
        let mut index = index(files);
        index.set_aliases(tsconfig_aliases(Path::new("/p"), tsconfig));
        index
    }

    fn aliased(files: &[&str], tsconfig: &str, importer: &str, specifier: &str) -> Option<usize> {
        JsAnalyzer
            .resolve(
                specifier,
                Path::new(importer),
                &with_aliases(files, tsconfig),
            )
            .map(|m| m.0 as usize)
    }

    #[test]
    fn a_paths_alias_resolves_to_the_file_it_names() {
        let files = ["/p/app/layout.tsx", "/p/components/guard.tsx"];
        let config = r#"{"compilerOptions": {"paths": {"@/*": ["./*"]}}}"#;
        assert_eq!(
            aliased(&files, config, "/p/app/layout.tsx", "@/components/guard"),
            Some(1),
            "the default Next.js alias must reach the component it names"
        );
    }

    #[test]
    fn a_path_pattern_is_not_mistaken_for_a_block_comment() {
        // `"@/*"` contains `/*`. A comment strip that does not track strings
        // eats the rest of the file and yields no aliases at all — which is
        // indistinguishable from a project that declared none.
        let config = r#"{
            // the whole app imports through this
            "compilerOptions": {"paths": {"@/*": ["./src/*"]}}
        }"#;
        let aliases = tsconfig_aliases(Path::new("/p"), config);
        assert_eq!(aliases.len(), 1, "got {aliases:?}");
        assert_eq!(aliases[0].prefix, "@/");
        assert_eq!(aliases[0].targets, vec![PathBuf::from("/p/src")]);
    }

    #[test]
    fn comments_and_trailing_commas_do_not_stop_a_tsconfig_being_read() {
        let config = r#"{
            /* generated by the framework */
            "compilerOptions": {
                "baseUrl": ".", // where paths start
                "paths": {
                    "@/*": ["./src/*"],
                },
            },
        }"#;
        let aliases = tsconfig_aliases(Path::new("/p"), config);
        assert_eq!(aliases.len(), 1, "got {aliases:?}");
    }

    #[test]
    fn base_url_moves_where_the_targets_point() {
        let files = ["/p/a.ts", "/p/src/lib/x.ts"];
        let config = r#"{"compilerOptions": {"baseUrl": "./src", "paths": {"~/*": ["lib/*"]}}}"#;
        assert_eq!(aliased(&files, config, "/p/a.ts", "~/x"), Some(1));
    }

    #[test]
    fn a_pattern_without_a_wildcard_matches_exactly() {
        let files = ["/p/a.ts", "/p/src/config.ts"];
        let config = r#"{"compilerOptions": {"paths": {"@config": ["./src/config.ts"]}}}"#;
        assert_eq!(aliased(&files, config, "/p/a.ts", "@config"), Some(1));
        assert_eq!(
            aliased(&files, config, "/p/a.ts", "@config/extra"),
            None,
            "an exact pattern must not match a longer specifier"
        );
    }

    #[test]
    fn a_wildcard_stands_for_at_least_one_character() {
        let files = ["/p/a.ts", "/p/index.ts"];
        let config = r#"{"compilerOptions": {"paths": {"@/*": ["./*"]}}}"#;
        assert_eq!(
            aliased(&files, config, "/p/a.ts", "@"),
            None,
            "a bare `@` is a package name, not the alias root"
        );
    }

    #[test]
    fn several_targets_are_tried_in_the_order_declared() {
        let files = ["/p/a.ts", "/p/second/x.ts"];
        let config = r#"{"compilerOptions": {"paths": {"~/*": ["./first/*", "./second/*"]}}}"#;
        assert_eq!(aliased(&files, config, "/p/a.ts", "~/x"), Some(1));
    }

    #[test]
    fn an_alias_applies_only_under_the_directory_that_declared_it() {
        let mut index = index(&["/p/one/a.ts", "/p/one/src/x.ts", "/p/two/b.ts"]);
        index.set_aliases(tsconfig_aliases(
            Path::new("/p/one"),
            r#"{"compilerOptions": {"paths": {"@/*": ["./src/*"]}}}"#,
        ));
        let resolve = |importer: &str| {
            JsAnalyzer
                .resolve("@/x", Path::new(importer), &index)
                .map(|m| m.0 as usize)
        };
        assert_eq!(resolve("/p/one/a.ts"), Some(1));
        assert_eq!(
            resolve("/p/two/b.ts"),
            None,
            "a sibling package must not borrow its neighbour's aliases"
        );
    }

    #[test]
    fn the_most_specific_alias_wins() {
        let mut index = index(&["/p/a.ts", "/p/wide/ui/button.ts", "/p/narrow/button.ts"]);
        index.set_aliases(tsconfig_aliases(
            Path::new("/p"),
            r#"{"compilerOptions": {"paths": {
                "@/*": ["./wide/*"],
                "@/ui/*": ["./narrow/*"]
            }}}"#,
        ));
        assert_eq!(
            JsAnalyzer
                .resolve("@/ui/button", Path::new("/p/a.ts"), &index)
                .map(|m| m.0 as usize),
            Some(2),
            "the longer prefix must be tried first"
        );
    }

    #[test]
    fn paths_are_inherited_through_a_relative_extends() {
        // A monorepo package usually carries only `extends`, and the aliases
        // live in the shared base beside the lockfile.
        let dir = std::env::temp_dir().join(format!("basta-extends-{}", std::process::id()));
        let package = dir.join("packages/app");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            dir.join("tsconfig.base.json"),
            r#"{"compilerOptions": {"paths": {"@shared/*": ["./shared/*"]}}}"#,
        )
        .unwrap();
        let aliases = tsconfig_aliases(&package, r#"{"extends": "../../tsconfig.base.json"}"#);
        assert_eq!(aliases.len(), 1, "got {aliases:?}");
        assert_eq!(
            aliases[0].targets,
            vec![dir.join("shared")],
            "an inherited target resolves against the base config's directory"
        );
        assert_eq!(
            aliases[0].scope, package,
            "but it applies to the package that extended it"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_extends_cycle_stops_instead_of_looping() {
        let dir = std::env::temp_dir().join(format!("basta-extends-cycle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), r#"{"extends": "./b.json"}"#).unwrap();
        std::fs::write(dir.join("b.json"), r#"{"extends": "./a.json"}"#).unwrap();
        assert!(tsconfig_aliases(&dir, r#"{"extends": "./a.json"}"#).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_package_extends_is_not_followed() {
        // It lives in node_modules, which is never scanned.
        assert!(tsconfig_aliases(Path::new("/p"), r#"{"extends": "@tsconfig/node20"}"#).is_empty());
    }

    #[test]
    fn a_malformed_tsconfig_yields_no_aliases_instead_of_failing() {
        assert!(tsconfig_aliases(Path::new("/p"), "not json at all").is_empty());
        assert!(tsconfig_aliases(Path::new("/p"), "{}").is_empty());
        assert!(tsconfig_aliases(Path::new("/p"), r#"{"compilerOptions":{}}"#).is_empty());
    }

    #[test]
    fn an_alias_does_not_make_a_scoped_package_resolve() {
        let files = ["/p/src/a.ts", "/p/scope/pkg.ts"];
        let config = r#"{"compilerOptions": {"paths": {"@/*": ["./*"]}}}"#;
        assert_eq!(
            aliased(&files, config, "/p/src/a.ts", "@scope/pkg"),
            None,
            "`@scope/pkg` is a dependency; only the `@/` prefix is aliased"
        );
    }

    // ── files named by a string rather than imported ────────────────────

    #[test]
    fn a_path_shaped_string_literal_is_a_reference() {
        for value in [
            "./chunk-worker.ts",
            "../producer/src/services/worker.ts",
            "src/runtime/entry.ts",
        ] {
            assert_eq!(
                module_path_literal(value).as_deref(),
                Some(value),
                "{value} names a source file"
            );
        }
    }

    #[test]
    fn prose_and_urls_do_not_become_references() {
        for value in [
            "hello world",
            "entry.ts",
            "https://example.com/app.js",
            "./assets",
            "text/plain",
            "1.5",
        ] {
            assert_eq!(module_path_literal(value), None, "{value} names no file");
        }
    }

    #[test]
    fn a_config_that_names_a_setup_file_imports_it() {
        let f = facts(
            r#"export default { test: { setupFiles: ["./src/setup.ts"] } };"#,
            "typescript",
        );
        assert!(
            f.imports.iter().any(
                |i| i.specifier == "./src/setup.ts" && matches!(i.kind, ImportKind::SideEffect)
            ),
            "a setup file listed in a config is used by it, got {:?}",
            f.imports
        );
    }

    #[test]
    fn bare_package_specifiers_do_not_resolve() {
        let files = ["/p/src/a.ts", "/p/react.ts", "/p/lodash/index.ts"];
        assert_eq!(js(&files, "/p/src/a.ts", "react"), None);
        assert_eq!(js(&files, "/p/src/a.ts", "@scope/pkg"), None);
    }

    #[test]
    fn a_path_like_bare_specifier_is_tried_against_the_scan_roots() {
        let files = ["/p/src/a.ts", "/p/src/shared/util.ts"];
        assert_eq!(js(&files, "/p/src/a.ts", "src/shared/util"), Some(1));
        assert_eq!(js(&files, "/p/src/a.ts", "./missing"), None);
    }

    // ── manifests ───────────────────────────────────────────────────────

    #[test]
    fn package_json_fields_become_entry_paths() {
        let json = serde_json::json!({
            "main": "./dist/index.js",
            "types": "./dist/index.d.ts",
            "bin": { "tool": "./bin/cli.js" },
            "exports": {
                ".": { "import": "./dist/esm.js", "require": "./dist/cjs.js" },
                "./sub": "./dist/sub.js"
            },
            "files": ["run-cpd.js", "dist", "src/**/*.js"],
            "scripts": { "build": "tsx scripts/build.ts --watch", "lint": "biome check ." },
            "name": "pkg",
            "version": "1.0.0"
        });
        let mut found = package_json_entries(&json);
        found.sort();
        assert_eq!(
            found,
            vec![
                "./bin/cli.js",
                "./dist/cjs.js",
                "./dist/esm.js",
                "./dist/index.d.ts",
                "./dist/index.js",
                "./dist/sub.js",
                "run-cpd.js",
                "scripts/build.ts",
            ],
            "the name, the version, a directory and a glob are not paths"
        );
    }

    #[test]
    fn a_built_entry_path_maps_back_to_the_source_it_came_from() {
        let candidates = source_candidates(Path::new("/p/dist/index.js"));
        for want in [
            "/p/dist/index.js",
            "/p/dist/index.ts",
            "/p/index.ts",
            "/p/src/index.ts",
            "/p/src/index.tsx",
        ] {
            assert!(candidates.contains(&PathBuf::from(want)), "{want} missing");
        }
        assert!(
            source_candidates(Path::new("/p/dist/config.d.ts"))
                .contains(&PathBuf::from("/p/src/config.ts")),
            "config.d.ts was generated from config.ts, not config.d.tsx"
        );
        assert_eq!(
            strip_output_dir(Path::new("/p/dist/a/b.js")),
            Some(PathBuf::from("/p/a/b.js"))
        );
        assert_eq!(strip_output_dir(Path::new("/p/src/a.ts")), None);
    }

    #[test]
    fn manifest_entries_go_through_the_trait() {
        let entries = JsAnalyzer.manifest_entries(
            Path::new("/pkg"),
            "package.json",
            r#"{"main": "./dist/index.js"}"#,
        );
        assert!(entries.contains(&PathBuf::from("/pkg/src/index.ts")));
        assert!(
            JsAnalyzer
                .manifest_entries(Path::new("/pkg"), "package.json", "not json")
                .is_empty()
        );
    }

    #[test]
    fn locations_are_resolved_to_lines() {
        let f = facts("\n\nexport function a() {\n  return 1;\n}\n", "javascript");
        let a = symbol(&f, "a");
        assert_eq!(a.start.line, 3);
        assert_eq!(a.end.line, 5);
        assert_eq!(a.lines, 3);
    }
}
