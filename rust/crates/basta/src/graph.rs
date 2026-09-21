//! The whole-project view: which module every specifier names, which
//! declaration every reference reaches, and what is left over.
//!
//! Analyzers speak in module-local terms. This module merges their output
//! into one address space and answers the two questions the report is made
//! of:
//!
//! * **Which files run?** Breadth-first over import edges from the entry
//!   points. A file nothing imports and nothing starts is unused.
//! * **Which declarations run?** Breadth-first over reference edges from the
//!   entry points' exports and from the top level of every reachable module,
//!   because importing a module runs its top level. A declaration nothing
//!   reaches is dead — including one reached only from other dead code, which
//!   is why this is a traversal and not a reference count.
//!
//! Where the languages differ in how precisely a reference can be resolved,
//! the traversal errs towards keeping things alive. A member access matches
//! members by name across the whole scan, because basta infers no types; a
//! wildcard re-export keeps every name of its target reachable. Both make
//! basta miss dead code it cannot prove is dead, which is the direction a
//! reader can afford.

use crate::lang::analyzer_for;
use crate::model::{
    FileFacts, Import, ImportKind, Module, ModuleId, Reference, ReferenceKind, Symbol, SymbolFlags,
    SymbolId, SymbolKind,
};
use crate::resolve::ModuleIndex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::ops::Range;

/// A merged, resolved view of every analyzed file.
pub struct Graph {
    pub modules: Vec<Module>,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub references: Vec<Reference>,
    /// Symbols of each module, as a range into [`Graph::symbols`].
    module_symbols: Vec<Range<usize>>,
    /// Target module of each import, parallel to [`Graph::imports`].
    import_targets: Vec<Option<ModuleId>>,
    /// Modules reached from entry points, tests included. Anything outside
    /// this set runs nowhere at all.
    reachable_modules: FxHashSet<ModuleId>,
    /// Modules reached from entry points that are *not* tests. A module in
    /// `reachable_modules` but not here exists only to serve the test suite.
    production_modules: FxHashSet<ModuleId>,
    /// Declarations reached from entry points, tests included.
    reachable_symbols: FxHashSet<SymbolId>,
    /// Declarations reached without going through a test file.
    production_symbols: FxHashSet<SymbolId>,
    /// `(module, export name)` pairs some live module imports by name.
    imported_names: FxHashSet<(ModuleId, String)>,
    /// The same, counting only live modules that are not tests.
    production_imported_names: FxHashSet<(ModuleId, String)>,
    /// Who imports `(module, name)`, by name — every importer, live or not.
    /// Built once so a reachability pass can ask about a symbol in O(1)
    /// instead of rescanning every import per symbol.
    importers: FxHashMap<(ModuleId, String), Vec<ModuleId>>,
    /// Modules whose every export may be reached without being named: the
    /// target of a namespace import or a wildcard re-export made by a live
    /// module.
    wildcarded: FxHashSet<ModuleId>,
    /// Import bindings with at least one reference.
    used_imports: FxHashSet<SymbolId>,
    /// Names read as members anywhere in the scan.
    member_names: FxHashSet<String>,
    /// Identifier-shaped strings seen anywhere in the scan.
    string_names: FxHashSet<String>,
    /// Names declared by more than one module, so reference matching across
    /// modules cannot tell them apart.
    ambiguous_names: FxHashSet<String>,
}

/// What [`Graph::build`] needs to know about each module beyond its facts.
pub struct ModuleInput {
    pub module: Module,
    pub facts: FileFacts,
    pub is_entry: bool,
    pub is_test: bool,
}

impl Graph {
    /// Merge per-file facts into one graph and resolve it.
    ///
    /// Both traversals run twice: once from every entry point, and once from
    /// the entry points that are not tests. The first answers "does this run
    /// at all"; the difference between them answers "does this run only when
    /// the test suite does", which is a different conversation to have with a
    /// reader and is scored differently.
    pub fn build(inputs: Vec<ModuleInput>, index: &ModuleIndex) -> Self {
        let mut graph = Self::merge(inputs);
        graph.resolve_imports(index);
        graph.index_module_boundary();
        graph.reachable_modules = graph.module_reachability(Roots::Everything);
        graph.production_modules = graph.module_reachability(Roots::ProductionOnly);
        graph.wildcarded = graph.wildcarded_by(&graph.reachable_modules);
        graph.reachable_symbols = graph.symbol_reachability(Roots::Everything);
        graph.production_symbols = graph.symbol_reachability(Roots::ProductionOnly);
        graph.index_import_use();
        graph.index_imported_names();
        graph
    }

