//! Python, through the ruff parser.
//!
//! Python has no `export` keyword, so the module boundary has to be inferred
//! from convention, and the convention is unusually clear: a module's public
//! surface is `__all__` when it declares one, and otherwise every top-level
//! name that does not begin with an underscore. Anything else — an `_helper`,
//! a nested function, a name a module declares but keeps out of `__all__` —
//! is module-private, and dying unnoticed inside one file rather than across
//! the project.
//!
//! The other half of the problem is that Python reaches names at runtime.
//! `getattr`, `globals()`, `__import__` and a bare string in a router table
//! can all keep a declaration alive with nothing an analyzer would call a
//! reference. Those constructs are recorded rather than resolved: they lower
//! the confidence of every finding in the file instead of silently deciding
//! it either way.

use super::{AnalyzeInput, Analyzer, is_identifier_like};
use crate::model::{
    FileFacts, Import, ImportKind, ModuleId, ModuleTraits, Reference, ReferenceKind, Symbol,
    SymbolFlags, SymbolId, SymbolKind,
};
use crate::resolve::{ModuleIndex, append_extension};
use cpd_core::models::Location;
use cpd_tokenizer::line_index::LineIndex;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor};
use ruff_python_ast::{
    Alias, Decorator, Expr, ExprContext, Identifier, Stmt, StmtClassDef, StmtFunctionDef,
};
use ruff_python_parser::{parse_module, parse_string_annotation};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

pub struct PythonAnalyzer;

impl Analyzer for PythonAnalyzer {
    fn language(&self) -> &'static str {
        "python"
    }

    fn formats(&self) -> &'static [&'static str] {
        &["python"]
    }

    fn analyze(&self, input: &AnalyzeInput<'_>) -> FileFacts {
        analyze_python(input)
    }

    fn resolve(&self, specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId> {
        resolve_python(specifier, importer, index)
    }

    /// `from pkg import thing` is ambiguous: `thing` is a name in
    /// `pkg/__init__.py`, or it is `pkg/thing.py`. Only the index can say, so
    /// the submodule is tried first — when it exists, the import binds the
    /// module object, and everything in it becomes reachable as an attribute.
    fn normalize_import(&self, import: &mut Import, importer: &Path, index: &ModuleIndex) {
        let ImportKind::Named(name) = &import.kind else {
            return;
        };
        let submodule = if import.specifier.ends_with('.') {
            format!("{}{name}", import.specifier)
        } else {
            format!("{}.{name}", import.specifier)
        };
        if resolve_python(&submodule, importer, index).is_some() {
            import.specifier = submodule;
            import.kind = ImportKind::Namespace;
        }
    }

    fn entry_globs(&self) -> &'static [&'static str] {
        &[
            "**/__main__.py",
            "**/manage.py",
            "**/wsgi.py",
            "**/asgi.py",
            "**/setup.py",
            "**/conftest.py",
            "**/settings.py",
            "**/urls.py",
            "**/celery.py",
            "**/tasks.py",
            // A module with one of these names is started by a person or a
            // process manager, never imported by a sibling.
            "**/cli.py",
            "**/main.py",
            "**/app.py",
            "**/run.py",
            "**/server.py",
            "**/worker.py",
        ]
    }

    fn test_globs(&self) -> &'static [&'static str] {
        &["**/test_*.py", "**/*_test.py", "**/conftest.py"]
    }

    /// A shebang, or a `__main__` guard. The guard has to look like the
    /// comparison itself — `__name__ ==` — not merely two strings a docstring
    /// might mention.
    fn is_self_starting(&self, source: &str) -> bool {
        source.starts_with("#!")
            || source.split("__name__").skip(1).any(|after| {
                let after = after.trim_start();
                after.starts_with("==") && after.contains("__main__")
            })
    }

    fn manifests(&self) -> &'static [&'static str] {
        &["pyproject.toml"]
    }

    fn manifest_entries(&self, directory: &Path, _manifest: &str, text: &str) -> Vec<PathBuf> {
        pyproject_entry_modules(text)
            .iter()
            .flat_map(|module| module_candidates(directory, module))
            .collect()
    }

    fn import_roots(&self, modules: &[PathBuf]) -> Vec<PathBuf> {
        package_parents(modules)
    }

    fn module_traits(&self, path: &str) -> ModuleTraits {
        let is_init = path.ends_with("__init__.py");
        ModuleTraits {
            // A package's `__init__.py` is its published surface; nothing
            // inside the package needs to import it for that to be true.
            entry_point: is_init,
            package_surface: is_init,
            ambient_declarations: false,
            // `mod.helper()` reaches a module-level `helper` with no import
            // naming it — including when the module arrives as a parameter,
            // which no static analysis will resolve.
            names_are_attributes: true,
        }
    }
}

// ── resolution ──────────────────────────────────────────────────────────────

/// `from .pkg import x` (leading dots) or `import pkg.mod` (dotted).
fn resolve_python(specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId> {
    let dots = specifier.chars().take_while(|c| *c == '.').count();
    let segments: Vec<&str> = specifier[dots..]
        .split('.')
        .filter(|s| !s.is_empty())
        .collect();

    if dots > 0 {
        // `.` is the importer's own package; each extra dot climbs one
        // package above it.
        let mut base = importer.parent()?.to_path_buf();
        for _ in 1..dots {
            base = base.parent()?.to_path_buf();
        }
        return candidates(index, &base, &segments);
    }
    if segments.is_empty() {
        return None;
    }
    // An absolute import is resolved from the package root the importer lives
    // in — the highest directory still holding an `__init__.py` — and then
    // from each scan root, which is what makes `src/` layouts and flat layouts
    // both work.
    if let Some(package_root) = package_root(importer)
        && let Some(id) = candidates(index, &package_root, &segments)
    {
        return Some(id);
    }
    if let Some(id) = index
        .roots()
        .iter()
        .find_map(|root| candidates(index, root, &segments))
    {
        return Some(id);
    }
    // Finally the roots the tree itself implies. A `src/` layout puts every
    // package under `src`, so a file outside it — a maintenance script in
    // `utils/` — resolves `import mypkg.thing` through no scan root and no
    // package of its own.
    index
        .import_roots()
        .iter()
        .find_map(|root| candidates(index, root, &segments))
}

/// The parent directory of every top-level package in the scan.
///
/// A directory holding `__init__.py` is a package; climbing while the parent
/// is also one lands on the outermost, and *its* parent is what an absolute
/// import is resolved against. For `src/mypkg/__init__.py` that is `src`.
fn package_parents(modules: &[PathBuf]) -> Vec<PathBuf> {
    let packages: FxHashSet<&Path> = modules
        .iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "__init__.py" || name == "__init__.pyi")
        })
        .filter_map(|path| path.parent())
        .collect();
    let mut roots: FxHashSet<PathBuf> = FxHashSet::default();
    for package in &packages {
        let mut outermost = *package;
        while let Some(parent) = outermost.parent() {
            if !packages.contains(parent) {
                break;
            }
            outermost = parent;
        }
        if let Some(parent) = outermost.parent()
            && !parent.as_os_str().is_empty()
        {
            roots.insert(parent.to_path_buf());
        }
    }
    roots.into_iter().collect()
}

