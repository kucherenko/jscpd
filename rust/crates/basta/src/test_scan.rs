//! Building a resolved [`Graph`] from source strings, for tests.
//!
//! The graph is a whole-project object, so testing anything above the
//! analyzers means assembling a project. Every test that needs one assembles
//! it the same way, and doing that in one place keeps a change to
//! [`ModuleInput`] from rippling through every test module.
//!
//! Nothing here knows a language: the format comes from the tokenizer's
//! extension table and everything else from the analyzer that serves it, so
//! a new language's tests can use this builder unchanged.

use crate::graph::{Graph, ModuleInput};
use crate::lang::{AnalyzeInput, analyzer_for};
use crate::model::{FileFacts, Import, ImportKind, Module, ModuleId, Reference, ReferenceKind};
use crate::resolve::ModuleIndex;
use std::path::{Path, PathBuf};

/// The scan root test paths are relative to. Nothing is read from disk, so it
/// need not exist.
pub const ROOT: &str = "/p";

/// Build a graph from `(path, source)` pairs, treating `entries` as entry
/// points and `tests` as test files.
pub fn build(files: &[(&str, &str)], entries: &[&str], tests: &[&str]) -> Graph {
    let mut index = ModuleIndex::new(vec![PathBuf::from(ROOT)]);
    let mut inputs = Vec::with_capacity(files.len());
    for (position, (path, source)) in files.iter().enumerate() {
        let id = ModuleId(position as u32);
        let extension = path.rsplit('.').next().unwrap_or("");
        let format = cpd_tokenizer::formats::get_format_by_extension(extension)
            .unwrap_or_else(|| panic!("no format for {path}"));
        let analyzer = analyzer_for(format).unwrap_or_else(|| panic!("no analyzer for {path}"));
        let real_path = PathBuf::from(ROOT).join(path);
        index.insert(real_path.clone(), id);
        let facts = analyzer.analyze(&AnalyzeInput {
            module: id,
            format,
            path,
            source,
        });
        inputs.push(ModuleInput {
            module: Module {
                id,
                path: (*path).to_string(),
                real_path,
                format: format.to_string(),
                language: analyzer.language(),
                traits: analyzer.module_traits(path),
                lines: source.lines().count().max(1) as u32,
                is_entry: false,
                is_test: false,
                has_dynamic_access: false,
                parse_failed: false,
            },
            facts,
            is_entry: entries.contains(path),
            is_test: tests.contains(path),
        });
    }
    Graph::build(inputs, &index)
}

/// A directory tree written under the system temp directory for one test,
/// and removed when the test ends — including when it fails, which a trailing
/// `remove_dir_all` never reaches.
pub struct TempTree(PathBuf);

impl TempTree {
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("basta-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("a temp directory");
        Self(root)
    }

    /// Write `text` to `relative`, creating the directories it needs.
    pub fn write(&self, relative: &str, text: &str) -> &Self {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(path, text).expect("a file");
        self
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// An index over absolute test paths, each given the id of its position.
pub fn index_of(files: &[&str]) -> ModuleIndex {
    let mut index = ModuleIndex::new(vec![PathBuf::from(ROOT)]);
    for (position, file) in files.iter().enumerate() {
        index.insert(PathBuf::from(file), ModuleId(position as u32));
    }
    index
}

/// The import of `specifier` taking this shape, or a panic listing what the
/// file does import.
pub fn import<'a>(facts: &'a FileFacts, specifier: &str, kind: &ImportKind) -> &'a Import {
    facts
        .imports
        .iter()
        .find(|i| i.specifier == specifier && &i.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} import of {specifier} in {:?}", facts.imports))
}

/// The reference that reads `name` as a binding.
pub fn binding_reference<'a>(facts: &'a FileFacts, name: &str) -> &'a Reference {
    facts
        .references
        .iter()
        .find(|r| r.name == name && r.kind == ReferenceKind::Binding)
        .unwrap_or_else(|| panic!("no reference to {name}"))
}

/// Every name the file reads off an object.
pub fn member_reads(facts: &FileFacts) -> Vec<&str> {
    facts
        .references
        .iter()
        .filter(|r| r.kind == ReferenceKind::Member)
        .map(|r| r.name.as_str())
        .collect()
}

/// The symbol named `name` declared in `path`.
pub fn symbol<'a>(graph: &'a Graph, path: &str, name: &str) -> &'a crate::model::Symbol {
    graph
        .symbols
        .iter()
        .find(|s| s.name == name && graph.module(s.module).path == path)
        .unwrap_or_else(|| panic!("no symbol {name} in {path}"))
}

/// Whether anything at all reaches the symbol named `name` in `path`.
pub fn reachable(graph: &Graph, path: &str, name: &str) -> bool {
    graph.is_symbol_reachable(symbol(graph, path, name).id)
}