    /// Flatten per-module ids into one address space.
    fn merge(inputs: Vec<ModuleInput>) -> Self {
        let mut modules = Vec::with_capacity(inputs.len());
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut references = Vec::new();
        let mut module_symbols = Vec::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            let module_id = ModuleId(index as u32);
            let offset = symbols.len() as u32;
            let ModuleInput {
                mut module,
                facts,
                is_entry,
                is_test,
            } = input;
            module.id = module_id;
            module.is_entry = is_entry;
            module.is_test = is_test;
            module.has_dynamic_access = facts.has_dynamic_access;
            module.parse_failed = facts.parse_failed;

            let start = symbols.len();
            for mut symbol in facts.symbols {
                symbol.id = SymbolId(symbol.id.0 + offset);
                symbol.module = module_id;
                symbol.parent = symbol.parent.map(|p| SymbolId(p.0 + offset));
                symbols.push(symbol);
            }
            module_symbols.push(start..symbols.len());

            imports.extend(facts.imports.into_iter().map(|mut import| {
                import.module = module_id;
                import.local = import.local.map(|l| SymbolId(l.0 + offset));
                import
            }));
            references.extend(facts.references.into_iter().map(|mut reference| {
                reference.module = module_id;
                reference.from = reference.from.map(|f| SymbolId(f.0 + offset));
                reference
            }));
            modules.push(module);
        }