/// `base/a/b.py` or `base/a/b/__init__.py` for segments `["a", "b"]`.
///
/// An empty segment list means the package directory itself, which is what
/// `from . import x` names.
fn candidates(index: &ModuleIndex, base: &Path, segments: &[&str]) -> Option<ModuleId> {
    let mut path = base.to_path_buf();
    for segment in segments {
        path.push(segment);
    }
    index
        .get(&append_extension(&path, "py"))
        .or_else(|| index.get(&path.join("__init__.py")))
        // A stub-only module still proves the import target exists.
        .or_else(|| index.get(&append_extension(&path, "pyi")))
}

/// The directory an absolute import would be resolved from: the parent of the
/// outermost package the file belongs to.
///
/// This is the one place resolution looks at the filesystem rather than the
/// index. A scan of `pkg/sub/` alone still has to know that `pkg/__init__.py`
/// exists above it, or `import pkg.sub.x` could never be resolved.
fn package_root(importer: &Path) -> Option<PathBuf> {
    let mut directory = importer.parent()?.to_path_buf();
    while directory.join("__init__.py").exists() {
        match directory.parent() {
            Some(parent) => directory = parent.to_path_buf(),
            None => break,
        }
    }
    Some(directory)
}

// ── manifests ───────────────────────────────────────────────────────────────

/// `pyproject.toml` tables whose values name a module that gets executed.
const ENTRY_TABLES: &[&str] = &[
    "project.scripts",
    "project.gui-scripts",
    "tool.poetry.scripts",
];

/// Dotted module paths named by a `pyproject.toml`'s entry-point tables.
///
/// Read with a small scanner rather than a TOML parser: the tables involved
/// hold nothing but `name = "pkg.module:function"` lines, and a file basta
/// cannot make sense of simply contributes no entry points — the same outcome
/// as a file that declares none. An entry-points table of any depth
/// (`[project.entry-points."console_scripts"]`) counts too, because a plugin
/// registered there is loaded by name and imported by nothing.
fn pyproject_entry_modules(text: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut in_entry_table = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let header = header.trim();
            in_entry_table = ENTRY_TABLES.contains(&header)
                || header.starts_with("project.entry-points")
                || header.starts_with("tool.poetry.plugins");
            continue;
        }
        if !in_entry_table || line.starts_with('#') {
            continue;
        }
        let Some((_, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'', ' ', ',']);
        // `pkg.module:function` — the module is everything before the colon.
        let module = value.split(':').next().unwrap_or(value).trim();
        if !module.is_empty()
            && module
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '_')
        {
            modules.push(module.to_string());
        }
    }
    modules
}

/// Files a dotted module path could live in, under `base` and under a `src/`
/// layout.
fn module_candidates(base: &Path, dotted: &str) -> Vec<PathBuf> {
    let segments: Vec<&str> = dotted.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(4);
    for root in [base.to_path_buf(), base.join("src")] {
        let mut path = root;
        for segment in &segments {
            path.push(segment);
        }
        out.push(path.with_extension("py"));
        out.push(path.join("__init__.py"));
    }
    out
}

// ── parsing ─────────────────────────────────────────────────────────────────

fn analyze_python(input: &AnalyzeInput<'_>) -> FileFacts {
    let Ok(parsed) = parse_module(input.source) else {
        return FileFacts::unparsed();
    };
    let body = &parsed.syntax().body;
    let lines = LineIndex::new(input.source.as_bytes());

    // `__all__` decides the module's public surface when it is present, so it
    // has to be known before the first declaration is classified.
    let dunder_all = collect_dunder_all(body);
    let is_package_surface = input.path.ends_with("__init__.py");

    let mut walk = Walk {
        symbols: Vec::new(),
        imports: Vec::new(),
        references: Vec::new(),
        module: input.module,
        lines: &lines,
        len: input.source.len(),
        stack: Vec::new(),
        class_stack: Vec::new(),
        dunder_all: dunder_all.as_ref(),
        is_package_surface,
        dynamic: false,
        in_annotation: 0,
        strings: FxHashSet::default(),
        declared: FxHashMap::default(),
        source: input.source,
    };
    walk.visit_body(body);

    let Walk {
        mut symbols,
        imports,
        mut references,
        dynamic,
        strings,
        ..
    } = walk;

    count_local_references(&mut symbols, &references);
    references.extend(strings.into_iter().map(|name| Reference {
        module: input.module,
        name,
        kind: ReferenceKind::String,
        from: None,
        at: Location {
            line: 0,
            column: 0,
            offset: 0,
        },
    }));

    FileFacts {
        symbols,
        imports,
        references,
        has_dynamic_access: dynamic,
        parse_failed: false,
    }
}

/// The names in a module-level `__all__ = [...]` / `__all__ += [...]`, or
/// `None` when the module declares none. An `__all__` built by anything other
/// than a list or tuple of string literals is treated as absent: guessing at
/// a computed public surface would be worse than falling back to convention.
fn collect_dunder_all(body: &[Stmt]) -> Option<FxHashSet<String>> {
    let mut names: Option<FxHashSet<String>> = None;
    let push = |value: &Expr, names: &mut Option<FxHashSet<String>>| {
        let elements = match value {
            Expr::List(l) => &l.elts,
            Expr::Tuple(t) => &t.elts,
            _ => return,
        };
        let set = names.get_or_insert_with(FxHashSet::default);
        for element in elements {
            if let Expr::StringLiteral(s) = element {
                set.insert(s.value.to_str().to_string());
            }
        }
    };
    for stmt in body {
        match stmt {
            Stmt::Assign(a) if targets_dunder_all(&a.targets) => push(&a.value, &mut names),
            Stmt::AugAssign(a) if is_dunder_all(&a.target) => push(&a.value, &mut names),
            Stmt::AnnAssign(a) if is_dunder_all(&a.target) => {
                if let Some(value) = &a.value {
                    push(value, &mut names);
                }
            }
            _ => {}
        }
    }
    names
}

fn targets_dunder_all(targets: &[Expr]) -> bool {
    targets.iter().any(is_dunder_all)
}

fn is_dunder_all(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(n) if n.id.as_str() == "__all__")
}

/// Count references to each declaration from within its own module.
///
/// Python has no binding resolution here, so this matches by name. A nested
/// `x` shadowing a module-level `x` inflates the outer one's count, which can
/// only ever hide a finding, never invent one.
fn count_local_references(symbols: &mut [Symbol], references: &[Reference]) {
    let mut counts: FxHashMap<&str, u32> = FxHashMap::default();
    for reference in references {
        if reference.kind == ReferenceKind::Binding {
            *counts.entry(reference.name.as_str()).or_default() += 1;
        }
    }
    let mut member_counts: FxHashMap<&str, u32> = FxHashMap::default();
    for reference in references {
        if reference.kind == ReferenceKind::Member {
            *member_counts.entry(reference.name.as_str()).or_default() += 1;
        }
    }
    for symbol in symbols {
        let table = if symbol.kind.is_member() {
            &member_counts
        } else {
            &counts
        };
        symbol.local_refs = table.get(symbol.name.as_str()).copied().unwrap_or(0);
    }
}

