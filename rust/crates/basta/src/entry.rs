//! Where the program starts.
//!
//! Every finding basta reports is an answer to "nothing reaches this", so the
//! whole result rests on knowing what *does* reach. Get the entry points
//! wrong and the tool reports a working application as dead code.
//!
//! Four sources, in decreasing order of authority:
//!
//! 1. **Manifests.** `package.json`'s `main`, `bin`, `exports` and friends,
//!    `pyproject.toml`'s script tables. Each language reads its own, through
//!    [`Analyzer::manifests`] and [`Analyzer::manifest_entries`].
//! 2. **Conventions.** `src/index.ts`, `__main__.py`, a config file the build
//!    tool loads by name, a file with a shebang. Each language lists its own,
//!    through [`Analyzer::entry_globs`], [`Analyzer::is_self_starting`] and
//!    [`ModuleTraits::entry_point`].
//! 3. **Scripts.** A shell script, a CI workflow, a Makefile or a Dockerfile
//!    that names a source file runs it, copies it or ships it. Those files
//!    are not analyzed — they are not source in any language basta reads —
//!    but what they mention is alive, and `publish-npm.sh` requiring
//!    `platform-map.js` is as real a use as any `import`.
//! 4. **The user.** `--entry` globs, which override everything: a project
//!    that does something unusual says so once instead of being argued with.
//!
//! Nothing in this file knows a language. It drives the analyzers and owns
//! the parts that are the same for all of them: the glob matching, the walk
//! for manifests and scripts, and the helpers a manifest reader needs.
//!
//! Anything uncertain is resolved towards *more* entry points. A file wrongly
//! treated as an entry point hides a real finding; a file wrongly treated as
//! unreachable produces a false one, and a tool that cries wolf gets turned
//! off.

use crate::lang::{ANALYZERS, is_source_path};
use crate::model::Module;
use crate::resolve::PathAlias;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

/// Directories whose contents are tests, fixtures, examples or benchmarks in
/// any language. File-name conventions (`*.test.ts`, `test_*.py`) are the
/// analyzers' to declare.
const TEST_DIRECTORIES: &[&str] = &[
    "**/test/**",
    "**/tests/**",
    "**/__tests__/**",
    "**/__mocks__/**",
    "**/spec/**",
    "**/e2e/**",
    "**/cypress/**",
    "**/playwright/**",
    "**/fixtures/**",
    "**/testdata/**",
    "**/example/**",
    "**/examples/**",
    "**/benchmark/**",
    "**/benchmarks/**",
    "**/bench/**",
];

/// Which modules of a scan are entry points and which are tests.
pub struct Entries {
    entry: FxHashSet<usize>,
    test: FxHashSet<usize>,
}

impl Entries {
    pub fn is_entry(&self, index: usize) -> bool {
        self.entry.contains(&index)
    }

    pub fn is_test(&self, index: usize) -> bool {
        self.test.contains(&index)
    }

    pub fn entry_count(&self) -> usize {
        self.entry.len()
    }
}

/// Classify every module of a scan.
///
/// `extra` are user globs from `--entry`; they add entry points and never
/// remove one, so a project can always widen what basta considers live.
pub fn detect(modules: &[Module], roots: &[PathBuf], extra: &[String]) -> Entries {
    let conventional = build_globs(
        ANALYZERS
            .iter()
            .flat_map(|analyzer| analyzer.entry_globs().iter().copied()),
    );
    let tests = build_globs(
        TEST_DIRECTORIES.iter().copied().chain(
            ANALYZERS
                .iter()
                .flat_map(|analyzer| analyzer.test_globs().iter().copied()),
        ),
    );
    let user = build_globs(extra.iter().map(String::as_str));
    let manifests = manifest_entry_paths(modules, roots);
    let mentioned = script_mentions(modules, roots);

    let mut entry = FxHashSet::default();
    let mut test = FxHashSet::default();
    for (index, module) in modules.iter().enumerate() {
        let path = module.path.replace('\\', "/");
        if tests.is_match(&path) {
            test.insert(index);
        }
        let is_entry = user.is_match(&path)
            || conventional.is_match(&path)
            || manifests.files.contains(&module.real_path)
            // A framework that loads a whole directory reaches every file in
            // it without any file naming one.
            || manifests
                .directories
                .iter()
                .any(|directory| module.real_path.starts_with(directory))
            || mentioned.contains(&index)
            // A file that declares it runs on its own — a shebang, a main
            // guard — is started by something outside the scan by definition.
            || module.is_entry
            // What the language says about the path itself.
            || module.traits.entry_point;
        if is_entry {
            entry.insert(index);
        }
    }
    Entries { entry, test }
}

