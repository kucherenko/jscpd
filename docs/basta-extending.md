# Extending basta: adding a language

basta finds dead code by building a graph of every file in a scan and walking
it from the project's entry points. The graph, the reachability passes, the
confidence model, the classifier, the reporters and both command lines are
language-agnostic. Everything a language contributes goes through one trait,
[`Analyzer`](../rust/crates/basta/src/lang/mod.rs), and one registry line.
Adding a language is one new file under `rust/crates/basta/src/lang/`.

This guide is the contract that file has to honour, a worked skeleton, and the
way to prove the result is right. Read it once before writing an analyzer; the
invariants in [§3](#3-the-contract) are the ones that took real bugs to find.

## 1. How a run works

```
walk ──► read ──► analyze (per file, parallel) ──► graph (whole project) ──► classify ──► report
         │              │                                │                       │
     ModuleTraits   FileFacts                    reachable_modules          Finding + confidence
   is_self_starting  symbols / imports /          reachable_symbols
                     references                   (two passes: all roots,
                                                   production roots)
```

1. **Walk.** `cpd-finder`'s walker lists every file whose extension maps, via
   `cpd_tokenizer::formats`, to a format some analyzer serves. This is why
   [`Analyzer::formats`](../rust/crates/basta/src/lang/mod.rs) must name formats that table already knows.
2. **Read.** Each file is read once. The analyzer answers two questions from
   the path and the bytes alone: `module_traits(path)` and
   `is_self_starting(source)`.
3. **Analyze.** `analyze(input)` turns one file into `FileFacts` — its
   declarations (`Symbol`), its imports (`Import`) and the names it reads
   (`Reference`). Strictly one file: nothing here may look at another file,
   the filesystem, or the index.
4. **Entry points.** `entry.rs` decides which files the program starts from:
   the analyzer's `entry_globs()`, its manifest readers, the frameworks
   `framework.rs` detects (see [section 6](#6-things-that-look-like-analyzer-work-but-are-not)), the
   traits and self-starting flags from step 2, scripts in the tree that name
   a source file, and the user's `--entry` globs.
5. **Graph.** `graph.rs` merges every file's facts into one address space,
   asks each import's analyzer to `normalize_import` and `resolve` it, and
   runs breadth-first reachability twice — once from every entry point (tests
   included) and once from production entry points only. The difference is
   what "used only by tests" means.
6. **Classify and report.** `classify.rs` turns unreachable things into
   findings under five categories, `confidence.rs` scores each one, and the
   `cpd-reporter` crate renders them.

The design rule that follows from this: **an analyzer states facts; it never
decides.** Whether a declaration is dead is the graph's call, made with every
language's facts in hand. An analyzer that "helpfully" suppresses a symbol it
thinks is used, or invents a reference it thinks should exist, breaks the
cascade and the confidence model for everyone.

## 2. The trait

```rust
pub trait Analyzer: Send + Sync {
    // Required.
    fn language(&self) -> &'static str;                       // "ruby"
    fn formats(&self) -> &'static [&'static str];             // &["ruby"]
    fn analyze(&self, input: &AnalyzeInput<'_>) -> FileFacts;
    fn resolve(&self, specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId>;

    // Optional — defaults mean "nothing special".
    fn normalize_import(&self, import: &mut Import, importer: &Path, index: &ModuleIndex) {}
    fn entry_globs(&self) -> &'static [&'static str] { &[] }
    fn test_globs(&self) -> &'static [&'static str] { &[] }
    fn is_self_starting(&self, source: &str) -> bool { source.starts_with("#!") }
    fn manifests(&self) -> &'static [&'static str] { &[] }
    fn manifest_entries(&self, directory: &Path, manifest: &str, text: &str) -> Vec<PathBuf> { vec![] }
    fn manifest_signals(&self, manifest: &str, text: &str) -> ManifestSignals { ManifestSignals::default() }
    fn config_setting(&self, config: &str, text: &str, key: &str) -> Option<Setting> { None }
    fn alias_configs(&self) -> &'static [&'static str] { &[] }
    fn path_aliases(&self, directory: &Path, config: &str, text: &str) -> Vec<PathAlias> { vec![] }
    fn import_roots(&self, modules: &[PathBuf]) -> Vec<PathBuf> { vec![] }
    fn module_traits(&self, path: &str) -> ModuleTraits { ModuleTraits::default() }
}
```

| Method | Answers | Who consumes it |
| --- | --- | --- |
| `language` | The id in reports (`Finding.language`) | reporters |
| `formats` | Which jscpd formats this analyzer parses | walker, `--format`, `--list` |
| `analyze` | What one file declares, imports and reads | graph |
| `resolve` | Which scanned file a specifier names | graph |
| `normalize_import` | Rewrites an ambiguous import before resolution | graph |
| `entry_globs` | Files that are entry points by convention | entry detection |
| `test_globs` | File-name patterns of tests in this language | entry detection, classifier |
| `is_self_starting` | Does this file declare it runs on its own | entry detection |
| `manifests` / `manifest_entries` | Manifest files and what they name | entry detection |
| `manifest_signals` | The dependencies and sections a manifest declares | framework detection |
| `config_setting` | The literal a framework config written in this language assigns to a key | framework detection |
| `alias_configs` / `path_aliases` | Config files that rename import paths, and what they declare | resolution |
| `import_roots` | Directories the tree itself implies imports are rooted at | resolution |
| `module_traits` | Path-only facts: package surface, ambient, attribute reach | classifier, entry detection |

## 3. The contract

These are the invariants the graph relies on. Each one was found by a false
positive on a real codebase; the tests that pin them are named in brackets.

### Symbols

- **One `Symbol` per declaration the language would let a reader delete.**
  Functions, classes, methods, fields, module-level variables, imports,
  enums and their members, type aliases. Not parameters, not locals inside a
  function body, not destructured temporaries.
- **`TOP_LEVEL` means module scope.** The classifier reports only top-level
  declarations and class members; anything nested is covered by whatever
  encloses it. Get this flag right or nothing is reported.
- **`MEMBER` + `parent` for class members.** Members are matched by name
  across the whole scan (basta infers no types), so they are reported under
  a separate, opt-in category with a low base confidence.
- **Rebinding is one declaration.** `X = a` then `X = b` in the same scope is
  a single symbol, or the first copy is reported as unused while every reader
  resolves to the second. Key your dedup by *scope*, not by class: a nested
  `def helper` must not merge with the top-level `helper`.
  [`a_nested_def_does_not_swallow_a_top_level_name`]
- **`exported_as` is the name other modules import it by.** `export { a as b }`
  gives `name: "a", exported_as: Some("b")`. For languages without an
  `export` keyword, encode the convention: Python uses `__all__` when present
  and the underscore rule otherwise, and treats an import as exported only in
  an `__init__.py`, in `__all__`, or with an explicit re-export marker.
- **`MAGIC`, `DECORATED`, `ABSTRACT`, `OVERRIDE`** make a symbol a
  reachability root or lower its confidence. Set `DECORATED` only for
  decorators you do not recognise; `@property`-style decorators that never
  call the function themselves are inert. [`inert_decorators_do_not_count_as_framework_magic`]
- **`local_refs`** is the count of references from inside the file, minus
  the ones that only *export* the name. `export { a }` and
  `module.exports = { a }` are not uses of `a`.
  [`an_export_clause_is_not_a_use_of_the_symbol`]
- **A name you could not read is `<computed>` or similar.** Such a symbol is
  never reported, which is right: it cannot be matched against anything.

### Imports

- **One `Import` per module-level import, with the specifier exactly as
  written.** Resolution is `resolve`'s job, not `analyze`'s.
- **`kind` is what is taken:** `Named(name)`, `Default`, `Namespace` (the
  whole module object: `import * as`, `import m`, `const m = require()`),
  `StarReExport`, `SideEffect` (`import './x'`, bare `require('./x')`), and
  `Dynamic` for a specifier that is an expression. A `Namespace` or
  `StarReExport` marks the target *wildcarded*: every export of it may be
  reached without being named.
- **`local` is the binding the import creates,** as a `SymbolId` of kind
  `Import`. That is how the unused-import rule works. A destructured
  `const { a } = require('./m')` binds `a` — re-kind it as an import.
- **Both module systems, if the language has more than one.** CommonJS was
  invisible for a whole release because only ESM went through the module
  record. [`commonjs_require_forms_become_imports`]
- **Compiler directives are not imports.** `from __future__ import annotations`
  binds a name nobody can use.

### References

- **One `Reference` per identifier read,** with `from` set to the innermost
  *reportable* enclosing declaration — top level or member — not simply the
  innermost. A reference inside `const all = helper()` inside an exported
  function belongs to the function, not to `all`; `all` is a local nothing
  can ever reach, and attributing the call to it strands `helper`.
  [`a_local_declaration_does_not_break_the_chain_to_what_it_calls`]
- **`Binding` for a plain name, `Member` for `obj.name` / `obj["name"]`,
  `String` for an identifier-shaped string literal.** Strings never create
  edges; they lower confidence. Object-literal *keys* are definitions, not
  reads — do not record them as `Member`. [`an_object_literal_key_is_not_a_member_read`]
- **Decorators and class bases are read by the enclosing scope,** not by the
  declaration they decorate. A dead handler must not be the thing keeping
  its own `@app.route` alive. [`a_decorator_belongs_to_the_enclosing_scope_not_the_function_it_decorates`]
- **Assignment targets that are not plain names are reads.** `os.environ[k]
  = v` reads `os`. Tuple targets bind each element.
  [`an_attribute_assignment_reads_the_name_it_is_assigned_through`]
- **Store context is not a read.** `x = 1` does not use `x`.

### Dynamic access

Set `FileFacts::has_dynamic_access` when the file resolves names at runtime:
`eval`, `getattr`, `globals()`, `require(expr)`, `import(expr)`, computed
member access with a non-literal key. Every finding in such a file loses
confidence. Do not try to resolve these; record them.

### Parse failures

Return `FileFacts::unparsed()` and nothing else. The run continues, the file
is counted and listed, every finding in the run carries `UnparsedModule`, and
nothing inside the file is ever reported. Never return partial facts from a
parse that gave up — half a symbol table looks exactly like a file full of
dead code.

### Resolution

- **Consult only `ModuleIndex`.** `index.get(path)` says whether a file was
  scanned. A specifier that resolves to nothing is not an edge; that is the
  right answer for a dependency or the standard library. The one sanctioned
  exception is Python's package-root search, which has to see an
  `__init__.py` above the scan root; it is documented at the call site.
- **Use `resolve::normalize` for `.`/`..`** so a specifier climbing out of the
  scan cannot touch the filesystem or panic, and `resolve::append_extension`
  for `x` → `x.ts`.
- **Use `normalize_import` for forms the syntax leaves ambiguous.** Python's
  `from pkg import thing` is a name or a submodule; only the index knows.
- **Read the project's own aliases if the language has them.** Declare the
  config file in `alias_configs` and parse it in `path_aliases`; the
  collected `PathAlias`es reach every resolver through
  `index.against_aliases(specifier, importer, …)`. Targets must be absolute
  and the `scope` must be the directory of the config that declared them, so
  one package of a monorepo never borrows its neighbour's aliases.

  This is not a nicety. A project that renames its own import paths and is
  read without them looks almost entirely unreachable, and the findings come
  out *confident*, because nothing the resolver can see imports those files.
  A benchmark of ten trending repositories put basta at 0.96% wrong on nine
  of them and 36.5% on the one with a `"@/*"` alias, before this existed.
- **Derive the roots the tree implies, in `import_roots`.** Python's is the
  parent of every top-level package, which is how a `src/` layout resolves
  `import mypkg.thing` from a script that belongs to no package at all.
  Consulted once, after the index is built, and tried last — a scan root is
  what the user asked for, a derived root is an inference.
- **Read every position a name can hide in.** A type annotation may be a
  string (`def f(m: "torch.nn.Conv1d")`), and the `TYPE_CHECKING` idiom exists
  so that it *is* one. Parse those: before Python did, 275 of 834 unused-import
  findings on a trending corpus were wrong, all of them this.

## 4. A worked skeleton

The smallest analyzer that participates correctly. Replace the parser calls
with the language's; keep the shape.

```rust
//! Ruby, through <parser crate>.

use super::{AnalyzeInput, Analyzer, is_identifier_like};
use crate::model::{
    FileFacts, Import, ImportKind, ModuleId, ModuleTraits, Reference, ReferenceKind, Symbol,
    SymbolFlags, SymbolId, SymbolKind,
};
use crate::resolve::{ModuleIndex, append_extension, normalize};
use cpd_core::models::Location;
use cpd_tokenizer::line_index::LineIndex;
use std::path::{Path, PathBuf};

pub struct RubyAnalyzer;

impl Analyzer for RubyAnalyzer {
    fn language(&self) -> &'static str { "ruby" }

    // Must exist in cpd_tokenizer::formats — `ruby` covers .rb, .rake, ...
    fn formats(&self) -> &'static [&'static str] { &["ruby"] }

    fn analyze(&self, input: &AnalyzeInput<'_>) -> FileFacts {
        let Ok(tree) = parse(input.source) else {
            return FileFacts::unparsed();
        };
        let lines = LineIndex::new(input.source.as_bytes());
        let mut walk = Walk::new(input.module, &lines, input.source.len());
        walk.visit(&tree);
        walk.finish()
    }

    fn resolve(&self, specifier: &str, importer: &Path, index: &ModuleIndex) -> Option<ModuleId> {
        // `require_relative 'x'` → x.rb next to the importer.
        if let Some(relative) = specifier.strip_prefix("relative:") {
            let base = normalize(&importer.parent()?.join(relative));
            return index.get(&append_extension(&base, "rb"));
        }
        // `require 'lib/x'` → try each scan root, then a `lib/` layout.
        index.against_roots(specifier, |base| {
            index.get(&append_extension(base, "rb"))
        })
    }

    fn entry_globs(&self) -> &'static [&'static str] {
        &["**/config.ru", "**/Rakefile", "bin/*", "**/db/seeds.rb", "**/config/**/*.rb"]
    }

    fn test_globs(&self) -> &'static [&'static str] {
        &["**/*_spec.rb", "**/*_test.rb", "**/spec_helper.rb"]
    }

    fn manifests(&self) -> &'static [&'static str] { &["*.gemspec"] }

    fn manifest_entries(&self, directory: &Path, _: &str, text: &str) -> Vec<PathBuf> {
        // spec.files / spec.executables name what ships.
        gemspec_files(text).map(|f| directory.join(f)).collect()
    }

    fn module_traits(&self, path: &str) -> ModuleTraits {
        ModuleTraits {
            // Ruby reaches constants through the object model, so a
            // member-style read of a name anywhere is evidence.
            names_are_attributes: true,
            ..ModuleTraits::default()
        }
    }
}

struct Walk<'a> {
    symbols: Vec<Symbol>,
    imports: Vec<Import>,
    references: Vec<Reference>,
    module: ModuleId,
    lines: &'a LineIndex,
    len: usize,
    /// Open declarations, innermost last, each flagged reportable or not.
    stack: Vec<(SymbolId, bool)>,
    class_stack: Vec<SymbolId>,
    dynamic: bool,
}

impl<'a> Walk<'a> {
    fn loc(&self, offset: u32) -> Location {
        self.lines.location((offset as usize).min(self.len))
    }

    /// The declaration a reference belongs to: the innermost *reportable* one.
    fn current(&self) -> Option<SymbolId> {
        self.stack.iter().rev().find_map(|(id, ok)| ok.then_some(*id))
    }

    fn declare(&mut self, name: String, kind: SymbolKind, start: u32, end: u32, mut flags: SymbolFlags) -> SymbolId {
        let top_level = self.stack.is_empty();
        let member = kind.is_member();
        flags.set(SymbolFlags::TOP_LEVEL, top_level);
        flags.set(SymbolFlags::MEMBER, member);
        let (start, end) = (self.loc(start), self.loc(end));
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id, module: self.module, name, kind, flags,
            lines: end.line.saturating_sub(start.line) + 1,
            start, end,
            exported_as: None,          // set per the language's visibility rule
            parent: member.then(|| self.class_stack.last().copied()).flatten(),
            local_refs: 0,              // count after the walk
        });
        self.stack.push((id, top_level || member));
        id
    }

    fn reference(&mut self, name: String, kind: ReferenceKind, at: u32) {
        let from = self.current();
        let at = self.loc(at);
        self.references.push(Reference { module: self.module, name, kind, from, at });
    }

    // visit(): on a def → declare + push, walk body, pop.
    //          on a constant read → reference(Binding)
    //          on `x.y` → reference(Member, "y")
    //          on `require 'x'` → imports.push(Import { kind: SideEffect, .. })
    //          on `send(:name)` / `const_get` → self.dynamic = true
    //          on a string literal that is_identifier_like → remember it,
    //            emit as ReferenceKind::String in finish()
}
```

Then register it:

```rust
// rust/crates/basta/src/lang/mod.rs
pub mod ruby;
pub static ANALYZERS: &[&dyn Analyzer] =
    &[&javascript::JsAnalyzer, &python::PythonAnalyzer, &ruby::RubyAnalyzer];
```

That is the whole change outside the new file. `--list`, `--format ruby`,
the walker, the reporters and `jscpd --dead-code` pick it up from the
registry.

## 5. Proving it right

Accuracy is the product. A language that parses but resolves badly reports a
working application as dead, and a tool that does that gets turned off. Work
through these in order.

### Unit tests in the analyzer file

Every analyzer has the same test helpers — `facts(source)` and
`symbol(&facts, name)` — and covers the same ground. Copy the shape from
`python.rs`:

- declarations get their kinds, spans and `TOP_LEVEL`/`MEMBER` flags;
- the language's visibility rule sets `exported_as` correctly, including the
  cases where it must *not* (`_private`, a name outside `__all__`);
- every import form yields the right `ImportKind` and binds the right local;
- references are attributed to the enclosing declaration
  (`references_are_attributed_to_the_enclosing_declaration`);
- an export clause / re-export marker is not a use;
- each dynamic construct sets `has_dynamic_access`, and static access does not;
- an unparsable file yields `FileFacts::unparsed()`;
- `resolve` handles relative, project-absolute and outside-the-scan
  specifiers, and never panics on `../../..`;
- `module_traits`, `is_self_starting` and the manifest reader each have a
  positive and a negative case.

### Graph tests

Use `crate::test_scan::build(files, entries, tests)` — it takes `(path,
source)` pairs, picks the analyzer from the extension, and returns a resolved
`Graph`. It knows nothing about any particular language. Assert
reachability with `test_scan::reachable(&graph, path, name)`. At minimum:

- a module imported from an entry point is reachable and an orphan is not;
- dead code cascades (a helper called only from a dead function is dead);
- the top level of an imported module runs (its references are roots);
- a class's methods keep the chain open;
- your language's equivalent of a namespace import wildcards its target.

### A fixture and a README

Every feature ships a runnable demo under `fixtures/dead-code-demo/<language>/`
with one file per category — an unused file, an unused export, a cascading
unused symbol, an unused import — and a README section listing the exact
command and the exact `Found N dead code findings` line. Run every command
you write down. The smoke job scans `fixtures/` as a whole, so keep the
demo's prose and code unique.

### Real projects

Unit tests prove the parser; only real code proves the heuristics. Pick two
or three open-source projects in the language — one library, one
application, one with a framework — and:

1. Run `basta <project> --min-confidence 0` and read the *categories* first.
   Hundreds of unused imports means the import form is not recognised.
   Hundreds of unused symbols in files that are clearly used means an entry
   point or a resolution rule is missing.
2. Sample twenty findings at the default confidence and verify each by hand
   (`grep -rnw <name>`). Anything used is a bug in a rule, not in the
   project; find the rule.
3. Run at `--min-confidence 90` and expect near-100% precision there. If a
   guess is scoring 90, the confidence model is missing a reason — add a
   `Reason` in `cpd_core::deadcode`, an `Evidence` field, and the
   observation that sets it.
4. Cross-check against the language's own dead-code tool (vulture for
   Python, knip for TypeScript). Perfect agreement is not the goal — those
   tools report different things — but every finding basta has that they
   lack needs an explanation you can write down.

Record what you found in the PR: the projects, the counts before and after
each fix, and the false-positive classes you closed. The next language will
hit the same classes.

## 6. Things that look like analyzer work but are not

- **A new finding category.** That is a new `Category` in
  `cpd_core::deadcode`, a rule in `classify::category_for`, a base score in
  `confidence::base_score`, and a message. Analyzers do not report; they
  describe.
- **A new reason a finding might be wrong.** A `Reason` in `cpd_core::deadcode`
  with a penalty and an explanation, an `Evidence` field, and the line in
  `classify::gather_evidence` that sets it. An analyzer supplies the fact
  (usually a `SymbolFlags` bit); the classifier turns it into evidence.
- **A new framework.** What a framework starts without an import — routes,
  plugin directories, handlers by convention — is data, not code: one entry
  in `rust/crates/basta/frameworks.yaml`, whose header documents the schema
  (`detect` by config file, dependency or `package.json` section; `variables`
  read from the config; `bases`, `entry` globs, whole `directories`,
  `autoImports`). `framework::tests::the_built_in_table_is_valid` checks the
  table, and a `Framework::root(...)`/`reaches(...)` test pins what a
  definition roots. The analyzer's part is only reading: `manifest_signals`
  for the manifest, `config_setting` for a config written in its language. A
  project carries the same shape in `basta.frameworks.yaml` for frameworks
  that are nobody else's.
- **A new reporter.** `cpd-reporter/src/deadcode/`, registered in
  `create_dead_code_reporter`; the names must stay in step with the clone
  reporters.
- **A resolution feature that spans languages** — `tsconfig.json` `paths`,
  workspace aliases. Read the config in `resolve` of the language that owns
  the config; the `ModuleIndex` is deliberately language-neutral.

## 7. Known gaps worth picking up

Documented so a contributor can start from a known place rather than
rediscover them:

- Alias configs other than `tsconfig.json`/`jsconfig.json` are not read: a
  `@/x` declared only in `vite.config.ts`, `webpack.config.js` or a
  `package.json` `imports` map still resolves to nothing.
- An `extends` that names a package (`"extends": "@tsconfig/node20"`) is not
  followed, because it lives in `node_modules` and is never scanned.
- Re-export chains are not followed: `export { a } from './m'` keeps `m.a`
  alive even when nothing imports `a` from the barrel.
- TypeScript `namespace` bodies are not opened as scopes, so their contents
  are neither reported nor attributed.
- `Import.type_only`, `SymbolFlags::TYPE_ONLY`, `PRIVATE_NAME` and
  `IN_DUNDER_ALL` are recorded but no rule consults them yet.
- A file named by a path string is only seen when the string carries an
  explicit source extension (`"./worker.ts"`). A build config that assembles
  the path, or names a directory, is still invisible.
- Python module-level `for` / `with` / `except … as` bindings are not
  declared.
- PEP 420 namespace packages are not import roots: `from lib.context import x`
  resolves only when `lib/` carries an `__init__.py`, since any directory at
  all could otherwise be one.
- A name quoted anywhere other than an annotation is still only weak evidence.
  `cast("Widget", x)` and `TypeVar("T", bound="Widget")` do not count as reads.
- An export used only inside its own file is deliberately not reported; it
  is a style question, not dead code.