struct Walk<'a> {
    symbols: Vec<Symbol>,
    imports: Vec<Import>,
    references: Vec<Reference>,
    module: crate::model::ModuleId,
    lines: &'a LineIndex,
    len: usize,
    /// Declarations currently open, innermost last, each flagged with whether
    /// it is one a reference can be attributed to. See [`Walk::current`].
    stack: Vec<(SymbolId, bool)>,
    /// Enclosing class declarations, so members get a parent.
    class_stack: Vec<SymbolId>,
    dunder_all: Option<&'a FxHashSet<String>>,
    is_package_surface: bool,
    dynamic: bool,
    strings: FxHashSet<String>,
    /// Names already declared, keyed by the innermost open declaration and
    /// the name, so a rebinding in the same scope updates the declaration
    /// that is already there instead of adding a rival — and a nested `def
    /// helper` does not collide with a top-level one.
    declared: FxHashMap<(Option<SymbolId>, String), SymbolId>,
    source: &'a str,
    /// How many annotations enclose the expression being visited. Inside one,
    /// a string literal is a forward reference rather than data.
    in_annotation: u32,
}

impl Walk<'_> {
    fn loc(&self, offset: u32) -> Location {
        self.lines.location((offset as usize).min(self.len))
    }

    /// The declaration a reference at this point belongs to.
    ///
    /// The innermost *reportable* one, not simply the innermost: a nested
    /// `def` is never reported, so nothing can ever reach it, and a call made
    /// inside one has to count as a call by the declaration that contains it.
    fn current(&self) -> Option<SymbolId> {
        self.stack
            .iter()
            .rev()
            .find_map(|(id, reportable)| reportable.then_some(*id))
    }

    /// Open a declaration, recording whether references inside it belong to
    /// it or to whatever encloses it.
    fn open(&mut self, id: SymbolId) {
        let symbol = &self.symbols[id.0 as usize];
        let reportable = symbol
            .flags
            .intersects(SymbolFlags::TOP_LEVEL | SymbolFlags::MEMBER);
        self.stack.push((id, reportable));
    }

    /// Whether the line holding `offset` carries a `# noqa` that covers the
    /// unused-import rule — either bare, or naming F401 among its codes.
    fn line_silences_unused_import(&self, offset: u32) -> bool {
        let start = self.source[..(offset as usize).min(self.len)]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = self.source[start..]
            .find('\n')
            .map_or(self.source.len(), |index| start + index);
        let Some(comment) = self.source[start..end].split_once('#') else {
            return false;
        };
        let comment = comment.1.trim();
        let Some(rest) = comment
            .strip_prefix("noqa")
            .or_else(|| comment.strip_prefix("NOQA"))
        else {
            return false;
        };
        // A bare `# noqa` silences everything; a list only silences its codes.
        rest.trim_start().strip_prefix(':').is_none_or(|codes| {
            codes
                .split(',')
                .any(|code| code.trim().eq_ignore_ascii_case("f401"))
        })
    }

    fn in_class_body(&self) -> bool {
        // A method's own body opens a function scope, so "directly inside a
        // class" means the innermost open declaration is that class.
        match (self.stack.last(), self.class_stack.last()) {
            (Some((open, _)), Some(class)) => open == class,
            _ => false,
        }
    }

    /// Whether a top-level name is part of the module's public surface, and
    /// the name it is visible under.
    ///
    /// An import is held to a stricter rule than a declaration. Every module
    /// technically re-exports everything it imports, so treating imports like
    /// declarations would make `import os` part of a module's API. Only the
    /// two places where a re-export is *deliberate* count: a package's
    /// `__init__.py`, whose imports are the package surface, and a name the
    /// module put in `__all__`.
    fn export_name(
        &self,
        name: &str,
        kind: SymbolKind,
        top_level: bool,
        explicit_re_export: bool,
    ) -> Option<String> {
        if !top_level {
            return None;
        }
        if kind == SymbolKind::Import {
            let deliberate = self.is_package_surface
                || self.dunder_all.is_some_and(|all| all.contains(name))
                || explicit_re_export;
            return deliberate.then(|| name.to_string());
        }

        match self.dunder_all {
            // An explicit `__all__` is the whole answer: a name outside it is
            // private however it is spelled.
            Some(all) => all.contains(name).then(|| name.to_string()),
            // Otherwise the underscore convention decides, except in a package
            // `__init__.py`, where re-exported private names are still the
            // package's surface.
            None => (!name.starts_with('_') || self.is_package_surface).then(|| name.to_string()),
        }
    }

    fn declare(
        &mut self,
        name: String,
        kind: SymbolKind,
        name_start: u32,
        end: u32,
        mut flags: SymbolFlags,
    ) -> SymbolId {
        let top_level = self.stack.is_empty();
        let member = self.in_class_body();
        flags.set(SymbolFlags::TOP_LEVEL, top_level);
        flags.set(SymbolFlags::MEMBER, member);
        flags.set(SymbolFlags::PRIVATE_NAME, name.starts_with('_'));
        flags.set(SymbolFlags::MAGIC, is_magic(&name));
        flags.set(
            SymbolFlags::IN_DUNDER_ALL,
            self.dunder_all.is_some_and(|all| all.contains(&name)),
        );
        let explicit_re_export = flags.contains(SymbolFlags::RE_EXPORT);
        let exported_as = self.export_name(&name, kind, top_level, explicit_re_export);
        let name_key = name.clone();
        let start = self.loc(name_start);
        let end = self.loc(end);
        let parent = member.then(|| self.class_stack.last().copied()).flatten();
        let scope = self.stack.last().map(|(id, _)| *id);

        // `WEIGHTS = {...}` followed by `WEIGHTS = tuned()` is one name bound
        // twice, not two declarations. Recording both would leave the first
        // with no references — every reader resolves to the last one — and
        // report a variable the module uses on every line. The scope is part
        // of the key: a `def helper` nested in one function is not the
        // top-level `helper`, and must not swallow its references.
        if let Some(existing) = self.declared.get(&(scope, name.clone())).copied() {
            let symbol = &mut self.symbols[existing.0 as usize];
            symbol.end = end;
            symbol.lines = symbol.end.line.saturating_sub(symbol.start.line) + 1;
            symbol.flags |= flags;
            if symbol.exported_as.is_none() {
                symbol.exported_as = exported_as;
            }
            return existing;
        }

        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id,
            module: self.module,
            name,
            kind,
            flags,
            lines: end.line.saturating_sub(start.line) + 1,
            start,
            end,
            exported_as,
            parent,
            local_refs: 0,
        });
        self.declared.insert((scope, name_key), id);
        id
    }

    fn reference(&mut self, name: String, kind: ReferenceKind, at: u32) {
        let from = self.current();
        let at = self.loc(at);
        self.references.push(Reference {
            module: self.module,
            name,
            kind,
            from,
            at,
        });
    }

    fn function(&mut self, f: &StmtFunctionDef) {
        // Decorators are evaluated in the enclosing scope, so they are visited
        // before the function joins the stack: a decorated dead function must
        // not be the thing keeping its own decorator alive.
        for decorator in &f.decorator_list {
            self.visit_decorator(decorator);
        }
        let kind = if self.in_class_body() {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        };
        let mut flags = SymbolFlags::NONE;
        let decorators: Vec<String> = f.decorator_list.iter().map(decorator_name).collect();
        flags.set(
            SymbolFlags::DECORATED,
            decorators.iter().any(|d| !is_inert_decorator(d)),
        );
        flags.set(
            SymbolFlags::ABSTRACT,
            decorators.iter().any(|d| d.contains("abstractmethod")),
        );
        flags.set(
            SymbolFlags::OVERRIDE,
            decorators.iter().any(|d| d.ends_with("override")),
        );
        let id = self.declare(
            f.name.to_string(),
            kind,
            f.name.range.start().to_u32(),
            f.range.end().to_u32(),
            flags,
        );

        self.open(id);
        if let Some(type_params) = &f.type_params {
            self.visit_type_params(type_params);
        }
        self.visit_parameters(&f.parameters);
        if let Some(returns) = &f.returns {
            self.visit_annotation(returns);
        }
        self.visit_body(&f.body);
        self.stack.pop();
    }

    fn class(&mut self, c: &StmtClassDef) {
        for decorator in &c.decorator_list {
            self.visit_decorator(decorator);
        }
        let mut flags = SymbolFlags::NONE;
        flags.set(
            SymbolFlags::DECORATED,
            c.decorator_list
                .iter()
                .map(decorator_name)
                .any(|d| !is_inert_decorator(&d)),
        );
        let id = self.declare(
            c.name.to_string(),
            SymbolKind::Class,
            c.name.range.start().to_u32(),
            c.range.end().to_u32(),
            flags,
        );

        self.open(id);
        self.class_stack.push(id);
        if let Some(type_params) = &c.type_params {
            self.visit_type_params(type_params);
        }
        if let Some(arguments) = &c.arguments {
            self.visit_arguments(arguments);
        }
        self.visit_body(&c.body);
        self.class_stack.pop();
        self.stack.pop();
    }

    /// `import a.b.c` / `import a.b as ab` — the binding is the first segment
    /// unless the import is aliased.
    fn import(&mut self, alias: &Alias) {
        let full = alias.name.as_str();
        let (local_name, local_start) = match &alias.asname {
            Some(as_name) => (as_name.to_string(), as_name.range.start().to_u32()),
            None => (
                full.split('.').next().unwrap_or(full).to_string(),
                alias.name.range.start().to_u32(),
            ),
        };
        let mut flags = SymbolFlags::TOP_LEVEL;
        let redundant_alias = alias.asname.as_ref().is_some_and(|a| a.as_str() == full);
        if redundant_alias || self.line_silences_unused_import(alias.range.start().to_u32()) {
            flags.insert(SymbolFlags::RE_EXPORT);
        }
        let local = self.declare(
            local_name,
            SymbolKind::Import,
            local_start,
            alias.range.end().to_u32(),
            flags,
        );
        let start = self.loc(alias.range.start().to_u32());
        self.imports.push(Import {
            module: self.module,
            specifier: full.to_string(),
            kind: ImportKind::Namespace,
            local: Some(local),
            start,
            type_only: false,
        });
    }

    /// `from .pkg import a, b as c` / `from . import x` / `from m import *`.
    ///
    /// The specifier keeps the leading dots so [`crate::resolve`] can tell a
    /// relative import from an absolute one without a second field.
    fn import_from(&mut self, level: u32, module: Option<&Identifier>, names: &[Alias], end: u32) {
        let specifier = format!(
            "{}{}",
            ".".repeat(level as usize),
            module.map(Identifier::as_str).unwrap_or("")
        );
        // `from __future__ import annotations` is a compiler directive. It
        // binds a name that no code can use and that every file needs, so
        // reporting it as unused would fire on half a modern codebase.
        if specifier == "__future__" {
            return;
        }
        for alias in names {
            let imported = alias.name.as_str();
            if imported == "*" {
                let start = self.loc(alias.range.start().to_u32());
                self.imports.push(Import {
                    module: self.module,
                    specifier: specifier.clone(),
                    kind: ImportKind::StarReExport,
                    local: None,
                    start,
                    type_only: false,
                });
                continue;
            }
            let (local_name, local_start) = match &alias.asname {
                Some(as_name) => (as_name.to_string(), as_name.range.start().to_u32()),
                None => (imported.to_string(), alias.name.range.start().to_u32()),
            };
            // Two conventions say "this import exists to be re-exported":
            // PEP 484's redundant alias (`from m import x as x`) and a
            // `# noqa: F401` silencing the linter that would flag it. Both
            // are deliberate, and both mean the binding is this module's API.
            let redundant_alias = alias
                .asname
                .as_ref()
                .is_some_and(|a| a.as_str() == imported);
            let mut flags = SymbolFlags::TOP_LEVEL;
            if redundant_alias || self.line_silences_unused_import(alias.range.start().to_u32()) {
                flags.insert(SymbolFlags::RE_EXPORT);
            }
            let local = self.declare(local_name, SymbolKind::Import, local_start, end, flags);
            let start = self.loc(alias.range.start().to_u32());
            self.imports.push(Import {
                module: self.module,
                specifier: specifier.clone(),
                kind: ImportKind::Named(imported.to_string()),
                local: Some(local),
                start,
                type_only: false,
            });
        }
    }

    /// Record what an assignment's targets declare, and what they read.
    ///
    /// `x = 1` declares `x`. `os.environ["K"] = v` declares nothing but reads
    /// `os` — and missing that read is how a module that plainly uses an
    /// import gets told the import is unused.
    fn assign_targets(&mut self, targets: &[Expr], end: u32) {
        let declares = self.stack.is_empty() || self.in_class_body();
        let kind = if self.in_class_body() {
            SymbolKind::Property
        } else {
            SymbolKind::Variable
        };
        for target in targets {
            match target {
                // A plain name is a binding, and binding it is not using it.
                Expr::Name(name) if declares => {
                    self.declare(
                        name.id.to_string(),
                        kind,
                        name.range.start().to_u32(),
                        end,
                        SymbolFlags::NONE,
                    );
                }
                Expr::Name(_) => {}
                // `A, B = 1, 2` and `[x, *rest] = …` bind each element.
                Expr::Tuple(tuple) => self.assign_targets(&tuple.elts, end),
                Expr::List(list) => self.assign_targets(&list.elts, end),
                Expr::Starred(starred) => {
                    self.assign_targets(std::slice::from_ref(&starred.value), end)
                }
                // Anything else — an attribute, a subscript — has to be
                // evaluated before it can be assigned to, so whatever it names
                // is read.
                other => self.visit_expr(other),
            }
        }
    }
}

