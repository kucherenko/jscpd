//! The index every resolver answers against, and the path arithmetic they
//! share.
//!
//! basta resolves only what it can see. A specifier that lands outside the
//! scan — a published package, a standard-library module, a generated file
//! that was never walked — resolves to nothing, and the import that names it
//! is simply not an edge in the module graph. That is the honest answer: a
//! file basta never read cannot be evidence that anything is alive or dead.
//!
//! How a specifier turns into a path is the language's business and lives
//! with the language, in [`crate::lang::Analyzer::resolve`]. This module
//! provides what every such resolver needs: the set of modules that exist,
//! the scan roots, the import path aliases the project declares, and the
//! string-only path helpers that keep a specifier climbing out of the scan
//! from touching the filesystem.

use crate::model::ModuleId;
use rustc_hash::FxHashMap;
use std::path::{Component, Path, PathBuf};

/// One import path alias a project declares for itself.
///
/// `"@/*": ["./*"]` in a `tsconfig.json` is the canonical example, and it is
/// not a niche convenience: it ships in the default Next.js template, so a
/// resolver that ignores it reports most of a live application as unreachable
/// — with full confidence, because nothing it can see imports those files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAlias {
    /// Directory of the config that declared it. The alias applies only to
    /// importers under this directory, so two packages of a monorepo may
    /// give the same prefix different meanings.
    pub scope: PathBuf,
    /// Literal text before the `*`: `@/` for `@/*`, or the whole pattern
    /// when it has no wildcard.
    pub prefix: String,
    /// Whether the pattern ended in `*` and so matches a suffix.
    pub wildcard: bool,
    /// Absolute paths the pattern maps to, in the order declared. For a
    /// wildcard these are base directories the matched suffix is joined to.
    pub targets: Vec<PathBuf>,
}

impl PathAlias {
    /// The paths this alias says `specifier` could name, most-preferred
    /// first, or an empty vector when the pattern does not match.
    pub fn apply(&self, specifier: &str) -> Vec<PathBuf> {
        let suffix = match self.wildcard {
            true => match specifier.strip_prefix(&self.prefix) {
                // `@/*` must not swallow a bare `@`; a wildcard stands for at
                // least one character, as TypeScript reads it.
                Some(rest) if !rest.is_empty() => rest,
                _ => return Vec::new(),
            },
            false if specifier == self.prefix => "",
            false => return Vec::new(),
        };
        self.targets
            .iter()
            .map(|target| match suffix.is_empty() {
                true => target.clone(),
                false => normalize(&target.join(suffix)),
            })
            .collect()
    }

    /// How specific this alias is. A deeper config and a longer prefix both
    /// win, which is how TypeScript picks between overlapping patterns.
    fn specificity(&self) -> (usize, usize) {
        (self.scope.components().count(), self.prefix.len())
    }
}

/// Every module in a scan, by canonical path, plus the scan roots and the
/// path aliases the project declares.
pub struct ModuleIndex {
    by_path: FxHashMap<PathBuf, ModuleId>,
    roots: Vec<PathBuf>,
    aliases: Vec<PathAlias>,
    import_roots: Vec<PathBuf>,
}