        Self {
            modules,
            symbols,
            imports,
            references,
            module_symbols,
            import_targets: Vec::new(),
            reachable_modules: FxHashSet::default(),
            production_modules: FxHashSet::default(),
            reachable_symbols: FxHashSet::default(),
            production_symbols: FxHashSet::default(),
            imported_names: FxHashSet::default(),
            production_imported_names: FxHashSet::default(),
            importers: FxHashMap::default(),
            wildcarded: FxHashSet::default(),
            used_imports: FxHashSet::default(),
            member_names: FxHashSet::default(),
            string_names: FxHashSet::default(),
            ambiguous_names: FxHashSet::default(),
        }
    }

    /// Ask each import's language which module it names. The language gets
    /// to rewrite the import first, for forms only the index can disambiguate.
    fn resolve_imports(&mut self, index: &ModuleIndex) {
        let mut targets = Vec::with_capacity(self.imports.len());
        for import in &mut self.imports {
            let module = &self.modules[import.module.0 as usize];
            let Some(analyzer) = analyzer_for(&module.format) else {
                targets.push(None);
                continue;
            };
            analyzer.normalize_import(import, &module.real_path, index);
            targets.push(match import.kind {
                ImportKind::Dynamic | ImportKind::Glob => None,
                _ => analyzer.resolve(&import.specifier, &module.real_path, index),
            });
        }
        self.import_targets = targets;
        self.expand_globs(index);
    }

    /// Turn each directory glob into one resolved edge per module it reaches.
    ///
    /// The edges are namespace imports — the kind a literal `import('./x')`
    /// already produces — because that is what a bundler hands back: the
    /// whole module object, whose members the importer then reads by a name
    /// it computes (`catalogs[path].messages`). Expanding here rather than
    /// teaching every pass about a second kind of target keeps the rest of
    /// the graph unchanged.
    fn expand_globs(&mut self, index: &ModuleIndex) {
        let mut expanded: Vec<(Import, ModuleId)> = Vec::new();
        for import in &self.imports {
            if import.kind != ImportKind::Glob {
                continue;
            }
            let module = &self.modules[import.module.0 as usize];
            let Some(analyzer) = analyzer_for(&module.format) else {
                continue;
            };
            for target in analyzer.glob_targets(&import.specifier, &module.real_path, index) {
                if target == import.module {
                    continue; // A directory glob reaching the file that wrote it.
                }
                expanded.push((
                    Import {
                        module: import.module,
                        specifier: import.specifier.clone(),
                        kind: ImportKind::Namespace,
                        local: None,
                        start: import.start.clone(),
                        type_only: false,
                    },
                    target,
                ));
            }
        }
        for (import, target) in expanded {
            self.imports.push(import);
            self.import_targets.push(Some(target));
        }
    }

    /// Everything that can be indexed before either traversal runs.
    fn index_module_boundary(&mut self) {
        for reference in &self.references {
            match reference.kind {
                ReferenceKind::Member => {
                    self.member_names.insert(reference.name.clone());
                }
                ReferenceKind::String => {
                    self.string_names.insert(reference.name.clone());
                }
                ReferenceKind::Binding => {}
            }
        }

        // A name several modules declare cannot be matched across modules with
        // any confidence — worth saying in the report rather than hiding.
        let mut seen: FxHashMap<&str, ModuleId> = FxHashMap::default();
        let mut ambiguous = FxHashSet::default();
        for symbol in &self.symbols {
            if symbol.kind == SymbolKind::Import || !symbol.flags.contains(SymbolFlags::TOP_LEVEL) {
                continue;
            }
            match seen.get(symbol.name.as_str()) {
                Some(other) if *other != symbol.module => {
                    ambiguous.insert(symbol.name.clone());
                }
                Some(_) => {}
                None => {
                    seen.insert(symbol.name.as_str(), symbol.module);
                }
            }
        }
        self.ambiguous_names = ambiguous;

        for (index, import) in self.imports.iter().enumerate() {
            let Some(target) = self.import_targets[index] else {
                continue;
            };
            let name = match &import.kind {
                ImportKind::Named(name) => name.clone(),
                ImportKind::Default => "default".to_string(),
                _ => continue,
            };
            self.importers
                .entry((target, name))
                .or_default()
                .push(import.module);
        }
    }

    /// Modules whose exports can be reached without being named, by a
    /// namespace import or a wildcard re-export made by one of `live`.
    fn wildcarded_by(&self, live: &FxHashSet<ModuleId>) -> FxHashSet<ModuleId> {
        self.imports
            .iter()
            .enumerate()
            .filter(|(_, import)| {
                matches!(
                    import.kind,
                    ImportKind::Namespace | ImportKind::StarReExport
                ) && live.contains(&import.module)
            })
            .filter_map(|(index, _)| self.import_targets[index])
            .collect()
    }

    /// Breadth-first over import edges from the entry points.
    fn module_reachability(&self, roots: Roots) -> FxHashSet<ModuleId> {
        // Imports grouped by the module that makes them, so the traversal
        // does not rescan the whole list per module.
        let mut edges: Vec<Vec<ModuleId>> = vec![Vec::new(); self.modules.len()];
        for (index, import) in self.imports.iter().enumerate() {
            if let Some(target) = self.import_targets[index] {
                edges[import.module.0 as usize].push(target);
            }
        }

        let mut queue: VecDeque<ModuleId> = VecDeque::new();
        let mut seen = FxHashSet::default();
        for module in &self.modules {
            if roots.accepts(module) && seen.insert(module.id) {
                queue.push_back(module.id);
            }
        }
        while let Some(current) = queue.pop_front() {
            for target in &edges[current.0 as usize] {
                if seen.insert(*target) {
                    queue.push_back(*target);
                }
            }
        }
        seen
    }

    /// Breadth-first over reference edges.
    ///
    /// Roots are every declaration the program can start at: the exports of
    /// entry modules, everything referenced from the top level of a reachable
    /// module (importing a module runs its top level), and the declarations
    /// the runtime calls without naming — constructors, dunder methods, and
    /// anything a framework decorator took a handle to.
    fn symbol_reachability(&self, roots: Roots) -> FxHashSet<SymbolId> {
        let scopes = self.build_scopes();
        let members_by_name = self.build_member_index();
        let live_modules = match roots {
            Roots::Everything => &self.reachable_modules,
            Roots::ProductionOnly => &self.production_modules,
        };
        // Both root sets can reach a namespace-imported module's exports, but
        // only through an importer that is itself live for that root set.
        let wildcarded = self.wildcarded_by(live_modules);

        // Reference edges, indexed by the declaration they appear in. A
        // reference at a module's top level is keyed by `None`.
        let mut from_symbol: Vec<Vec<SymbolId>> = vec![Vec::new(); self.symbols.len()];
        let mut from_top_level: Vec<Vec<SymbolId>> = vec![Vec::new(); self.modules.len()];
        for reference in &self.references {
            if reference.kind == ReferenceKind::String {
                continue; // Evidence for the confidence model, never an edge.
            }
            let targets = resolve_reference(reference, &scopes, &members_by_name);
            match reference.from {
                Some(from) => from_symbol[from.0 as usize].extend(targets),
                None => from_top_level[reference.module.0 as usize].extend(targets),
            }
        }

        // Members of each class, so reaching a class reaches what it can run.
        let mut members_of: Vec<Vec<SymbolId>> = vec![Vec::new(); self.symbols.len()];
        for symbol in &self.symbols {
            if let Some(parent) = symbol.parent {
                members_of[parent.0 as usize].push(symbol.id);
            }
        }

        let mut queue: VecDeque<SymbolId> = VecDeque::new();
        let mut seen: FxHashSet<SymbolId> = FxHashSet::default();
        let enqueue = |id: SymbolId, seen: &mut FxHashSet<SymbolId>, queue: &mut VecDeque<_>| {
            if seen.insert(id) {
                queue.push_back(id);
            }
        };

        for module in &self.modules {
            if !live_modules.contains(&module.id) {
                continue;
            }
            // Importing a module runs its top level.
            for target in &from_top_level[module.id.0 as usize] {
                enqueue(*target, &mut seen, &mut queue);
            }
            let is_root_surface = roots.accepts(module);
            for index in self.module_symbols[module.id.0 as usize].clone() {
                let symbol = &self.symbols[index];
                let always_live = symbol.flags.intersects(
                    SymbolFlags::MAGIC
                        | SymbolFlags::DECORATED
                        | SymbolFlags::ABSTRACT
                        | SymbolFlags::FRAMEWORK_GLOBAL,
                );
                // An entry module's exports are its public surface: something
                // outside the scan calls them.
                let entry_export = is_root_surface && symbol.is_exported();
                // A name imported by some reachable module is reached by it,
                // even when the importing binding itself goes unused: the
                // import is then the finding, not the export.
                // Whether anything imports this declaration by name. Both
                // names are tried: the one it is exported under and the one it
                // is declared under. Python has no `export`, so a module that
                // writes `from .llm import _call_with_retry` imports a name
                // basta does not count as public — and that is still
                // unmistakably a use of it.
                let imported = symbol
                    .export_name()
                    .is_some_and(|name| self.imported_by(module.id, name, roots, live_modules))
                    || self.imported_by(module.id, &symbol.name, roots, live_modules)
                    || (symbol.is_exported() && wildcarded.contains(&module.id));
                if always_live || entry_export || imported {
                    enqueue(symbol.id, &mut seen, &mut queue);
                }
            }
        }

        while let Some(current) = queue.pop_front() {
            // Reaching a member reaches the class that owns it.
            if let Some(parent) = self.symbols[current.0 as usize].parent {
                enqueue(parent, &mut seen, &mut queue);
            }
            // And reaching a class reaches its members, because whoever holds
            // the class calls them: an HTTP handler's `do_GET`, a React
            // component's `render`, a subclass hook. Nothing in the project
            // names those, and treating a live class's methods as dead would
            // strand every helper they call.
            //
            // This is about following the chain, not about the verdict: a
            // method that no code anywhere names is still reported, by the
            // separate name-based rule in `classify`.
            for member in &members_of[current.0 as usize] {
                enqueue(*member, &mut seen, &mut queue);
            }
            let targets = std::mem::take(&mut from_symbol[current.0 as usize]);
            for target in &targets {
                enqueue(*target, &mut seen, &mut queue);
            }
            from_symbol[current.0 as usize] = targets;
        }

        seen
    }

    /// Whether a live module of this root set imports `(module, name)`.
    fn imported_by(
        &self,
        module: ModuleId,
        name: &str,
        roots: Roots,
        live_modules: &FxHashSet<ModuleId>,
    ) -> bool {
        self.importers
            .get(&(module, name.to_string()))
            .is_some_and(|importers| {
                importers.iter().any(|importer| {
                    live_modules.contains(importer)
                        && (roots == Roots::Everything
                            || !self.modules[importer.0 as usize].is_test)
                })
            })
    }

    /// Which exports are imported by name, once it is known which modules
    /// actually run. An import made by a file nothing reaches is not evidence
    /// that its target is used.
    fn index_imported_names(&mut self) {
        let mut all = FxHashSet::default();
        let mut production = FxHashSet::default();
        for (index, import) in self.imports.iter().enumerate() {
            let Some(target) = self.import_targets[index] else {
                continue;
            };
            if !self.reachable_modules.contains(&import.module) {
                continue;
            }
            let name = match &import.kind {
                ImportKind::Named(name) => name.clone(),
                ImportKind::Default => "default".to_string(),
                _ => continue,
            };
            if self.production_modules.contains(&import.module) {
                production.insert((target, name.clone()));
            }
            all.insert((target, name));
        }
        self.imported_names = all;
        self.production_imported_names = production;
    }

    fn index_import_use(&mut self) {
        self.used_imports = self
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Import && s.local_refs > 0)
            .map(|s| s.id)
            .collect();
    }

    /// Per-module name → declaration, for resolving binding references.
    /// Import bindings are in here too, which is what carries a reference
    /// across the module boundary.
    fn build_scopes(&self) -> Vec<FxHashMap<&str, SymbolId>> {
        self.module_symbols
            .iter()
            .map(|range| {
                let mut scope = FxHashMap::default();
                for symbol in &self.symbols[range.clone()] {
                    if symbol.flags.contains(SymbolFlags::TOP_LEVEL) {
                        scope.insert(symbol.name.as_str(), symbol.id);
                    }
                }
                scope
            })
            .collect()
    }

    /// Member declarations by name, across every module. basta infers no
    /// types, so `x.render()` reaches every `render` there is.
    fn build_member_index(&self) -> FxHashMap<&str, Vec<SymbolId>> {
        let mut index: FxHashMap<&str, Vec<SymbolId>> = FxHashMap::default();
        for symbol in &self.symbols {
            if symbol.kind.is_member() {
                index
                    .entry(symbol.name.as_str())
                    .or_default()
                    .push(symbol.id);
            }
        }
        index
    }

    // ── queries the report layer asks ───────────────────────────────────────

    pub fn module(&self, id: ModuleId) -> &Module {
        &self.modules[id.0 as usize]
    }

    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    pub fn is_module_reachable(&self, id: ModuleId) -> bool {
        self.reachable_modules.contains(&id)
    }

    /// Whether anything at all reaches this declaration, tests included.
    pub fn is_symbol_reachable(&self, id: SymbolId) -> bool {
        self.reachable_symbols.contains(&id)
    }

    /// Whether the shipped program reaches this declaration. A declaration
    /// only the test suite reaches is live but not shipped, which is a
    /// finding with a reason rather than a clean bill of health.
    pub fn is_symbol_production_reachable(&self, id: SymbolId) -> bool {
        self.production_symbols.contains(&id)
    }

    pub fn reachable_module_count(&self) -> usize {
        self.reachable_modules.len()
    }

    /// Whether some live module imports this export by name.
    pub fn is_export_imported(&self, module: ModuleId, export_name: &str) -> bool {
        self.imported_names
            .contains(&(module, export_name.to_string()))
    }

    /// Whether some live module that is not a test imports this export.
    pub fn is_export_imported_by_production(&self, module: ModuleId, export_name: &str) -> bool {
        self.production_imported_names
            .contains(&(module, export_name.to_string()))
    }

    /// Whether the module's exports can be reached without being named.
    pub fn is_wildcarded(&self, module: ModuleId) -> bool {
        self.wildcarded.contains(&module)
    }

    pub fn is_import_used(&self, id: SymbolId) -> bool {
        self.used_imports.contains(&id)
    }

    pub fn is_member_name_read(&self, name: &str) -> bool {
        self.member_names.contains(name)
    }

    pub fn appears_in_string(&self, name: &str) -> bool {
        self.string_names.contains(name)
    }

    pub fn is_name_ambiguous(&self, name: &str) -> bool {
        self.ambiguous_names.contains(name)
    }

    /// Whether a declaration runs, but only when the test suite does.
    pub fn is_used_only_by_tests(&self, id: SymbolId) -> bool {
        self.reachable_symbols.contains(&id) && !self.production_symbols.contains(&id)
    }

    /// Whether a file is imported, but only from tests.
    pub fn is_module_used_only_by_tests(&self, id: ModuleId) -> bool {
        self.reachable_modules.contains(&id) && !self.production_modules.contains(&id)
    }

    pub fn has_unparsed_modules(&self) -> bool {
        self.modules.iter().any(|m| m.parse_failed)
    }

    /// Symbols of one module.
    pub fn symbols_of(&self, id: ModuleId) -> &[Symbol] {
        &self.symbols[self.module_symbols[id.0 as usize].clone()]
    }
}