impl<'a> SourceOrderVisitor<'a> for Walk<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(f) => self.function(f),
            Stmt::ClassDef(c) => self.class(c),
            Stmt::Import(i) => {
                for alias in &i.names {
                    self.import(alias);
                }
            }
            Stmt::ImportFrom(i) => {
                self.import_from(i.level, i.module.as_ref(), &i.names, i.range.end().to_u32());
            }
            Stmt::Assign(a) => {
                // The value is visited first so `x = x + 1` reads the old `x`
                // before the new declaration shadows it in the symbol list.
                self.visit_expr(&a.value);
                self.assign_targets(&a.targets, a.range.end().to_u32());
            }
            Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    self.visit_expr(value);
                }
                self.visit_annotation(&a.annotation);
                self.assign_targets(std::slice::from_ref(&a.target), a.range.end().to_u32());
            }
            // `COUNT += 1` reads `COUNT` before it writes it: a declaration
            // whose only other mention is an augmented assignment is used.
            Stmt::AugAssign(a) => {
                self.visit_expr(&a.value);
                if let Expr::Name(name) = a.target.as_ref() {
                    self.reference(
                        name.id.to_string(),
                        ReferenceKind::Binding,
                        name.range.start().to_u32(),
                    );
                }
                self.assign_targets(std::slice::from_ref(&a.target), a.range.end().to_u32());
            }
            other => source_order::walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            // A plain name read. A store target is a declaration, handled by
            // the statement visitor, and counting it as a reference would make
            // every assignment look like a use.
            Expr::Name(name) => {
                if name.ctx == ExprContext::Load {
                    self.reference(
                        name.id.to_string(),
                        ReferenceKind::Binding,
                        name.range.start().to_u32(),
                    );
                }
            }
            Expr::Attribute(attribute) => {
                self.reference(
                    attribute.attr.to_string(),
                    ReferenceKind::Member,
                    attribute.attr.range.start().to_u32(),
                );
                self.visit_expr(&attribute.value);
                return;
            }
            // `obj["name"]` reaches a member; `obj[key]` reaches any of them.
            Expr::Subscript(subscript) => {
                match subscript.slice.as_ref() {
                    Expr::StringLiteral(s) => {
                        let name = s.value.to_str().to_string();
                        self.reference(name, ReferenceKind::Member, s.range.start().to_u32());
                    }
                    Expr::NumberLiteral(_) | Expr::Slice(_) => {}
                    _ => {
                        // A subscript on a name is usually a container read,
                        // not member lookup; only a subscript on a call or
                        // attribute chain suggests a computed dispatch table.
                        if !matches!(subscript.value.as_ref(), Expr::Name(_)) {
                            self.dynamic = true;
                        }
                    }
                }
                source_order::walk_expr(self, expr);
                return;
            }
            Expr::Call(call) => {
                if let Some(name) = callee_name(&call.func)
                    && is_dynamic_builtin(&name)
                {
                    self.dynamic = true;
                }
            }
            Expr::StringLiteral(s) => {
                if self.in_annotation > 0 {
                    // `def f(m: 'torch.nn.Conv1d')` reads `torch` exactly as
                    // surely as the unquoted form does. The quotes are there
                    // because the name may not be importable at run time —
                    // which is the entire point of `if TYPE_CHECKING:` — and
                    // without parsing them the import that supplies the name
                    // looks unused.
                    for part in s.value.iter() {
                        let Ok(parsed) = parse_string_annotation(self.source, part) else {
                            continue;
                        };
                        let mut names = AnnotationNames::default();
                        names.visit_expr(parsed.expr());
                        for (name, kind, at) in names.found {
                            self.reference(name, kind, at);
                        }
                    }
                    return;
                }
                let value = s.value.to_str();
                if is_identifier_like(value) {
                    self.strings.insert(value.to_string());
                }
            }
            _ => {}
        }
        source_order::walk_expr(self, expr);
    }

    fn visit_annotation(&mut self, expr: &'a Expr) {
        self.in_annotation += 1;
        source_order::walk_annotation(self, expr);
        self.in_annotation -= 1;
    }

    fn visit_decorator(&mut self, decorator: &'a Decorator) {
        source_order::walk_decorator(self, decorator);
    }
}

