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
use crate::model::{Module, ModuleId};
use crate::resolve::ModuleIndex;
use std::path::PathBuf;

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
