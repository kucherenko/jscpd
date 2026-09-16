//! Language plug-ins.
//!
//! One [`Analyzer`] per language family. It is the *only* place a language
//! is allowed to be special: the walker, the graph, the confidence model, the
//! classifier, the reporters and both CLIs are language-agnostic, and they
//! learn everything they need about a language through this trait.
//!
//! An analyzer answers four questions about its language, and nothing else
//! in the crate answers any of them:
//!
//! 1. **What does one file declare, import and read?** — [`Analyzer::analyze`]
//!    turns a source file into [`FileFacts`], strictly within that file.
//!    Resolution across files happens once, in [`crate::graph`], on facts
//!    from every language at the same time.
//! 2. **Which file does a specifier name?** — [`Analyzer::resolve`], given the
//!    index of every module in the scan.
//! 3. **Where does a program in this language start?** — [`Analyzer::entry_globs`],
//!    [`Analyzer::is_self_starting`], and the manifests it can read. A
//!    language whose projects rename their own import paths also declares
//!    [`Analyzer::alias_configs`].
//! 4. **What can be told about a file from its path?** — [`Analyzer::module_traits`].
//!
//! The first two are required. The rest have defaults that mean "nothing
//! special", so a minimal language is one file and one line in [`ANALYZERS`].
//! See `docs/basta-extending.md` for the walk-through.

pub mod javascript;
pub mod python;

use crate::model::{FileFacts, Import, ModuleId, ModuleTraits};
use crate::resolve::{ModuleIndex, PathAlias};
use std::path::{Path, PathBuf};

/// Everything an [`Analyzer`] needs about the file it is given.
pub struct AnalyzeInput<'a> {
    /// Id the emitted facts must carry.
    pub module: ModuleId,
    /// jscpd format name, so one analyzer can serve several dialects.
    pub format: &'a str,
    /// Scan-root-relative display path.
    pub path: &'a str,
    pub source: &'a str,
}

/// A language plug-in.
pub trait Analyzer: Send + Sync {
    /// Stable, lower-case id that appears in reports: `js`, `python`.
    fn language(&self) -> &'static str;