/// Collects the names a parsed forward-reference annotation mentions.
///
/// Kept separate from [`Walk`] because the parsed annotation is owned by a
/// local value and so cannot hand out the borrow the main visitor needs.
#[derive(Default)]
struct AnnotationNames {
    found: Vec<(String, ReferenceKind, u32)>,
}

impl<'b> SourceOrderVisitor<'b> for AnnotationNames {
    fn visit_expr(&mut self, expr: &'b Expr) {
        match expr {
            Expr::Name(name) => self.found.push((
                name.id.to_string(),
                ReferenceKind::Binding,
                name.range.start().to_u32(),
            )),
            Expr::Attribute(attribute) => self.found.push((
                attribute.attr.to_string(),
                ReferenceKind::Member,
                attribute.attr.range.start().to_u32(),
            )),
            _ => {}
        }
        source_order::walk_expr(self, expr);
    }
}

/// The dotted name a decorator applies, as written (`app.route`, `property`).
fn decorator_name(decorator: &Decorator) -> String {
    fn dotted(expr: &Expr, out: &mut String) {
        match expr {
            Expr::Name(n) => out.push_str(n.id.as_str()),
            Expr::Attribute(a) => {
                dotted(&a.value, out);
                out.push('.');
                out.push_str(a.attr.as_str());
            }
            Expr::Call(c) => dotted(&c.func, out),
            _ => {}
        }
    }
    let mut out = String::new();
    dotted(&decorator.expression, &mut out);
    out
}

/// Decorators that change how a declaration is called but never call it
/// themselves. A function carrying only these is as analyzable as a bare one.
fn is_inert_decorator(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or(name);
    matches!(
        last,
        "property"
            | "staticmethod"
            | "classmethod"
            | "cached_property"
            | "abstractmethod"
            | "abstractproperty"
            | "override"
            | "final"
            | "overload"
            | "wraps"
            | "dataclass"
            | "total_ordering"
            | "lru_cache"
            | "cache"
            | "contextmanager"
            | "asynccontextmanager"
    )
}

/// The callee of a call, as a dotted name.
fn callee_name(func: &Expr) -> Option<String> {
    match func {
        Expr::Name(n) => Some(n.id.to_string()),
        Expr::Attribute(a) => Some(a.attr.to_string()),
        _ => None,
    }
}

/// Builtins that reach a declaration without naming it.
fn is_dynamic_builtin(name: &str) -> bool {
    matches!(
        name,
        "getattr"
            | "setattr"
            | "hasattr"
            | "delattr"
            | "eval"
            | "exec"
            | "globals"
            | "locals"
            | "vars"
            | "__import__"
            | "import_module"
    )
}