impl ModuleIndex {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            by_path: FxHashMap::default(),
            roots,
            aliases: Vec::new(),
            import_roots: Vec::new(),
        }
    }

    /// Install the directories absolute imports may additionally be rooted at.
    pub fn set_import_roots(&mut self, mut roots: Vec<PathBuf>) {
        roots.sort();
        roots.dedup();
        self.import_roots = roots;
    }

    /// Directories a language derived from the scanned files themselves: the
    /// `src` of a src-layout, the parent of each top-level package. Tried
    /// after [`ModuleIndex::roots`], which are what the user asked for.
    pub fn import_roots(&self) -> &[PathBuf] {
        &self.import_roots
    }

    /// Every module path in the scan, for a language that has to derive
    /// something from the shape of the tree.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.by_path.keys().map(PathBuf::as_path)
    }

    /// Install the project's path aliases, most specific first.
    pub fn set_aliases(&mut self, mut aliases: Vec<PathAlias>) {
        aliases.sort_by_key(|alias| std::cmp::Reverse(alias.specificity()));
        self.aliases = aliases;
    }

    /// The aliases in effect, most specific first.
    pub fn aliases(&self) -> &[PathAlias] {
        &self.aliases
    }

    /// Register a module at its canonical path.
    pub fn insert(&mut self, path: PathBuf, module: ModuleId) {
        self.by_path.insert(path, module);
    }

    /// The module at exactly this path, if one was scanned.
    pub fn get(&self, path: &Path) -> Option<ModuleId> {
        self.by_path.get(path).copied()
    }

    /// True when a file at this path was scanned. The same question as
    /// [`ModuleIndex::get`], for callers that only need the answer.
    pub fn contains(&self, path: &Path) -> bool {
        self.by_path.contains_key(path)
    }

    /// Canonical scan roots, in the order given. A resolver tries these for
    /// project-absolute specifiers (`baseUrl`-style imports, a Python package
    /// named by its full dotted path).
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Every module inside `directory`, at any depth.
    ///
    /// Used for the one specifier shape that names a directory rather than a
    /// file: a bundler expanding ``import(`./dir/${x}.js`)`` reaches all of
    /// them, so all of them are live.
    /// `keep` sees each path relative to `directory`.
    pub fn under(&self, directory: &Path, keep: impl Fn(&Path) -> bool) -> Vec<ModuleId> {
        let mut found: Vec<(&PathBuf, ModuleId)> = self
            .by_path
            .iter()
            .filter(|(path, _)| path.strip_prefix(directory).is_ok_and(&keep))
            .map(|(path, id)| (path, *id))
            .collect();
        // Sorted so a run does not depend on hash order.
        found.sort_unstable();
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// The first root under which `relative` resolves through `try_at`.
    pub fn against_roots(
        &self,
        relative: &str,
        try_at: impl Fn(&Path) -> Option<ModuleId>,
    ) -> Option<ModuleId> {
        self.roots
            .iter()
            .find_map(|root| try_at(&normalize(&root.join(relative))))
    }

    /// The first module an aliased specifier resolves to, as seen from
    /// `importer`. Aliases whose scope does not contain the importer are
    /// skipped, so a monorepo package never borrows its neighbour's aliases.
    pub fn against_aliases(
        &self,
        specifier: &str,
        importer: &Path,
        try_at: impl Fn(&Path) -> Option<ModuleId>,
    ) -> Option<ModuleId> {
        self.aliases
            .iter()
            .filter(|alias| importer.starts_with(&alias.scope))
            .flat_map(|alias| alias.apply(specifier))
            .find_map(|candidate| try_at(&candidate))
    }

    /// Whether a declared alias in scope for `importer` claims `specifier`,
    /// whether or not the file it names exists. A catch-all pattern — a
    /// tsconfig `"*"` — claims nothing in particular and does not count.
    pub fn claims(&self, specifier: &str, importer: &Path) -> bool {
        self.aliases
            .iter()
            .filter(|alias| !alias.prefix.is_empty() && importer.starts_with(&alias.scope))
            .any(|alias| !alias.apply(specifier).is_empty())
    }

    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }
}

/// `x` + `ts` → `x.ts`, keeping any dots already in the file name.
pub fn append_extension(path: &Path, extension: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(extension);
    path.with_file_name(name)
}