    /// jscpd format names this analyzer serves. Every one must exist in
    /// `cpd_tokenizer::formats` — that table is how the walker maps file
    /// extensions to formats, so a format it does not know is never walked.
    fn formats(&self) -> &'static [&'static str];

    /// Declarations, imports and references of one file. An analyzer that
    /// cannot parse its input returns [`FileFacts::unparsed`] rather than an
    /// error: one broken file must not fail the run, but the graph has to
    /// know its references are unknown.
    fn analyze(&self, input: &AnalyzeInput<'_>) -> FileFacts;

    /// The module a specifier names, as seen from `importer`, or `None` when
    /// it names something outside the scan (a dependency, the standard
    /// library). Only `index` may be consulted for what exists; the resolver
    /// must not invent files.
    fn resolve(&self, specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId>;

    /// A chance to rewrite an import before it is resolved, for languages
    /// where the written form is ambiguous. Python's `from pkg import x`
    /// names either a symbol in `pkg/__init__.py` or the module `pkg/x.py`,
    /// and only the index can tell which.
    fn normalize_import(&self, _import: &mut Import, _importer: &Path, _index: &ModuleIndex) {}

    /// Globs, against scan-root-relative paths, of files that are entry
    /// points by convention: `src/index.ts`, `__main__.py`, a framework's
    /// route directory. A glob without a leading `**/` also matches at any
    /// depth.
    fn entry_globs(&self) -> &'static [&'static str] {
        &[]
    }

    /// Globs of files that are tests, fixtures or examples in this language
    /// (`*.test.ts`, `test_*.py`). Directory conventions shared by every
    /// language (`tests/`, `__tests__/`, `fixtures/`) are built in.
    fn test_globs(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether a file declares that it runs on its own. A shebang says so in
    /// any language; Python adds `if __name__ == "__main__"`.
    fn is_self_starting(&self, source: &str) -> bool {
        source.starts_with("#!")
    }

    /// Manifest file names this analyzer can read for entry points
    /// (`package.json`, `pyproject.toml`). Looked for in every directory
    /// that holds a scanned file, and in the scan roots.
    fn manifests(&self) -> &'static [&'static str] {
        &[]
    }

    /// Absolute paths a manifest names as entry points. `directory` holds
    /// the manifest; `text` is its contents. Paths need not exist — a
    /// manifest often names a built file, and the analyzer should return the
    /// sources it could have been built from as well.
    fn manifest_entries(&self, _directory: &Path, _manifest: &str, _text: &str) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Config files that declare import path aliases (`tsconfig.json`).
    /// Looked for in the same directories as [`Analyzer::manifests`].
    fn alias_configs(&self) -> &'static [&'static str] {
        &[]
    }

    /// Path aliases one such config declares. `directory` holds the config;
    /// `text` is its contents. Targets must be absolute — join them to
    /// `directory` — and need not exist, since the index decides that.
    fn path_aliases(&self, _directory: &Path, _config: &str, _text: &str) -> Vec<PathAlias> {
        Vec::new()
    }

    /// Directories absolute imports may be rooted at, derived from the files
    /// in the scan. A Python `src/` layout puts the package root at `src/`,
    /// which no scan root and no importer's own package can reveal.
    /// Consulted once, after the index is built.
    fn import_roots(&self, _modules: &[PathBuf]) -> Vec<PathBuf> {
        Vec::new()
    }

    /// What the path alone says about a file. See [`ModuleTraits`].
    fn module_traits(&self, _path: &str) -> ModuleTraits {
        ModuleTraits::default()
    }
}

/// Registered analyzers, consulted in order. Add new languages here.
pub static ANALYZERS: &[&dyn Analyzer] = &[&javascript::JsAnalyzer, &python::PythonAnalyzer];

/// The analyzer serving `format`, if any.
pub fn analyzer_for(format: &str) -> Option<&'static dyn Analyzer> {
    ANALYZERS
        .iter()
        .copied()
        .find(|a| a.formats().contains(&format))
}

/// True when basta can analyze this jscpd format.
pub fn supports(format: &str) -> bool {
    analyzer_for(format).is_some()
}

/// Every format basta analyzes, sorted, for `--list` and error messages.
pub fn supported_formats() -> Vec<&'static str> {
    let mut formats: Vec<&'static str> = ANALYZERS
        .iter()
        .flat_map(|a| a.formats().iter().copied())
        .collect();
    formats.sort_unstable();
    formats
}

/// Every file extension basta analyzes, from the tokenizer's format table.
/// Used wherever a path has to be recognised as source without parsing it —
/// a `package.json` script line, a `files` entry.
pub fn supported_extensions() -> Vec<&'static str> {
    let formats = supported_formats();
    let mut extensions: Vec<&'static str> = cpd_tokenizer::formats::SUPPORTED_FORMATS
        .iter()
        .filter(|entry| formats.contains(&entry.name))
        .flat_map(|entry| entry.extensions.iter().copied())
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    extensions
}

/// True when `path` ends in an extension some analyzer serves.
pub fn is_source_path(path: &str) -> bool {
    supported_extensions()
        .iter()
        .any(|extension| path.ends_with(&format!(".{extension}")))
}

/// The language id serving `format`, if any.
pub fn language_of(format: &str) -> Option<&'static str> {
    analyzer_for(format).map(|a| a.language())
}