/// Which entry points a traversal starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Roots {
    /// Every entry point, test files included. A test runner starts a test
    /// file, so code the tests reach does run.
    Everything,
    /// Entry points that are not tests. Code outside this set ships without
    /// ever being called.
    ProductionOnly,
}

impl Roots {
    fn accepts(self, module: &Module) -> bool {
        match self {
            Self::Everything => module.is_entry || module.is_test,
            Self::ProductionOnly => module.is_entry && !module.is_test,
        }
    }
}

/// Every declaration a reference could reach.
fn resolve_reference(
    reference: &Reference,
    scopes: &[FxHashMap<&str, SymbolId>],
    members_by_name: &FxHashMap<&str, Vec<SymbolId>>,
) -> Vec<SymbolId> {
    match reference.kind {
        ReferenceKind::Binding => scopes[reference.module.0 as usize]
            .get(reference.name.as_str())
            .copied()
            .into_iter()
            .collect(),
        // A member access reaches every same-named member. `ns.thing` on a
        // namespace import looks exactly like one; that case is handled by
        // treating the namespace-imported module as wildcarded.
        ReferenceKind::Member => members_by_name
            .get(reference.name.as_str())
            .cloned()
            .unwrap_or_default(),
        ReferenceKind::String => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scan::{reachable, symbol};

    /// A graph over the given files, with `entries` as the entry points.
    fn graph(files: &[(&str, &str)], entries: &[&str]) -> Graph {
        crate::test_scan::build(files, entries, &[])
    }

    fn find<'a>(g: &'a Graph, path: &str, name: &str) -> &'a Symbol {
        symbol(g, path, name)
    }

    #[test]
    fn module_reachability_follows_imports_from_entry_points() {
        let g = graph(
            &[
                ("index.ts", "import { a } from './used';\na();\n"),
                ("used.ts", "export function a() {}\n"),
                ("orphan.ts", "export function b() {}\n"),
            ],
            &["index.ts"],
        );
        assert!(g.is_module_reachable(ModuleId(0)));
        assert!(g.is_module_reachable(ModuleId(1)));
        assert!(
            !g.is_module_reachable(ModuleId(2)),
            "a file nothing imports and nothing starts is unreachable"
        );
    }

    #[test]
    fn a_side_effect_import_keeps_a_module_reachable() {
        let g = graph(
            &[
                ("index.ts", "import './polyfill';\n"),
                ("polyfill.ts", "globalThis.x = 1;\n"),
            ],
            &["index.ts"],
        );
        assert!(g.is_module_reachable(ModuleId(1)));
    }

    #[test]
    fn dead_code_cascades_through_the_reference_graph() {
        let g = graph(
            &[(
                "index.ts",
                "export function live() { liveHelper(); }\n\
                 function liveHelper() {}\n\
                 function dead() { deadHelper(); }\n\
                 function deadHelper() {}\n",
            )],
            &["index.ts"],
        );
        assert!(reachable(&g, "index.ts", "live"));
        assert!(reachable(&g, "index.ts", "liveHelper"));
        assert!(!reachable(&g, "index.ts", "dead"));
        assert!(
            !reachable(&g, "index.ts", "deadHelper"),
            "a helper called only from dead code is dead too"
        );
    }

    #[test]
    fn a_module_top_level_runs_when_the_module_is_imported() {
        let g = graph(
            &[
                ("index.ts", "import './register';\n"),
                (
                    "register.ts",
                    "function handler() {}\nregistry.add(handler);\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(
            reachable(&g, "register.ts", "handler"),
            "top-level code runs on import, so what it names is alive"
        );
    }

    #[test]
    fn an_export_nobody_imports_is_not_reached() {
        let g = graph(
            &[
                ("index.ts", "import { used } from './api';\nused();\n"),
                (
                    "api.ts",
                    "export function used() {}\nexport function neverImported() {}\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(reachable(&g, "api.ts", "used"));
        assert!(!reachable(&g, "api.ts", "neverImported"));
        assert!(g.is_export_imported(ModuleId(1), "used"));
        assert!(!g.is_export_imported(ModuleId(1), "neverImported"));
    }

    #[test]
    fn a_namespace_import_keeps_every_export_of_its_target_reachable() {
        let g = graph(
            &[
                ("index.ts", "import * as api from './api';\napi.one();\n"),
                (
                    "api.ts",
                    "export function one() {}\nexport function two() {}\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(g.is_wildcarded(ModuleId(1)));
        assert!(
            reachable(&g, "api.ts", "two"),
            "a namespace import can reach a name without writing it down"
        );
    }

    #[test]
    fn a_namespace_import_from_a_test_does_not_make_exports_production_reachable() {
        let g = crate::test_scan::build(
            &[
                ("index.ts", "export const shipped = 1;\n"),
                (
                    "api.ts",
                    "export function one() {}\nexport function two() {}\n",
                ),
                ("api.test.ts", "import * as api from './api';\napi.one();\n"),
            ],
            &["index.ts"],
            &["api.test.ts"],
        );
        assert!(g.is_module_reachable(ModuleId(1)), "the test imports it");
        assert!(
            !g.is_symbol_production_reachable(find(&g, "api.ts", "two").id),
            "only a test namespace-imports api.ts, so nothing ships that reaches `two`"
        );
        assert!(g.is_symbol_reachable(find(&g, "api.ts", "two").id));
    }

    #[test]
    fn renamed_exports_are_matched_by_the_name_they_are_imported_under() {
        let g = graph(
            &[
                ("index.ts", "import { outer } from './api';\nouter();\n"),
                (
                    "api.ts",
                    "function inner() {}\nexport { inner as outer };\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(reachable(&g, "api.ts", "inner"));
    }

    #[test]
    fn unused_import_bindings_are_identified() {
        let g = graph(
            &[
                (
                    "index.ts",
                    "import { used, unused } from './api';\nused();\n",
                ),
                (
                    "api.ts",
                    "export function used() {}\nexport function unused() {}\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(g.is_import_used(find(&g, "index.ts", "used").id));
        assert!(!g.is_import_used(find(&g, "index.ts", "unused").id));
    }

    #[test]
    fn a_live_class_keeps_the_chain_through_its_methods_open() {
        let g = graph(
            &[(
                "index.ts",
                "export class Widget {\n  render() { helper(); }\n}\n\
                 function helper() {}\n\
                 const w = new Widget();\nw.render();\n",
            )],
            &["index.ts"],
        );
        assert!(reachable(&g, "index.ts", "Widget"));
        assert!(
            reachable(&g, "index.ts", "helper"),
            "a class that runs runs its methods, and what they call runs too"
        );
    }

    #[test]
    fn a_framework_held_class_does_not_strand_the_helpers_its_methods_call() {
        // Nothing in the project calls `handle`: the HTTP server does. The
        // helper it calls has to stay alive all the same.
        let g = graph(
            &[
                (
                    "index.ts",
                    "import { Handler } from './h';\nserve(Handler);\n",
                ),
                (
                    "h.ts",
                    "function formatError() {}\nexport class Handler {\n  handle() { formatError(); }\n}\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(reachable(&g, "h.ts", "Handler"));
        assert!(reachable(&g, "h.ts", "formatError"));
    }

    #[test]
    fn reaching_a_method_reaches_the_class_that_owns_it() {
        let g = graph(
            &[(
                "index.ts",
                "class Helper {\n  run() {}\n}\nexport function go(h) { h.run(); }\n",
            )],
            &["index.ts"],
        );
        assert!(
            reachable(&g, "index.ts", "Helper"),
            "a class whose method is called is alive"
        );
    }

    #[test]
    fn python_modules_resolve_through_relative_imports() {
        let g = graph(
            &[
                ("app/__main__.py", "from .core import run\n\nrun()\n"),
                (
                    "app/core.py",
                    "def run():\n    _helper()\n\n\ndef _helper():\n    pass\n\n\ndef orphan():\n    pass\n",
                ),
            ],
            &["app/__main__.py"],
        );
        assert!(g.is_module_reachable(ModuleId(1)));
        assert!(reachable(&g, "app/core.py", "run"));
        assert!(reachable(&g, "app/core.py", "_helper"));
        assert!(!reachable(&g, "app/core.py", "orphan"));
    }

    #[test]
    fn magic_and_decorated_declarations_are_always_reachable() {
        let g = graph(
            &[(
                "app/handlers.py",
                "import app\n\n\nclass C:\n    def __init__(self):\n        pass\n\n\n@app.route(\"/x\")\ndef handler():\n    pass\n",
            )],
            &["app/handlers.py"],
        );
        assert!(reachable(&g, "app/handlers.py", "__init__"));
        assert!(
            reachable(&g, "app/handlers.py", "handler"),
            "a framework decorator took a handle to it"
        );
    }

    #[test]
    fn a_local_declaration_does_not_break_the_chain_to_what_it_calls() {
        let g = graph(
            &[
                ("index.ts", "import { api } from './api';\napi();\n"),
                (
                    "api.ts",
                    "const helper = (d: string) => d;\n\
                     export const api = () => {\n  const all = helper('x');\n  return all;\n};\n",
                ),
            ],
            &["index.ts"],
        );
        assert!(
            reachable(&g, "api.ts", "helper"),
            "a call made while initialising a local is still a call by the enclosing function"
        );
    }

    #[test]
    fn a_call_from_a_nested_function_counts_for_the_declaration_that_holds_it() {
        let g = graph(
            &[(
                "app/mod.py",
                "def helper():\n    pass\n\n\ndef outer():\n    def inner():\n        helper()\n\n    return inner\n\n\nouter()\n",
            )],
            &["app/mod.py"],
        );
        assert!(reachable(&g, "app/mod.py", "helper"));
    }

    #[test]
    fn importing_a_python_submodule_by_name_binds_the_module_itself() {
        let g = graph(
            &[
                ("pkg/__init__.py", ""),
                (
                    "pkg/__main__.py",
                    "from pkg import helpers\n\nhelpers.run()\n",
                ),
                ("pkg/helpers.py", "def run():\n    pass\n"),
            ],
            &["pkg/__main__.py"],
        );
        assert!(
            g.is_module_reachable(ModuleId(2)),
            "`from pkg import helpers` names pkg/helpers.py, not a name inside pkg/__init__.py"
        );
        assert!(reachable(&g, "pkg/helpers.py", "run"));
    }

    #[test]
    fn a_private_python_name_another_module_imports_is_used() {
        let g = graph(
            &[
                ("pkg/__init__.py", ""),
                ("pkg/__main__.py", "from pkg.caller import go\n\ngo()\n"),
                (
                    "pkg/caller.py",
                    "from pkg.llm import _call_with_retry\n\n\ndef go():\n    _call_with_retry()\n",
                ),
                (
                    "pkg/llm.py",
                    "def _call_with_retry():\n    _helper()\n\n\ndef _helper():\n    pass\n\n\ndef _never_called():\n    pass\n",
                ),
            ],
            &["pkg/__main__.py"],
        );
        assert!(
            reachable(&g, "pkg/llm.py", "_call_with_retry"),
            "the underscore says `do not depend on this`, not `this cannot be imported`"
        );
        assert!(
            reachable(&g, "pkg/llm.py", "_helper"),
            "and the chain continues past it"
        );
        assert!(!reachable(&g, "pkg/llm.py", "_never_called"));
    }

    #[test]
    fn a_commonjs_project_resolves_end_to_end() {
        let g = graph(
            &[
                (
                    "run.js",
                    "#!/usr/bin/env node\nconst { getPlatformKey } = require('./platform-map');\ngetPlatformKey();\n",
                ),
                (
                    "platform-map.js",
                    "function getPlatformKey() { return host(); }\n\
                     function host() {}\n\
                     function describeHost() {}\n\
                     function unexported() {}\n\
                     module.exports = { getPlatformKey, describeHost };\n",
                ),
                ("orphan.js", "module.exports = {};\n"),
            ],
            &["run.js"],
        );
        assert!(g.is_module_reachable(ModuleId(1)), "`require` is an import");
        assert!(!g.is_module_reachable(ModuleId(2)));
        assert!(reachable(&g, "platform-map.js", "getPlatformKey"));
        assert!(
            reachable(&g, "platform-map.js", "host"),
            "the chain continues"
        );
        assert!(
            !reachable(&g, "platform-map.js", "describeHost"),
            "exported through module.exports but nothing requires it by name"
        );
        assert!(!reachable(&g, "platform-map.js", "unexported"));
    }

    #[test]
    fn a_name_declared_in_two_modules_is_flagged_ambiguous() {
        let g = graph(
            &[
                ("index.ts", "export function shared() {}\n"),
                ("other.ts", "export function shared() {}\n"),
                ("third.ts", "export function unique() {}\n"),
            ],
            &["index.ts"],
        );
        assert!(g.is_name_ambiguous("shared"));
        assert!(!g.is_name_ambiguous("unique"));
    }

    #[test]
    fn string_literals_never_create_edges_but_are_remembered() {
        let g = graph(
            &[(
                "index.ts",
                "function handleRoot() {}\nexport const routes = { '/': 'handleRoot' };\n",
            )],
            &["index.ts"],
        );
        assert!(g.appears_in_string("handleRoot"));
        assert!(
            !reachable(&g, "index.ts", "handleRoot"),
            "a string is evidence for the confidence model, not a call"
        );
    }

    #[test]
    fn ids_stay_consistent_after_merging_several_modules() {
        let g = graph(
            &[
                ("a.ts", "export function one() {}\n"),
                ("b.ts", "export function two() {}\n"),
                ("c.ts", "export function three() {}\n"),
            ],
            &[],
        );
        for (index, symbol) in g.symbols.iter().enumerate() {
            assert_eq!(symbol.id.0 as usize, index, "symbol ids index the vector");
            assert!(
                g.symbols_of(symbol.module)
                    .iter()
                    .any(|s| s.id == symbol.id),
                "every symbol belongs to its module's range"
            );
        }
    }
}