/// Resolve `.` and `..` without touching the filesystem, so a specifier that
/// climbs out of the scan fails to match instead of erroring.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_answers_exact_paths_only() {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        index.insert(PathBuf::from("/p/src/a.ts"), ModuleId(3));
        assert_eq!(index.get(Path::new("/p/src/a.ts")), Some(ModuleId(3)));
        assert!(index.contains(Path::new("/p/src/a.ts")));
        assert_eq!(index.get(Path::new("/p/src/a")), None);
        assert_eq!(index.len(), 1);
        assert!(!index.is_empty());
    }

    #[test]
    fn against_roots_tries_each_root_in_order() {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/first"), PathBuf::from("/second")]);
        index.insert(PathBuf::from("/second/lib/x.ts"), ModuleId(1));
        let found = index.against_roots("lib/x.ts", |candidate| index.get(candidate));
        assert_eq!(found, Some(ModuleId(1)));
        assert_eq!(
            index.against_roots("lib/missing.ts", |candidate| index.get(candidate)),
            None
        );
    }

    fn alias(scope: &str, prefix: &str, wildcard: bool, targets: &[&str]) -> PathAlias {
        PathAlias {
            scope: PathBuf::from(scope),
            prefix: prefix.to_string(),
            wildcard,
            targets: targets.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn a_wildcard_alias_joins_the_matched_suffix_to_each_target() {
        let a = alias("/p", "@/", true, &["/p/src", "/p/lib"]);
        assert_eq!(
            a.apply("@/ui/button"),
            vec![
                PathBuf::from("/p/src/ui/button"),
                PathBuf::from("/p/lib/ui/button")
            ]
        );
        assert!(
            a.apply("@").is_empty(),
            "a wildcard needs something to match"
        );
        assert!(a.apply("other/x").is_empty());
    }

    #[test]
    fn an_exact_alias_matches_the_whole_specifier_only() {
        let a = alias("/p", "@config", false, &["/p/src/config.ts"]);
        assert_eq!(a.apply("@config"), vec![PathBuf::from("/p/src/config.ts")]);
        assert!(a.apply("@config/deep").is_empty());
    }

    #[test]
    fn aliases_are_tried_most_specific_first() {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        index.insert(PathBuf::from("/p/narrow/button.ts"), ModuleId(1));
        index.insert(PathBuf::from("/p/wide/ui/button.ts"), ModuleId(2));
        index.set_aliases(vec![
            alias("/p", "@/", true, &["/p/wide"]),
            alias("/p", "@/ui/", true, &["/p/narrow"]),
        ]);
        assert_eq!(index.aliases()[0].prefix, "@/ui/", "longer prefix first");
        let found = index.against_aliases("@/ui/button", Path::new("/p/a.ts"), |candidate| {
            index.get(&append_extension(candidate, "ts"))
        });
        assert_eq!(found, Some(ModuleId(1)));
    }

    #[test]
    fn an_alias_outside_the_importers_directory_is_skipped() {
        let mut index = ModuleIndex::new(vec![PathBuf::from("/p")]);
        index.insert(PathBuf::from("/p/one/src/x.ts"), ModuleId(1));
        index.set_aliases(vec![alias("/p/one", "@/", true, &["/p/one/src"])]);
        let resolve = |importer: &str| {
            index.against_aliases("@/x", Path::new(importer), |candidate| {
                index.get(&append_extension(candidate, "ts"))
            })
        };
        assert_eq!(resolve("/p/one/a.ts"), Some(ModuleId(1)));
        assert_eq!(resolve("/p/two/b.ts"), None);
    }

    #[test]
    fn normalize_collapses_dot_segments() {
        assert_eq!(
            normalize(Path::new("/p/src/./deep/../shared")),
            PathBuf::from("/p/src/shared")
        );
    }

    #[test]
    fn climbing_above_the_root_does_not_panic() {
        assert_eq!(
            normalize(Path::new("/p/../../x")),
            PathBuf::from("/../x"),
            "the excess `..` survives as a path nothing in the index can match"
        );
    }

    #[test]
    fn append_extension_keeps_dots_already_in_the_name() {
        assert_eq!(
            append_extension(Path::new("/p/a.spec"), "ts"),
            PathBuf::from("/p/a.spec.ts")
        );
    }
}
