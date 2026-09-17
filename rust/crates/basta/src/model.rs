//! The data model shared by every language analyzer and by the graph.
//!
//! A run produces one [`Module`] per analyzed file and one [`Symbol`] per
//! declaration inside it. Analyzers never resolve anything across files: they
//! emit [`Symbol`]s, [`Import`]s and [`Reference`]s in module-local terms, and
//! [`crate::graph`] links them. That split is what keeps a new language to one
//! file under `lang/`.

use cpd_core::models::Location;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// The report-facing vocabulary lives in `cpd-core`, beside the clone models,
// so reporters can render a run without linking the analyzer. Analyzers use
// the same types rather than a parallel set that would need translating.
pub use cpd_core::deadcode::SymbolKind;

/// Index of a module in [`crate::graph::Graph::modules`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModuleId(pub u32);

/// Index of a symbol in [`crate::graph::Graph::symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolId(pub u32);

/// Properties an analyzer observed about a declaration that the graph and the
/// confidence model need but that are not worth a field each.
///
/// Not every flag drives a rule yet. `TYPE_ONLY`, `PRIVATE_NAME` and
/// `IN_DUNDER_ALL` are recorded because an analyzer knows them cheaply and a
/// future rule (or a reporter) may want them; nothing in `graph` or
/// `classify` consults them today. Setting them is correct, relying on them
/// is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SymbolFlags(u16);

impl SymbolFlags {
    pub const NONE: Self = Self(0);
    /// Declared with `export default` / is the module's default export.
    pub const DEFAULT_EXPORT: Self = Self(1 << 0);
    /// A TypeScript `export type` / `import type` binding: erased at runtime.
    pub const TYPE_ONLY: Self = Self(1 << 1);
    /// Carries a decorator (JS `@dec`, Python `@dec`) that the analyzer did not
    /// recognise. Frameworks call decorated symbols reflectively.
    pub const DECORATED: Self = Self(1 << 2);
    /// Declared inside a class body.
    pub const MEMBER: Self = Self(1 << 3);
    /// Python: listed in the module's `__all__`.
    pub const IN_DUNDER_ALL: Self = Self(1 << 4);
    /// A dunder / magic name the runtime calls for you (`__init__`, `__iter__`).
    pub const MAGIC: Self = Self(1 << 5);
    /// Name begins with `_` (Python) or `#` (JS private field).
    pub const PRIVATE_NAME: Self = Self(1 << 6);
    /// Declared at module top level rather than nested in another symbol.
    pub const TOP_LEVEL: Self = Self(1 << 7);
    /// Overrides or implements an inherited member: the base class decides
    /// whether it is called.
    pub const OVERRIDE: Self = Self(1 << 8);
    /// An abstract declaration — the implementation lives in subclasses.
    pub const ABSTRACT: Self = Self(1 << 9);
    /// An import the module declares in order to re-export it: a redundant
    /// alias (`from m import x as x`) or a `# noqa: F401`.
    pub const RE_EXPORT: Self = Self(1 << 10);

    /// True when every bit of `other` is set here.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// True when any bit of `other` is set here.
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Set or clear every bit of `other`.
    #[inline]
    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for SymbolFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for SymbolFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One analyzed source file.
#[derive(Debug, Clone)]
pub struct Module {
    pub id: ModuleId,
    /// Scan-root-relative display path: the id reports print.
    pub path: String,
    /// Absolute path the bytes were read from, used for module resolution.
    pub real_path: PathBuf,
    /// jscpd format name (`typescript`, `python`, ...).
    pub format: String,
    /// The analyzer's language id (`js`, `python`), for reports.
    pub language: &'static str,
    /// What the analyzer knows about this file from its path alone.
    pub traits: ModuleTraits,
    pub lines: u32,
    /// The file matched an entry-point rule, so its exports are a public API.
    pub is_entry: bool,
    /// The file looks like a test, fixture or example.
    pub is_test: bool,
    /// The module accesses names it computes at runtime (`eval`, `getattr`,
    /// `globals()`, `require(expr)`, computed member access). Every finding in
    /// such a module loses confidence.
    pub has_dynamic_access: bool,
    /// The analyzer could not parse the file. Its symbols are unknown, so the
    /// module is treated as referencing everything it imports.
    pub parse_failed: bool,
}

/// What an analyzer can tell about a file before reading it — from its path,
/// its name, its place in the tree. These feed entry-point detection and the
/// classifier, and they are the only language knowledge those two ever get.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModuleTraits {
    /// The file is an entry point by the language's own convention, whatever
    /// the project's manifests say: a Python package's `__init__.py`.
    pub entry_point: bool,
    /// The file exists to re-export a package's surface (`index.ts`,
    /// `__init__.py`). Findings inside it lose confidence: a re-export that
    /// nothing in the scan imports is often a deliberate public API.
    pub package_surface: bool,
    /// Every declaration in the file is ambient — declared for a compiler,
    /// never imported by a module (`.d.ts`). Nothing inside is reported.
    pub ambient_declarations: bool,
    /// Top-level names are reachable as attributes of the module object
    /// (`mod.helper()` in Python), so a member-style read of a name anywhere
    /// in the scan is evidence the declaration may be used.
    pub names_are_attributes: bool,
}