fn build_globs<'a>(patterns: impl Iterator<Item = &'a str>) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
        // A pattern with no leading `**/` should still match at any depth, the
        // way jscpd's own ignore patterns do.
        if !pattern.starts_with("**/")
            && !pattern.starts_with('/')
            && let Ok(glob) = Glob::new(&format!("**/{pattern}"))
        {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Absolute paths named by every manifest that covers an analyzed file.
///
/// Manifests are looked for in the ancestor directories of the files actually
/// scanned, which finds every workspace package of a monorepo without
/// walking, and reads nothing in a repository whose code was not scanned.
/// Each manifest is handed to the analyzer that declared it.
fn manifest_entry_paths(modules: &[Module], roots: &[PathBuf]) -> ManifestEntries {
    let mut found = ManifestEntries::default();
    for directory in config_directories(modules, roots) {
        for analyzer in ANALYZERS {
            for manifest in analyzer.manifests() {
                let Ok(text) = std::fs::read_to_string(directory.join(manifest)) else {
                    continue;
                };
                found
                    .files
                    .extend(analyzer.manifest_entries(directory, manifest, &text));
                found
                    .directories
                    .extend(analyzer.manifest_entry_directories(directory, manifest, &text));
            }
        }
    }
    found
}

/// What a project's manifests name as entry points.
#[derive(Default)]
struct ManifestEntries {
    /// Files, matched exactly.
    files: FxHashSet<PathBuf>,
    /// Directories a framework loads whole, matched as a prefix. A project
    /// declares a handful at most, so testing every module against all of them
    /// costs nothing.
    directories: Vec<PathBuf>,
}

/// Every directory that could hold a config for a scanned file: the ancestors
/// of each analyzed file up to a scan root, plus the roots themselves.
///
/// Walking ancestors rather than the tree finds every workspace package of a
/// monorepo without listing directories, and reads nothing in a repository
/// whose code was not scanned.
pub(crate) fn config_directories<'a>(
    modules: &'a [Module],
    roots: &'a [PathBuf],
) -> FxHashSet<&'a Path> {
    let mut directories: FxHashSet<&Path> = FxHashSet::default();
    for module in modules {
        let mut directory = module.real_path.parent();
        while let Some(current) = directory {
            if !directories.insert(current) {
                break; // This chain was already walked.
            }
            if roots.iter().any(|root| root == current) {
                break;
            }
            directory = current.parent();
        }
    }
    for root in roots {
        directories.insert(root);
    }
    directories
}

/// Every path alias the scanned project declares for itself.
///
/// Read from the same directories as the manifests, and handed to the
/// analyzer that declared the config file, so a language that has no such
/// idea contributes nothing.
pub(crate) fn path_aliases(modules: &[Module], roots: &[PathBuf]) -> Vec<PathAlias> {
    let mut aliases = Vec::new();
    for directory in config_directories(modules, roots) {
        for analyzer in ANALYZERS {
            for config in analyzer.alias_configs() {
                let Ok(text) = std::fs::read_to_string(directory.join(config)) else {
                    continue;
                };
                aliases.extend(analyzer.path_aliases(directory, config, &text));
            }
        }
    }
    aliases
}

// ── helpers for manifest readers ────────────────────────────────────────────

/// Every string leaf of a JSON value. `bin` is either one path or a map of
/// command name to path, and `exports` nests paths under subpaths and
/// conditions to any depth; this flattens all of them.
pub fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Object(map) => {
            for nested in map.values() {
                collect_strings(nested, out);
            }
        }
        serde_json::Value::Array(items) => {
            for nested in items {
                collect_strings(nested, out);
            }
        }
        _ => {}
    }
}

/// True when a manifest entry names a source file in some language basta
/// analyzes, rather than a directory or a glob.
pub fn looks_like_source_file(entry: &str) -> bool {
    !entry.contains('*') && is_source_path(entry)
}

/// Tokens of a shell command that name a source file. A script is a command
/// line, not a path: `tsx scripts/build.ts --watch` names one file among its
/// arguments, and that file is started by the script and imported by nothing.
pub fn script_file_arguments(command: &str) -> Vec<String> {
    command
        .split(|c: char| c.is_whitespace() || c == '&' || c == '|' || c == ';')
        .filter(|token| !token.is_empty() && !token.starts_with('-'))
        .filter(|token| is_source_path(token))
        .map(str::to_string)
        .collect()
}