/// Names the interpreter calls for you.
fn is_magic(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__") && name.len() > 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModuleId;

    fn facts_at(path: &str, source: &str) -> FileFacts {
        PythonAnalyzer.analyze(&AnalyzeInput {
            module: ModuleId(0),
            format: "python",
            path,
            source,
        })
    }

    fn facts(source: &str) -> FileFacts {
        facts_at("pkg/mod.py", source)
    }

    fn symbol<'a>(facts: &'a FileFacts, name: &str) -> &'a Symbol {
        facts
            .symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| {
                let have: Vec<&str> = facts.symbols.iter().map(|s| s.name.as_str()).collect();
                panic!("no symbol {name} in {have:?}")
            })
    }

    // ── forward references ──────────────────────────────────────────────

    fn reads(facts: &FileFacts, name: &str) -> bool {
        facts
            .references
            .iter()
            .any(|r| r.name == name && r.kind == ReferenceKind::Binding)
    }

    #[test]
    fn a_quoted_annotation_reads_the_import_that_supplies_it() {
        // The `TYPE_CHECKING` idiom exists precisely so the name need not be
        // importable at run time, and the quotes are what make that work.
        // Reading them is the difference between one import and every import
        // in a typed project looking unused.
        let f = facts(
            "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    import torch\n\n\ndef fn(m: 'torch.nn.Conv1d') -> None:\n    pass\n",
        );
        assert!(reads(&f, "torch"), "got {:?}", f.references);
    }

    #[test]
    fn a_quoted_return_annotation_is_read_too() {
        let f = facts("def fn() -> 'Widget':\n    pass\n");
        assert!(reads(&f, "Widget"), "got {:?}", f.references);
    }

    #[test]
    fn a_quoted_name_nested_in_an_annotation_is_read() {
        let f = facts(
            "from typing import Optional\n\n\ndef fn(x: Optional['Widget']) -> None:\n    pass\n",
        );
        assert!(reads(&f, "Widget"), "got {:?}", f.references);
        assert!(reads(&f, "Optional"));
    }

    #[test]
    fn an_annotated_assignment_reads_its_quoted_type() {
        let f = facts("value: 'Widget' = make()\n");
        assert!(reads(&f, "Widget"), "got {:?}", f.references);
    }

    #[test]
    fn a_string_outside_an_annotation_stays_weak_evidence() {
        let f = facts("name = 'Widget'\n");
        assert!(
            !reads(&f, "Widget"),
            "a plain string must not become a binding read"
        );
        assert!(
            f.references
                .iter()
                .any(|r| r.name == "Widget" && r.kind == ReferenceKind::String),
            "but it stays a string signal, got {:?}",
            f.references
        );
    }

    #[test]
    fn an_unparseable_annotation_string_is_ignored() {
        let f = facts("def fn(x: 'not a type at all!') -> None:\n    pass\n");
        assert!(
            f.references
                .iter()
                .all(|r| r.kind != ReferenceKind::Binding || r.name != "not"),
            "a string that is not an expression must not invent references"
        );
    }

    #[test]
    fn declarations_get_their_kinds_and_spans() {
        let f = facts(
            "def fn():\n    pass\n\nclass C:\n    attr = 1\n\n    def method(self):\n        pass\n\nCONST = 3\n",
        );
        assert_eq!(symbol(&f, "fn").kind, SymbolKind::Function);
        assert_eq!(symbol(&f, "C").kind, SymbolKind::Class);
        assert_eq!(symbol(&f, "attr").kind, SymbolKind::Property);
        assert_eq!(symbol(&f, "method").kind, SymbolKind::Method);
        assert_eq!(symbol(&f, "CONST").kind, SymbolKind::Variable);
        assert_eq!(symbol(&f, "fn").start.line, 1);
        assert_eq!(symbol(&f, "method").parent, Some(symbol(&f, "C").id));
    }

    #[test]
    fn locals_inside_a_function_are_not_declarations() {
        let f = facts("def fn():\n    local = 1\n    return local\n");
        assert!(
            f.symbols.iter().all(|s| s.name != "local"),
            "a function-local assignment is not a declaration basta reports on"
        );
    }

    #[test]
    fn the_underscore_convention_decides_the_public_surface() {
        let f = facts("def public():\n    pass\n\ndef _private():\n    pass\n");
        assert_eq!(symbol(&f, "public").export_name(), Some("public"));
        assert_eq!(symbol(&f, "_private").export_name(), None);
    }

    #[test]
    fn dunder_all_overrides_the_underscore_convention() {
        let f = facts(
            "__all__ = [\"_exposed\"]\n\ndef _exposed():\n    pass\n\ndef public():\n    pass\n",
        );
        assert_eq!(
            symbol(&f, "_exposed").export_name(),
            Some("_exposed"),
            "a name in __all__ is public however it is spelled"
        );
        assert!(
            symbol(&f, "_exposed")
                .flags
                .contains(SymbolFlags::IN_DUNDER_ALL)
        );
        assert_eq!(
            symbol(&f, "public").export_name(),
            None,
            "with __all__ present, a name outside it is private"
        );
    }

    #[test]
    fn an_init_file_exposes_even_underscored_names() {
        let f = facts_at("pkg/__init__.py", "def _reexported():\n    pass\n");
        assert_eq!(symbol(&f, "_reexported").export_name(), Some("_reexported"));
    }

    #[test]
    fn a_computed_dunder_all_falls_back_to_convention() {
        let f = facts("__all__ = compute()\n\ndef public():\n    pass\n");
        assert_eq!(symbol(&f, "public").export_name(), Some("public"));
    }

    #[test]
    fn imports_record_specifier_binding_and_shape() {
        let f = facts(
            "import os\nimport os.path as osp\nfrom . import sibling\nfrom .utils import helper as h\nfrom ..pkg.mod import *\n",
        );
        let find = |spec: &str, kind: &ImportKind| {
            f.imports
                .iter()
                .find(|i| i.specifier == spec && &i.kind == kind)
                .unwrap_or_else(|| panic!("no {kind:?} import of {spec}"))
        };
        find("os", &ImportKind::Namespace);
        find("os.path", &ImportKind::Namespace);
        find(".", &ImportKind::Named("sibling".into()));
        find(".utils", &ImportKind::Named("helper".into()));
        find("..pkg.mod", &ImportKind::StarReExport);

        assert_eq!(symbol(&f, "os").kind, SymbolKind::Import);
        assert_eq!(
            symbol(&f, "osp").kind,
            SymbolKind::Import,
            "an aliased import binds the alias, not the dotted path"
        );
        assert_eq!(symbol(&f, "h").kind, SymbolKind::Import);
    }

    #[test]
    fn a_future_directive_binds_nothing_and_is_never_a_finding() {
        let f = facts("from __future__ import annotations\n\n\ndef fn():\n    pass\n");
        assert!(
            f.symbols.iter().all(|s| s.name != "annotations"),
            "a compiler directive is not a binding anyone can use"
        );
        assert!(f.imports.iter().all(|i| i.specifier != "__future__"));
    }

    #[test]
    fn an_import_is_only_a_re_export_where_convention_says_so() {
        let ordinary = facts("import os\nfrom .util import helper\n");
        assert_eq!(
            symbol(&ordinary, "os").export_name(),
            None,
            "`import os` does not make os part of this module's API"
        );
        assert_eq!(symbol(&ordinary, "helper").export_name(), None);

        let package = facts_at("pkg/__init__.py", "from .base import BaseAdapter\n");
        assert_eq!(
            symbol(&package, "BaseAdapter").export_name(),
            Some("BaseAdapter"),
            "an import in __init__.py is the package surface"
        );

        let listed = facts("__all__ = [\"helper\"]\nfrom .util import helper\nimport os\n");
        assert_eq!(symbol(&listed, "helper").export_name(), Some("helper"));
        assert_eq!(symbol(&listed, "os").export_name(), None);
    }

    #[test]
    fn an_attribute_assignment_reads_the_name_it_is_assigned_through() {
        let f = facts("import os\n\nos.environ[\"TOKEN\"] = value\n");
        assert!(
            symbol(&f, "os").local_refs > 0,
            "`os.environ[k] = v` has to evaluate `os` before it can assign"
        );
    }

    #[test]
    fn assigning_to_a_plain_name_is_not_a_use_of_it() {
        let f = facts("value = 1\n");
        assert_eq!(symbol(&f, "value").local_refs, 0);
    }

    #[test]
    fn a_noqa_on_a_plain_import_marks_it_deliberate() {
        let f = facts("import inquirer  # noqa: F401\n");
        assert_eq!(symbol(&f, "inquirer").export_name(), Some("inquirer"));
        let plain = facts("import inquirer\n");
        assert_eq!(symbol(&plain, "inquirer").export_name(), None);
    }

    #[test]
    fn rebinding_a_module_level_name_does_not_create_a_second_declaration() {
        let f = facts(
            "WEIGHTS = {\"a\": 1}\nif tuned:\n    WEIGHTS = load()\n\n\ndef use():\n    return WEIGHTS\n",
        );
        let declarations: Vec<&Symbol> = f.symbols.iter().filter(|s| s.name == "WEIGHTS").collect();
        assert_eq!(
            declarations.len(),
            1,
            "one name bound twice is one declaration, not two"
        );
        assert!(
            declarations[0].local_refs > 0,
            "the reader resolves to the same declaration the writer created"
        );
    }

    #[test]
    fn explicit_re_export_conventions_make_an_import_part_of_the_api() {
        let redundant = facts("from .observation_loader import load as load\n");
        assert_eq!(
            symbol(&redundant, "load").export_name(),
            Some("load"),
            "PEP 484's redundant alias is an explicit re-export"
        );

        let silenced = facts("from .observation_loader import load  # noqa: F401\n");
        assert_eq!(symbol(&silenced, "load").export_name(), Some("load"));

        let bare_noqa = facts("from .loader import load  # noqa\n");
        assert_eq!(symbol(&bare_noqa, "load").export_name(), Some("load"));

        let other_code = facts("from .loader import load  # noqa: E501\n");
        assert_eq!(
            symbol(&other_code, "load").export_name(),
            None,
            "silencing a different rule says nothing about the import"
        );

        let plain = facts("from .loader import load\n");
        assert_eq!(symbol(&plain, "load").export_name(), None);
    }

    #[test]
    fn references_are_attributed_to_the_enclosing_declaration() {
        let f = facts("def outer():\n    helper()\n\ndef helper():\n    pass\n");
        let outer = symbol(&f, "outer").id;
        let call = f
            .references
            .iter()
            .find(|r| r.name == "helper" && r.kind == ReferenceKind::Binding)
            .expect("reference to helper");
        assert_eq!(call.from, Some(outer));
        assert_eq!(symbol(&f, "helper").local_refs, 1);
        assert_eq!(symbol(&f, "outer").local_refs, 0);
    }

    #[test]
    fn a_decorator_belongs_to_the_enclosing_scope_not_the_function_it_decorates() {
        let f = facts("import app\n\n@app.route(\"/x\")\ndef handler():\n    pass\n");
        let decorator_ref = f
            .references
            .iter()
            .find(|r| r.name == "app" && r.kind == ReferenceKind::Binding)
            .expect("reference to app");
        assert_eq!(
            decorator_ref.from, None,
            "a decorator runs at module level, so a dead handler cannot keep it alive"
        );
        assert!(symbol(&f, "handler").flags.contains(SymbolFlags::DECORATED));
    }

    #[test]
    fn inert_decorators_do_not_count_as_framework_magic() {
        let f = facts(
            "import functools\n\nclass C:\n    @property\n    def a(self):\n        pass\n\n    @staticmethod\n    def b():\n        pass\n\n    @functools.lru_cache\n    def c(self):\n        pass\n",
        );
        for name in ["a", "b", "c"] {
            assert!(
                !symbol(&f, name).flags.contains(SymbolFlags::DECORATED),
                "@property / @staticmethod / @lru_cache never call the function themselves ({name})"
            );
        }
    }

    #[test]
    fn abstract_and_magic_members_are_flagged() {
        let f = facts(
            "from abc import abstractmethod\n\nclass C:\n    def __init__(self):\n        pass\n\n    @abstractmethod\n    def must_implement(self):\n        pass\n",
        );
        assert!(symbol(&f, "__init__").flags.contains(SymbolFlags::MAGIC));
        assert!(
            symbol(&f, "must_implement")
                .flags
                .contains(SymbolFlags::ABSTRACT)
        );
    }

    #[test]
    fn attribute_access_is_recorded_as_a_member_reference() {
        let f = facts("obj.used()\nother[\"also_used\"]\n");
        let members: Vec<&str> = f
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::Member)
            .map(|r| r.name.as_str())
            .collect();
        assert!(members.contains(&"used"), "{members:?}");
        assert!(members.contains(&"also_used"), "{members:?}");
    }

    #[test]
    fn runtime_name_resolution_sets_the_dynamic_flag() {
        for src in [
            "getattr(obj, name)\n",
            "eval(\"1\")\n",
            "globals()[\"x\"]\n",
            "import importlib\nimportlib.import_module(name)\n",
        ] {
            assert!(facts(src).has_dynamic_access, "{src}");
        }
        assert!(
            !facts("obj.attr\nitems[0]\nmapping[\"key\"]\nvalues[i]\n").has_dynamic_access,
            "plain attribute and container access is not dynamic"
        );
    }

    #[test]
    fn identifier_shaped_strings_become_weak_references() {
        let f = facts("ROUTES = {\"/\": \"handle_root\"}\n");
        assert!(
            f.references
                .iter()
                .any(|r| r.kind == ReferenceKind::String && r.name == "handle_root")
        );
    }

    #[test]
    fn an_unparsable_file_reports_the_failure_rather_than_lying() {
        let f = facts("def (:\n  ???\n");
        assert!(f.parse_failed);
        assert!(f.symbols.is_empty());
    }

    #[test]
    fn a_nested_def_does_not_swallow_a_top_level_name() {
        let f = facts(
            "def helper():\n    pass\n\n\ndef outer():\n    def helper():\n        pass\n\n    return helper\n\n\nhelper()\n",
        );
        let helpers: Vec<&Symbol> = f.symbols.iter().filter(|s| s.name == "helper").collect();
        assert_eq!(helpers.len(), 2, "two scopes, two declarations");
        let top = helpers
            .iter()
            .find(|s| s.flags.contains(SymbolFlags::TOP_LEVEL))
            .unwrap();
        assert_eq!(top.start.line, 1);
    }

    #[test]
    fn tuple_unpacking_declares_every_name() {
        let f = facts("A, B = 1, 2\n[c, *rest] = items\n");
        for name in ["A", "B", "c", "rest"] {
            assert_eq!(symbol(&f, name).kind, SymbolKind::Variable, "{name}");
        }
    }

    #[test]
    fn an_augmented_assignment_is_a_use() {
        let f = facts("COUNT = 0\nCOUNT += 1\n");
        assert_eq!(symbol(&f, "COUNT").local_refs, 1);
        assert_eq!(
            f.symbols.iter().filter(|s| s.name == "COUNT").count(),
            1,
            "still one declaration"
        );
    }

    #[test]
    fn a_main_guard_has_to_be_a_comparison() {
        let a = PythonAnalyzer;
        assert!(a.is_self_starting("if __name__ == \"__main__\":\n    main()\n"));
        assert!(a.is_self_starting("if __name__=='__main__': main()\n"));
        assert!(a.is_self_starting("#!/usr/bin/env python3\n"));
        assert!(
            !a.is_self_starting("\"\"\"Sets __name__ and __main__ in the docstring.\"\"\"\n"),
            "two strings in prose are not a main guard"
        );
    }

    #[test]
    fn module_traits_make_a_package_init_the_surface_and_names_attribute_reachable() {
        let init = PythonAnalyzer.module_traits("pkg/__init__.py");
        assert!(init.entry_point && init.package_surface && init.names_are_attributes);
        let plain = PythonAnalyzer.module_traits("pkg/mod.py");
        assert!(!plain.entry_point && !plain.package_surface && plain.names_are_attributes);
        assert!(!plain.ambient_declarations);
    }

    // ── resolution ──────────────────────────────────────────────────────

    fn index(files: &[&str]) -> ModuleIndex {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        for (i, f) in files.iter().enumerate() {
            index.insert(PathBuf::from(f), ModuleId(i as u32));
        }
        index
    }

    fn py(files: &[&str], importer: &str, specifier: &str) -> Option<usize> {
        PythonAnalyzer
            .resolve(specifier, Path::new(importer), &index(files))
            .map(|m| m.0 as usize)
    }

    #[test]
    fn a_src_layout_implies_its_own_import_root() {
        let roots = package_parents(&[
            PathBuf::from("/p/src/mypkg/__init__.py"),
            PathBuf::from("/p/src/mypkg/thing.py"),
            PathBuf::from("/p/utils/script.py"),
        ]);
        assert_eq!(roots, vec![PathBuf::from("/p/src")]);
    }

    #[test]
    fn only_the_outermost_package_contributes_a_root() {
        let roots = package_parents(&[
            PathBuf::from("/p/src/a/__init__.py"),
            PathBuf::from("/p/src/a/b/__init__.py"),
        ]);
        assert_eq!(
            roots,
            vec![PathBuf::from("/p/src")],
            "a nested package is reached through its parent, not on its own"
        );
    }

    #[test]
    fn a_tree_without_packages_implies_no_roots() {
        assert!(package_parents(&[PathBuf::from("/p/script.py")]).is_empty());
    }

    #[test]
    fn an_absolute_import_resolves_through_a_derived_root() {
        // `utils/check_repo.py` in a src-layout project belongs to no package
        // and sits under no scan root that makes `mypkg.thing` resolvable.
        let files = [
            "/p/src/mypkg/__init__.py",
            "/p/src/mypkg/thing.py",
            "/p/utils/script.py",
        ];
        let mut index = index(&files);
        let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        index.set_import_roots(package_parents(&paths));
        assert_eq!(
            PythonAnalyzer
                .resolve("mypkg.thing", Path::new("/p/utils/script.py"), &index)
                .map(|m| m.0 as usize),
            Some(1)
        );
    }

    #[test]
    fn relative_imports_count_dots() {
        let files = [
            "/p/pkg/sub/mod.py",
            "/p/pkg/sub/sibling.py",
            "/p/pkg/other.py",
            "/p/pkg/sub/__init__.py",
        ];
        assert_eq!(py(&files, "/p/pkg/sub/mod.py", ".sibling"), Some(1));
        assert_eq!(py(&files, "/p/pkg/sub/mod.py", "..other"), Some(2));
        assert_eq!(
            py(&files, "/p/pkg/sub/mod.py", "."),
            Some(3),
            "`from . import x` names the package's own __init__"
        );
        assert_eq!(
            py(
                &["/p/pkg/mod.py", "/p/pkg/sub/__init__.py"],
                "/p/pkg/mod.py",
                ".sub"
            ),
            Some(1)
        );
    }

    #[test]
    fn absolute_imports_resolve_from_the_scan_root() {
        let files = ["/p/app/main.py", "/p/app/services/db.py"];
        assert_eq!(py(&files, "/p/app/main.py", "app.services.db"), Some(1));
        assert_eq!(py(&["/p/a.py"], "/p/a.py", "os.path"), None);
    }

    #[test]
    fn a_named_import_of_a_submodule_is_normalised_to_the_module() {
        let files = ["/p/pkg/__init__.py", "/p/pkg/helpers.py", "/p/pkg/main.py"];
        let mut import = Import {
            module: ModuleId(2),
            specifier: "pkg".into(),
            kind: ImportKind::Named("helpers".into()),
            local: None,
            start: Location {
                line: 1,
                column: 0,
                offset: 0,
            },
            type_only: false,
        };
        PythonAnalyzer.normalize_import(&mut import, Path::new("/p/pkg/main.py"), &index(&files));
        assert_eq!(import.specifier, "pkg.helpers");
        assert_eq!(import.kind, ImportKind::Namespace);

        let mut plain = Import {
            kind: ImportKind::Named("not_a_module".into()),
            ..import.clone()
        };
        plain.specifier = "pkg".into();
        PythonAnalyzer.normalize_import(&mut plain, Path::new("/p/pkg/main.py"), &index(&files));
        assert_eq!(
            plain.kind,
            ImportKind::Named("not_a_module".into()),
            "left alone"
        );
    }

    // ── manifests ───────────────────────────────────────────────────────

    #[test]
    fn pyproject_console_scripts_name_their_module() {
        let toml = r#"
[project]
name = "skylos"
dependencies = ["rich", "click"]

[project.scripts]
skylos = "skylos.cli:main"
skylos-mcp = "skylos_mcp.server:run"

[project.entry-points."pytest11"]
skylos = "skylos.plugins.pytest_plugin"

[tool.setuptools.packages.find]
where = ["."]
include = ["skylos*"]
"#;
        let mut modules = pyproject_entry_modules(toml);
        modules.sort();
        assert_eq!(
            modules,
            vec![
                "skylos.cli",
                "skylos.plugins.pytest_plugin",
                "skylos_mcp.server"
            ],
            "a dependency list and a packages-find table are not entry points"
        );
        assert!(pyproject_entry_modules("this is not toml at all {{{").is_empty());

        let paths = PythonAnalyzer.manifest_entries(Path::new("/p"), "pyproject.toml", toml);
        for want in [
            "/p/skylos/cli.py",
            "/p/skylos/cli/__init__.py",
            "/p/src/skylos/cli.py",
        ] {
            assert!(paths.contains(&PathBuf::from(want)), "{want} missing");
        }
    }

    #[test]
    fn a_method_body_is_a_function_scope_not_a_class_body() {
        let f = facts("class C:\n    def m(self):\n        inner = 1\n        return inner\n");
        assert!(
            f.symbols.iter().all(|s| s.name != "inner"),
            "an assignment inside a method is a local, not a class attribute"
        );
    }
}