/// A declaration inside a module.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub module: ModuleId,
    /// Declared name. For a default export with no name this is `default`.
    pub name: String,
    pub kind: SymbolKind,
    pub flags: SymbolFlags,
    /// Start of the declaration (the name, not the leading decorators).
    pub start: Location,
    /// End of the whole declaration body.
    pub end: Location,
    /// The name this symbol is visible under outside its module, when it is
    /// exported at all. `export { a as b }` gives `name = "a"`, `exported_as
    /// = Some("b")`.
    pub exported_as: Option<String>,
    /// Enclosing class or enum, for members.
    pub parent: Option<SymbolId>,
    /// References to this symbol from within its own module, excluding the
    /// declaration itself.
    pub local_refs: u32,
    /// Lines of code the declaration spans.
    pub lines: u32,
}

impl Symbol {
    /// True when the symbol is visible to other modules.
    pub fn is_exported(&self) -> bool {
        self.exported_as.is_some()
    }

    /// The name other modules import this symbol by.
    pub fn export_name(&self) -> Option<&str> {
        self.exported_as.as_deref()
    }
}

/// A module-level `import` / `from … import …` / `require` / re-export.
#[derive(Debug, Clone)]
pub struct Import {
    /// Module doing the importing.
    pub module: ModuleId,
    /// The specifier exactly as written (`./utils`, `os.path`, `lodash`).
    pub specifier: String,
    /// What is taken from the target module.
    pub kind: ImportKind,
    /// The local binding the import creates, when it creates one. `None` for
    /// bare side-effect imports (`import './polyfill'`).
    pub local: Option<SymbolId>,
    pub start: Location,
    /// A TypeScript `import type` — erased at runtime. Recorded for
    /// completeness; the graph currently follows type-only imports like any
    /// other, since a file imported only for its types is still needed.
    pub type_only: bool,
}

/// What an [`Import`] takes from its target module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    /// A named export: `import { a } from`, `from m import a`.
    Named(String),
    /// The default export: `import a from`.
    Default,
    /// The whole namespace: `import * as a from`, `import m`.
    Namespace,
    /// `export * from 'm'` — every name of the target becomes a name here.
    StarReExport,
    /// `import './side-effect'` — keeps the module alive but names nothing.
    SideEffect,
    /// A specifier computed at runtime (`import(expr)`, `__import__(name)`).
    /// Nothing can be resolved; the enclosing module gets
    /// [`Module::has_dynamic_access`].
    Dynamic,
    /// A specifier that stands for a set of files: ``import(`./locales/${name}.json`)``
    /// or `import.meta.glob('./pages/*.vue')`. A bundler expands it at build
    /// time to every file the pattern matches, so every one of them is
    /// reachable; the specifier holds the pattern, `./locales/*.json`.
    Glob,
}

/// A use of a name inside a module body, before resolution.
///
/// Analyzers emit one per identifier read. [`crate::graph`] turns them into
/// edges; unresolved ones become string evidence for the confidence model.
#[derive(Debug, Clone)]
pub struct Reference {
    pub module: ModuleId,
    /// Name being read. For `a.b()` the analyzer emits both `a` (a binding
    /// read) and a [`ReferenceKind::Member`] entry for `b`.
    pub name: String,
    pub kind: ReferenceKind,
    /// The innermost declaration whose body contains this reference, when the
    /// reference is not at module top level.
    pub from: Option<SymbolId>,
    pub at: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A plain identifier read that resolves through scoping.
    Binding,
    /// A property access (`obj.name`, `obj["name"]`): matched against member
    /// symbols by name only, since basta does no type inference.
    Member,
    /// The name appeared inside a string literal. Weak evidence, used only to
    /// lower confidence, never to mark a symbol used.
    String,
}

/// Everything one analyzer learned about one file.
#[derive(Debug, Default)]
pub struct FileFacts {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub references: Vec<Reference>,
    /// The file resolves names at runtime (`eval`, `getattr`, `require(x)`).
    pub has_dynamic_access: bool,
    /// The parser gave up. `symbols`, `imports` and `references` are then
    /// empty and the graph treats the file's references as unknown.
    pub parse_failed: bool,
}

impl FileFacts {
    /// The facts for a file the analyzer could not read.
    pub fn unparsed() -> Self {
        Self {
            parse_failed: true,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_contain_and_combine() {
        let mut f = SymbolFlags::TOP_LEVEL;
        assert!(f.contains(SymbolFlags::TOP_LEVEL));
        assert!(!f.contains(SymbolFlags::MAGIC));
        f.insert(SymbolFlags::MAGIC);
        assert!(f.contains(SymbolFlags::TOP_LEVEL | SymbolFlags::MAGIC));
        assert!(f.intersects(SymbolFlags::MAGIC | SymbolFlags::ABSTRACT));
    }

    #[test]
    fn flags_set_clears_a_bit() {
        let mut f = SymbolFlags::TOP_LEVEL | SymbolFlags::PRIVATE_NAME;
        f.set(SymbolFlags::PRIVATE_NAME, false);
        assert!(!f.contains(SymbolFlags::PRIVATE_NAME));
        assert!(f.contains(SymbolFlags::TOP_LEVEL));
    }

    #[test]
    fn empty_flags_are_empty() {
        assert!(SymbolFlags::NONE.is_empty());
        assert!(!SymbolFlags::MAGIC.is_empty());
    }

    #[test]
    fn member_and_type_kinds() {
        assert!(SymbolKind::Method.is_member());
        assert!(SymbolKind::EnumMember.is_member());
        assert!(!SymbolKind::Function.is_member());
        assert!(SymbolKind::TypeAlias.is_type_only());
        assert!(!SymbolKind::Class.is_type_only());
    }
}