// ── scripts that name source files ──────────────────────────────────────────

/// Files that run, copy or ship other files without importing them.
///
/// Matched on the file name alone; the walk that finds them is separate from
/// the source walk because none of these is a format basta analyzes.
fn is_script_file(name: &str) -> bool {
    const NAMES: &[&str] = &[
        "Makefile",
        "makefile",
        "GNUmakefile",
        "Justfile",
        "justfile",
        "Procfile",
        "Dockerfile",
        "Containerfile",
    ];
    const EXTENSIONS: &[&str] = &[
        "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd", "yml", "yaml", "mk",
    ];
    NAMES.contains(&name)
        || name.starts_with("Dockerfile.")
        || name.ends_with(".Dockerfile")
        || name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| EXTENSIONS.contains(&extension))
}

/// Scripts larger than this are skipped: a mention buried in a megabyte of
/// generated YAML is not worth the read, and the cap keeps a stray dump from
/// dominating the walk.
const SCRIPT_SIZE_LIMIT: u64 = 1024 * 1024;

/// Indices of the modules some script in the tree mentions by path.
///
/// A mention is the module's scan-root-relative path, or its file name as a
/// path segment (`/platform-map.js`, which is how `./platform-map.js` and
/// `../lib/platform-map.js` both end). When two modules share a file name —
/// `util/scripts.ts` and `plugins/mise/scripts.ts` — the segment has to
/// include the parent directory too, or a script naming one would keep the
/// other alive.
///
/// The scripts are read once and split into path-shaped tokens, and each
/// token is looked up in a table built from the modules. That keeps the cost
/// proportional to the size of the scripts, not to scripts × modules: a
/// monorepo with ten thousand files and two hundred workflows is a few
/// megabytes of tokenizing, not billions of substring searches.
fn script_mentions(modules: &[Module], roots: &[PathBuf]) -> FxHashSet<usize> {
    let mut mentioned = FxHashSet::default();
    if modules.is_empty() {
        return mentioned;
    }

    let mut name_counts: FxHashMap<&str, usize> = FxHashMap::default();
    for module in modules {
        if let Some(name) = module.path.rsplit(['/', '\\']).next() {
            *name_counts.entry(name).or_default() += 1;
        }
    }
    // Full relative path → module, and distinguishing suffix → module. A
    // token matches when it *is* a relative path, or *ends with* a suffix.
    let mut by_path: FxHashMap<String, usize> = FxHashMap::default();
    let mut by_suffix: FxHashMap<String, usize> = FxHashMap::default();
    for (index, module) in modules.iter().enumerate() {
        let path = module.path.replace('\\', "/");
        let mut segments = path.rsplit('/');
        let Some(name) = segments.next() else {
            continue;
        };
        let suffix = if name_counts.get(name).copied().unwrap_or(0) > 1 {
            match segments.next() {
                Some(parent) => format!("/{parent}/{name}"),
                None => format!("/{name}"),
            }
        } else {
            format!("/{name}")
        };
        by_suffix.insert(suffix, index);
        by_path.insert(path, index);
    }

    for root in roots {
        let mut walk = ignore::WalkBuilder::new(root);
        walk.hidden(false).git_ignore(true).filter_entry(|entry| {
            // Hidden directories hold CI workflows (`.github/`), so they are
            // walked; the ones that are never a project's own scripts are not.
            let name = entry.file_name().to_string_lossy();
            !(entry.file_type().is_some_and(|t| t.is_dir())
                && matches!(
                    name.as_ref(),
                    ".git"
                        | "node_modules"
                        | "target"
                        | "dist"
                        | "build"
                        | ".venv"
                        | "venv"
                        | "__pycache__"
                ))
        });
        for entry in walk.build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file())
                || !is_script_file(&entry.file_name().to_string_lossy())
                || entry.metadata().is_ok_and(|m| m.len() > SCRIPT_SIZE_LIMIT)
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for token in path_tokens(&text) {
                if let Some(index) = by_path.get(token) {
                    mentioned.insert(*index);
                    continue;
                }
                // `./x.js` and `x.js` both end in `/x.js` once a `/` is
                // prepended; `foo/x.jsx` does not.
                let with_slash = format!("/{token}");
                for (suffix, index) in &by_suffix {
                    if with_slash.ends_with(suffix.as_str()) {
                        mentioned.insert(*index);
                    }
                }
            }
        }
    }
    mentioned
}