/// True for a string that could be the name of a declaration.
///
/// Used on string literals, to decide whether one is worth remembering as
/// weak evidence that a name may be looked up at runtime. `$` is accepted for
/// JavaScript's sake; it cannot appear in a Python identifier, so a Python
/// string containing one simply never matches a declaration.
///
/// The length cap keeps a data-heavy file — a fixture full of prose, a
/// generated table of strings — from filling the set with things no
/// declaration could be called.
pub fn is_identifier_like(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 100
        && text.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '$')
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_format_has_exactly_one_analyzer() {
        let formats = supported_formats();
        let mut seen = std::collections::HashSet::new();
        for f in &formats {
            assert!(seen.insert(*f), "format {f} is claimed by two analyzers");
            assert!(supports(f), "{f} must resolve");
        }
    }

    #[test]
    fn every_analyzer_format_is_one_the_walker_knows() {
        // The walker maps extensions to formats through the tokenizer's
        // table. An analyzer claiming a format that table lacks would compile,
        // register, and never see a single file.
        let known = cpd_tokenizer::formats::list_formats();
        for analyzer in ANALYZERS {
            for format in analyzer.formats() {
                assert!(
                    known.contains(format),
                    "{} claims format {format:?}, which cpd_tokenizer::formats does not define",
                    analyzer.language()
                );
            }
        }
    }

    #[test]
    fn language_ids_are_stable_lower_case_and_distinct() {
        let mut ids: Vec<&str> = ANALYZERS.iter().map(|a| a.language()).collect();
        for id in &ids {
            assert!(
                !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase()),
                "{id:?} is not a lower-case ascii id"
            );
        }
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "two analyzers share a language id");
    }

    #[test]
    fn supported_extensions_come_from_the_format_table() {
        let extensions = supported_extensions();
        for want in ["ts", "tsx", "js", "mjs", "py", "pyi"] {
            assert!(
                extensions.contains(&want),
                "{want} missing from {extensions:?}"
            );
        }
        assert!(!extensions.contains(&"java"));
        assert!(is_source_path("src/a.tsx"));
        assert!(is_source_path("pkg/mod.py"));
        assert!(!is_source_path("README.md"));
    }

    #[test]
    fn identifier_shaped_strings_are_told_from_prose() {
        for yes in ["handleRoot", "_private", "$el", "a1", "run_phase"] {
            assert!(is_identifier_like(yes), "{yes}");
        }
        for no in [
            "",
            "1abc",
            "has space",
            "path/to/file",
            "kebab-case",
            &"x".repeat(101),
        ] {
            assert!(!is_identifier_like(no), "{no:?}");
        }
    }

    #[test]
    fn known_formats_map_to_the_right_language() {
        for f in ["javascript", "typescript", "jsx", "tsx"] {
            assert_eq!(language_of(f), Some("js"), "{f}");
        }
        assert_eq!(language_of("python"), Some("python"));
        assert_eq!(language_of("ruby"), None);
    }

    #[test]
    fn the_trait_defaults_mean_nothing_special() {
        // A minimal analyzer: two required methods and the defaults.
        struct Minimal;
        impl Analyzer for Minimal {
            fn language(&self) -> &'static str {
                "minimal"
            }
            fn formats(&self) -> &'static [&'static str] {
                &["minimal"]
            }
            fn analyze(&self, _: &AnalyzeInput<'_>) -> FileFacts {
                FileFacts::default()
            }
            fn resolve(&self, _: &str, _: &Path, _: &ModuleIndex) -> Option<ModuleId> {
                None
            }
        }
        let m = Minimal;
        assert!(m.entry_globs().is_empty());
        assert!(m.test_globs().is_empty());
        assert!(m.manifests().is_empty());
        assert!(m.is_self_starting("#!/usr/bin/env thing\n"));
        assert!(!m.is_self_starting("plain source\n"));
        assert_eq!(m.module_traits("any/file.minimal"), ModuleTraits::default());
        let index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        assert!(m.manifest_entries(Path::new("/p"), "x.toml", "").is_empty());
        assert!(
            m.resolve("./x", Path::new("/p/a.minimal"), &index)
                .is_none()
        );
    }
}