/// Maximal runs of path characters in a script, minus anything that cannot
/// name a source file.
fn path_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '@')))
        .filter(|token| token.contains('.') && is_source_path(token))
        .map(|token| token.trim_start_matches("./"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ModuleId, ModuleTraits};

    fn module(path: &str) -> Module {
        let analyzer = crate::lang::analyzer_for(if path.ends_with(".py") {
            "python"
        } else {
            "typescript"
        })
        .unwrap();
        Module {
            id: ModuleId(0),
            path: path.to_string(),
            real_path: PathBuf::from("/p").join(path),
            format: if path.ends_with(".py") {
                "python".into()
            } else {
                "typescript".into()
            },
            language: analyzer.language(),
            traits: analyzer.module_traits(path),
            lines: 10,
            is_entry: false,
            is_test: false,
            has_dynamic_access: false,
            parse_failed: false,
        }
    }

    fn classify(paths: &[&str]) -> (Vec<bool>, Vec<bool>) {
        let modules: Vec<Module> = paths.iter().map(|p| module(p)).collect();
        let entries = detect(&modules, &[PathBuf::from("/p")], &[]);
        (
            (0..paths.len()).map(|i| entries.is_entry(i)).collect(),
            (0..paths.len()).map(|i| entries.is_test(i)).collect(),
        )
    }

    #[test]
    fn analyzers_supply_the_conventions_and_the_driver_applies_them() {
        // One JavaScript convention, one Python convention, one of each kind
        // of non-entry: the driver does not know which is which.
        let paths = [
            "src/index.ts",
            "app/__main__.py",
            "src/internal/helper.ts",
            "pkg/internal.py",
        ];
        let (entry, _) = classify(&paths);
        assert_eq!(entry, vec![true, true, false, false]);
    }

    #[test]
    fn module_traits_can_make_a_file_an_entry_point() {
        let mut m = module("pkg/plain.ts");
        assert!(!detect(std::slice::from_ref(&m), &[PathBuf::from("/p")], &[]).is_entry(0));
        m.traits = ModuleTraits {
            entry_point: true,
            ..ModuleTraits::default()
        };
        assert!(detect(std::slice::from_ref(&m), &[PathBuf::from("/p")], &[]).is_entry(0));
    }

    #[test]
    fn shared_test_directories_and_language_test_globs_both_count() {
        let paths = [
            "src/a.test.ts",
            "tests/test_thing.py",
            "src/__tests__/b.ts",
            "e2e/flow.spec.ts",
            "src/a.ts",
            "examples/demo.py",
        ];
        let (_, test) = classify(&paths);
        assert_eq!(test, vec![true, true, true, true, false, true]);
    }

    #[test]
    fn user_globs_add_entry_points() {
        let modules = [module("src/handlers/webhook.ts"), module("src/a.ts")];
        let entries = detect(
            &modules,
            &[PathBuf::from("/p")],
            &["src/handlers/**".to_string()],
        );
        assert!(entries.is_entry(0));
        assert!(!entries.is_entry(1));
    }

    #[test]
    fn a_self_starting_file_is_an_entry_point() {
        let mut modules = [module("src/tool.ts")];
        modules[0].is_entry = true;
        let entries = detect(&modules, &[PathBuf::from("/p")], &[]);
        assert!(entries.is_entry(0));
    }

    #[test]
    fn script_arguments_that_name_source_files_are_found() {
        assert_eq!(
            script_file_arguments("tsx scripts/build.ts --watch && python tools/gen.py"),
            vec!["scripts/build.ts", "tools/gen.py"]
        );
        assert!(script_file_arguments("tsc --project tsconfig.json").is_empty());
        assert!(script_file_arguments("rimraf dist").is_empty());
    }

    #[test]
    fn source_files_are_told_from_directories_and_globs() {
        assert!(looks_like_source_file("run-cpd.js"));
        assert!(looks_like_source_file("./tools/gen.py"));
        assert!(!looks_like_source_file("dist"));
        assert!(!looks_like_source_file("src/**/*.js"));
        assert!(!looks_like_source_file("schema.json"));
    }

    #[test]
    fn a_script_that_names_a_file_keeps_it_alive() {
        let dir = std::env::temp_dir().join(format!("basta-scripts-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::create_dir_all(dir.join(".github/workflows")).unwrap();
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("platform-map.js"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("lib/helper.js"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("lib/orphan.js"), "module.exports = {};\n").unwrap();
        std::fs::write(
            dir.join("scripts/publish.sh"),
            "node -e \"require('./platform-map.js')\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(".github/workflows/ci.yml"),
            "run: node lib/helper.js\n",
        )
        .unwrap();

        let modules: Vec<Module> = ["platform-map.js", "lib/helper.js", "lib/orphan.js"]
            .iter()
            .map(|p| Module {
                real_path: dir.join(p),
                ..module(p)
            })
            .collect();
        let entries = detect(&modules, std::slice::from_ref(&dir), &[]);
        assert!(entries.is_entry(0), "a shell script requires it");
        assert!(
            entries.is_entry(1),
            "a CI workflow under a hidden directory runs it"
        );
        assert!(!entries.is_entry(2), "nothing mentions it");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_shared_file_name_needs_its_directory_to_count_as_a_mention() {
        let dir = std::env::temp_dir().join(format!("basta-scripts-shared-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("util")).unwrap();
        std::fs::create_dir_all(dir.join("plugins/mise")).unwrap();
        std::fs::write(dir.join("util/scripts.ts"), "export {};\n").unwrap();
        std::fs::write(dir.join("plugins/mise/scripts.ts"), "export {};\n").unwrap();
        std::fs::write(dir.join("run.sh"), "tsx util/scripts.ts\n").unwrap();

        let modules: Vec<Module> = ["util/scripts.ts", "plugins/mise/scripts.ts"]
            .iter()
            .map(|p| Module {
                real_path: dir.join(p),
                ..module(p)
            })
            .collect();
        let entries = detect(&modules, std::slice::from_ref(&dir), &[]);
        assert!(entries.is_entry(0));
        assert!(
            !entries.is_entry(1),
            "a script naming util/scripts.ts says nothing about plugins/mise/scripts.ts"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_longer_file_name_that_merely_contains_the_module_name_is_not_a_mention() {
        let tokens: Vec<&str> = path_tokens("node lib/platform-map.jsx && echo x.py").collect();
        assert_eq!(tokens, vec!["lib/platform-map.jsx", "x.py"]);
        // `/platform-map.js` is not a suffix of `/lib/platform-map.jsx`.
        assert!(!format!("/{}", tokens[0]).ends_with("/platform-map.js"));
    }

    #[test]
    fn path_tokens_strip_the_leading_dot_slash_and_ignore_prose() {
        let tokens: Vec<&str> =
            path_tokens("run ./scripts/build.ts, then 'src/x.py' (see docs)").collect();
        assert_eq!(tokens, vec!["scripts/build.ts", "src/x.py"]);
    }

    #[test]
    fn script_files_are_recognised_by_name() {
        for yes in [
            "deploy.sh",
            "ci.yml",
            "Makefile",
            "Dockerfile",
            "Dockerfile.dev",
            "build.ps1",
        ] {
            assert!(is_script_file(yes), "{yes}");
        }
        for no in ["index.ts", "README.md", "package.json", "notes.txt"] {
            assert!(!is_script_file(no), "{no}");
        }
    }

    #[test]
    fn manifests_are_read_from_the_directories_that_hold_scanned_files() {
        let dir = std::env::temp_dir().join(format!("basta-entry-{}", std::process::id()));
        let package = dir.join("packages/ui");
        std::fs::create_dir_all(package.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"main": "./src/entry.ts"}"#,
        )
        .unwrap();

        let modules = [Module {
            real_path: package.join("src/entry.ts"),
            path: "packages/ui/src/entry.ts".into(),
            ..module("packages/ui/src/entry.ts")
        }];
        let entries = detect(&modules, std::slice::from_ref(&dir), &[]);
        assert!(
            entries.is_entry(0),
            "a workspace package's own manifest names its entry"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_manifest_can_root_a_whole_directory() {
        let dir = std::env::temp_dir().join(format!("basta-framework-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("components")).unwrap();
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("nuxt.config.ts"), "export default {}").unwrap();

        let modules = [
            Module {
                real_path: dir.join("components/Card.vue"),
                ..module("components/Card.vue")
            },
            Module {
                real_path: dir.join("lib/helper.ts"),
                ..module("lib/helper.ts")
            },
        ];
        let entries = detect(&modules, std::slice::from_ref(&dir), &[]);
        assert!(
            entries.is_entry(0),
            "Nuxt renders a component here with no file importing it"
        );
        assert!(
            !entries.is_entry(1),
            "a directory the framework does not load is still judged by the graph"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
