//! Frameworks: the programs that start a project's files without importing them.
//!
//! A framework reads a directory and turns what it finds into routes,
//! handlers, plugins and pages. No file records that, so every one of those
//! files looks unreachable until basta knows which framework is at work and
//! what it loads.
//!
//! Both halves of that are data, not code. `frameworks.yaml`, compiled into
//! the binary, lists each framework with the signals that give it away — a
//! config file by name, a dependency or a section of the project manifest —
//! and the files and directories it starts. A project extends or overrides
//! the list with a file of the same shape, in YAML or JSON, so a framework
//! basta has never heard of is a dozen lines of config rather than a release.
//!
//! Nothing here knows a language. What a manifest depends on and what a
//! config file written in source code assigns to a key are both the
//! analyzers' to say, through [`Analyzer::manifest_signals`] and
//! [`Analyzer::config_setting`].
//!
//! [`Analyzer::manifest_signals`]: crate::lang::Analyzer::manifest_signals
//! [`Analyzer::config_setting`]: crate::lang::Analyzer::config_setting

use crate::lang::ANALYZERS;
use crate::resolve::normalize;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use rustc_hash::FxHashSet;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The built-in table. See the header of the file for the schema.
const BUILTIN: &str = include_str!("../frameworks.yaml");

/// File names a project's own definitions are looked for under, in the
/// working directory, when `--frameworks-config` does not name one.
pub const PROJECT_FILES: &[&str] = &[
    "basta.frameworks.yaml",
    "basta.frameworks.yml",
    "basta.frameworks.json",
];

/// One framework: how to tell it is in use, and what it starts.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Framework {
    pub name: String,
    #[serde(default)]
    pub detect: Detect,
    /// Config keys that move directories around, with the value to assume
    /// when the config does not set one. Written `${key}` in a path.
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Directories `entry` and `directories` are relative to, themselves
    /// relative to the directory the framework was detected in.
    #[serde(default = "project_directory")]
    pub bases: Vec<String>,
    /// Globs of files the framework loads by name.
    #[serde(default)]
    pub entry: Vec<String>,
    /// Directories the framework loads whole.
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub auto_imports: Option<AutoImports>,
    /// Names the framework reads by convention. A declaration under one of
    /// them is used by the framework, whatever the import graph says.
    #[serde(default)]
    pub globals: Vec<Globals>,
}

/// Names a framework looks up in the project's code: an export it calls
/// (`getServerSideProps`), a lifecycle method it invokes (`ngOnInit`).
///
/// A bare name holds anywhere in the project. The longer form ties names to
/// the files the framework actually reads them from, because `loader` is
/// Remix's word in a route and anybody's word everywhere else.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Globals {
    Name(String),
    Scoped(ScopedGlobals),
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopedGlobals {
    pub names: Vec<String>,
    /// Globs, relative to each base, of the files these names mean something
    /// in. Empty means every file of the project.
    #[serde(default)]
    pub files: Vec<String>,
}

impl Globals {
    fn names(&self) -> &[String] {
        match self {
            Self::Name(name) => std::slice::from_ref(name),
            Self::Scoped(scoped) => &scoped.names,
        }
    }

    fn files(&self) -> &[String] {
        match self {
            Self::Name(_) => &[],
            Self::Scoped(scoped) => &scoped.files,
        }
    }
}

fn project_directory() -> Vec<String> {
    vec![".".to_string()]
}

/// The signals that give a framework away. Any one is enough.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Detect {
    /// Config file names; `{a,b}` alternatives are expanded, and a name may
    /// reach into a subdirectory (`.storybook/main.ts`).
    #[serde(default)]
    pub config_files: Vec<String>,
    /// Packages the manifest depends on. A trailing `*` matches a prefix.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Top-level sections of the manifest (`"jest": {…}` in `package.json`).
    #[serde(default)]
    pub package_json_keys: Vec<String>,
}

/// Directories loaded whole until one config key turns that off.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoImports {
    /// The key whose `false` disables these (`imports: false`).
    #[serde(default)]
    pub disabled_by: Option<String>,
    pub directories: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameworkFile {
    frameworks: Vec<Framework>,
}

/// What a project manifest says about the toolchain, as far as detection
/// cares: the packages it depends on and the sections it carries.
#[derive(Debug, Default)]
pub struct ManifestSignals {
    pub dependencies: FxHashSet<String>,
    pub sections: FxHashSet<String>,
}

impl ManifestSignals {
    pub fn merge(&mut self, other: ManifestSignals) {
        self.dependencies.extend(other.dependencies);
        self.sections.extend(other.sections);
    }
}

/// What a config file assigns to a key, when it is something static analysis
/// can read.
#[derive(Debug, PartialEq)]
pub enum Setting {
    /// A string literal: `srcDir: "src"`.
    Text(String),
    /// The literal `false`: `imports: false`.
    Off,
}

/// Every framework a run knows about, and how detection should behave.
#[derive(Debug, Clone)]
pub struct Registry {
    frameworks: Vec<Known>,
    /// Frameworks taken as present at every scan root, whatever the signals
    /// say — for a scan that starts below the manifest that would name them.
    forced: Vec<String>,
    enabled: bool,
}

/// A definition with the part of it that is worth computing once.
#[derive(Debug, Clone)]
struct Known {
    definition: Framework,
    /// `detect.config_files`, alternatives expanded.
    config_names: Vec<String>,
}

impl Default for Registry {
    /// The built-in frameworks, detected automatically.
    fn default() -> Self {
        static PARSED: OnceLock<Vec<Framework>> = OnceLock::new();
        let builtin = PARSED.get_or_init(|| {
            // Guarded by `the_built_in_table_is_valid`: a table that does not
            // parse never leaves the test suite.
            parse(BUILTIN, Format::Yaml).expect("frameworks.yaml is valid")
        });
        let mut registry = Self {
            frameworks: Vec::new(),
            forced: Vec::new(),
            enabled: true,
        };
        registry.extend(builtin.iter().cloned());
        registry
    }
}

/// Everything a run was told about frameworks, from whichever of the command
/// line and the config file said it.
#[derive(Debug, Default)]
pub struct Sources<'a> {
    /// A definitions file named outright. Without one, the working directory
    /// is looked in for [`PROJECT_FILES`].
    pub file: Option<PathBuf>,
    /// Definitions written inline, in the dead-code section of a jscpd
    /// config. They go on last, so they win over a file.
    pub inline: &'a [Framework],
    /// Frameworks to take as present at the scan roots.
    pub forced: &'a [String],
    pub disabled: bool,
}

impl Registry {
    /// The registry those sources describe, and everything wrong with them.
    ///
    /// Both front ends build theirs here, so a definitions file, an inline
    /// definition and a forced name mean the same thing to `basta` and to
    /// `jscpd --dead-code`. A problem is an error for the caller to refuse
    /// the run over: going on without a definition would report as dead
    /// exactly the files it was written to keep alive.
    pub fn assemble(sources: Sources<'_>) -> (Self, Vec<String>) {
        let mut registry = Self::default();
        let mut problems = Vec::new();
        let file = sources.file.or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|directory| project_file(&directory))
        });
        if let Some(file) = file {
            match load(&file) {
                Ok(definitions) => registry.extend(definitions),
                Err(error) => problems.push(error),
            }
        }
        match sources.inline.iter().try_for_each(Framework::validate) {
            Ok(()) => registry.extend(sources.inline.iter().cloned()),
            Err(error) => problems.push(error),
        }
        for name in sources.forced {
            if !registry.knows(name) {
                problems.push(format!(
                    "'{name}' is not a framework basta knows (basta --list-frameworks prints them)"
                ));
            }
        }
        registry.force(sources.forced.iter().cloned());
        if sources.disabled {
            registry.disable();
        }
        (registry, problems)
    }

    /// Add definitions. One that carries the name of a framework already
    /// known replaces it, which is how a project corrects a built-in.
    pub fn extend(&mut self, definitions: impl IntoIterator<Item = Framework>) {
        for definition in definitions {
            let known = Known {
                config_names: definition
                    .detect
                    .config_files
                    .iter()
                    .flat_map(|pattern| expand_braces(pattern))
                    .collect(),
                definition,
            };
            match self
                .frameworks
                .iter_mut()
                .find(|existing| existing.definition.name == known.definition.name)
            {
                Some(existing) => *existing = known,
                None => self.frameworks.push(known),
            }
        }
    }

    /// Take these frameworks as present at every scan root.
    pub fn force(&mut self, names: impl IntoIterator<Item = String>) {
        self.forced.extend(names);
    }

    /// Turn detection off: no framework contributes an entry point.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn forced(&self) -> &[String] {
        &self.forced
    }

    pub fn knows(&self, name: &str) -> bool {
        self.frameworks.iter().any(|f| f.definition.name == name)
    }

    /// Every known framework, in definition order.
    pub fn definitions(&self) -> impl Iterator<Item = &Framework> {
        self.frameworks.iter().map(|known| &known.definition)
    }

    /// The frameworks at work in `directory`.
    ///
    /// `signals` is what the manifests in that directory said; `is_root`
    /// marks a scan root, where forced frameworks apply.
    pub(crate) fn detect(
        &self,
        directory: &Path,
        signals: &ManifestSignals,
        is_root: bool,
    ) -> Vec<Rooted> {
        if !self.enabled {
            return Vec::new();
        }
        // One listing per directory instead of one `stat` per config name:
        // the table names a few hundred files and nearly none of them exist.
        let present: FxHashSet<String> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();

        self.frameworks
            .iter()
            .filter_map(|known| {
                let definition = &known.definition;
                let config = known
                    .config_names
                    .iter()
                    .find(|name| match name.contains('/') {
                        true => directory.join(name.as_str()).is_file(),
                        false => present.contains(name.as_str()),
                    });
                let detected = config.is_some()
                    || definition
                        .detect
                        .dependencies
                        .iter()
                        .any(|wanted| depends_on(&signals.dependencies, wanted))
                    || definition
                        .detect
                        .package_json_keys
                        .iter()
                        .any(|key| signals.sections.contains(key))
                    || (is_root && self.forced.contains(&definition.name));
                if !detected {
                    return None;
                }
                let text = config.and_then(|name| {
                    let text = std::fs::read_to_string(directory.join(name)).ok()?;
                    Some((name.as_str(), text))
                });
                Some(definition.root(directory, text.as_ref().map(|(n, t)| (*n, t.as_str()))))
            })
            .collect()
    }
}

/// `wanted` among `dependencies`, where a trailing `*` matches a prefix so a
/// family of packages (`@storybook/*`) needs one line.
fn depends_on(dependencies: &FxHashSet<String>, wanted: &str) -> bool {
    match wanted.strip_suffix('*') {
        Some(prefix) => dependencies.iter().any(|name| name.starts_with(prefix)),
        None => dependencies.contains(wanted),
    }
}

impl Framework {
    /// This framework as detected in `directory`: variables read from its
    /// config, paths made absolute.
    ///
    /// `config` is the matched config file's name and text, absent when the
    /// framework was detected some other way — then every variable keeps its
    /// default, which is what the framework itself would do.
    fn root(&self, directory: &Path, config: Option<(&str, &str)>) -> Rooted {
        let setting = |key: &str| config.and_then(|(name, text)| config_setting(name, text, key));
        let variables: Vec<(String, String)> = self
            .variables
            .iter()
            .map(|(key, default)| {
                let value = match setting(key) {
                    Some(Setting::Text(value)) => relative_directory(&value),
                    _ => None,
                };
                (
                    format!("${{{key}}}"),
                    value.unwrap_or_else(|| default.clone()),
                )
            })
            .collect();
        let substitute = |template: &str, escape: bool| {
            variables
                .iter()
                .fold(template.to_string(), |text, (name, value)| match escape {
                    true => text.replace(name, &globset::escape(value)),
                    false => text.replace(name, value),
                })
        };

        let mut bases: Vec<PathBuf> = Vec::new();
        for base in &self.bases {
            let base = normalize(&directory.join(substitute(base, false)));
            if !bases.contains(&base) {
                bases.push(base);
            }
        }

        let auto_imported = self.auto_imports.iter().flat_map(|auto| {
            let off = auto
                .disabled_by
                .as_deref()
                .is_some_and(|key| setting(key) == Some(Setting::Off));
            auto.directories.iter().filter(move |_| !off)
        });
        let mut directories: Vec<PathBuf> = Vec::new();
        for name in self.directories.iter().chain(auto_imported) {
            let name = substitute(name, false);
            for base in &bases {
                let path = normalize(&base.join(&name));
                // A directory that resolves to its own base (`"."`) would
                // root the whole project.
                if path != *base && !directories.contains(&path) {
                    directories.push(path);
                }
            }
        }

        let mut entry = GlobSetBuilder::new();
        for pattern in &self.entry {
            let pattern = substitute(pattern, true);
            if let Ok(glob) = entry_glob(pattern.trim_start_matches("./")) {
                entry.add(glob);
            }
        }

        let globals = self
            .globals
            .iter()
            .map(|globals| {
                let mut files = GlobSetBuilder::new();
                for pattern in globals.files() {
                    let pattern = substitute(pattern, true);
                    if let Ok(glob) = entry_glob(pattern.trim_start_matches("./")) {
                        files.add(glob);
                    }
                }
                GlobalScope {
                    names: globals.names().iter().cloned().collect(),
                    files: (!globals.files().is_empty())
                        .then(|| files.build().unwrap_or_else(|_| GlobSet::empty())),
                }
            })
            .collect();
        let global_names = self
            .globals
            .iter()
            .flat_map(|globals| globals.names().iter().cloned())
            .collect();

        Rooted {
            name: self.name.clone(),
            directory: directory.to_path_buf(),
            bases,
            entry: entry.build().unwrap_or_else(|_| GlobSet::empty()),
            directories,
            globals,
            global_names,
        }
    }

    /// What is wrong with this definition, if anything. Checked when a file
    /// is loaded, so a typo is an error message and not a silently inert
    /// framework — which would report a working application as dead.
    fn validate(&self) -> Result<(), String> {
        let name = &self.name;
        if name.trim().is_empty() {
            return Err("a framework needs a name".to_string());
        }
        let paths = self
            .bases
            .iter()
            .chain(&self.directories)
            .chain(self.auto_imports.iter().flat_map(|auto| &auto.directories))
            .chain(&self.entry)
            .chain(self.globals.iter().flat_map(Globals::files))
            .chain(self.variables.values());
        for path in paths {
            if path.starts_with('/') || path.split('/').any(|segment| segment == "..") {
                return Err(format!(
                    "{name}: '{path}' must stay inside the project directory"
                ));
            }
            let mut rest = path.as_str();
            while let Some(at) = rest.find("${") {
                let variable = rest[at + 2..].split('}').next().unwrap_or_default();
                if !self.variables.contains_key(variable) {
                    return Err(format!(
                        "{name}: '{path}' uses ${{{variable}}}, which `variables` does not declare"
                    ));
                }
                rest = &rest[at + 2..];
            }
        }
        for globals in &self.globals {
            if globals
                .names()
                .iter()
                .any(|global| global.trim().is_empty())
            {
                return Err(format!("{name}: a global needs a name"));
            }
        }
        let globs = self
            .entry
            .iter()
            .chain(self.globals.iter().flat_map(Globals::files));
        for pattern in globs {
            // A variable is a path, so any path stands in for it here.
            let probe = self.variables.keys().fold(pattern.clone(), |text, key| {
                text.replace(&format!("${{{key}}}"), "x")
            });
            entry_glob(&probe).map_err(|error| format!("{name}: glob '{pattern}': {error}"))?;
        }
        Ok(())
    }
}

/// A glob over a base-relative path. `*` stays inside one segment, the way a
/// router reads it: `src/preload/*.ts` is not `src/preload/lib/util.ts`.
fn entry_glob(pattern: &str) -> Result<globset::Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}

/// A framework detected in one directory, ready to be asked about files.
#[derive(Debug)]
pub(crate) struct Rooted {
    pub name: String,
    /// Where the framework was detected.
    pub directory: PathBuf,
    bases: Vec<PathBuf>,
    entry: GlobSet,
    directories: Vec<PathBuf>,
    globals: Vec<GlobalScope>,
    /// Every name in `globals`, whatever its scope.
    global_names: FxHashSet<String>,
}

/// Names the framework reads, and the files it reads them from.
#[derive(Debug)]
struct GlobalScope {
    names: FxHashSet<String>,
    /// `None` when the names hold in every file of the project.
    files: Option<GlobSet>,
}

impl Rooted {
    /// Whether the framework starts `path` on its own.
    pub fn reaches(&self, path: &Path) -> bool {
        self.directories
            .iter()
            .any(|directory| path.starts_with(directory))
            || self.bases.iter().any(|base| {
                path.strip_prefix(base).is_ok_and(|relative| {
                    self.entry
                        .is_match(relative.to_string_lossy().replace('\\', "/"))
                })
            })
    }
}

impl Rooted {
    /// Whether the framework reads the name `name` out of the file `path`.
    ///
    /// Only inside the project the framework was detected in: a monorepo's
    /// Remix app says nothing about what `loader` means in the package next
    /// to it.
    pub fn reads(&self, path: &Path, name: &str) -> bool {
        // The name first: it is asked of every declaration in the project,
        // nearly all of which no framework has a word for, and a hash lookup
        // is nothing beside walking a path.
        if !self.global_names.contains(name) || !path.starts_with(&self.directory) {
            return false;
        }
        self.globals
            .iter()
            .filter(|scope| scope.names.contains(name))
            .any(|scope| match &scope.files {
                None => true,
                Some(files) => self.bases.iter().any(|base| {
                    path.strip_prefix(base).is_ok_and(|relative| {
                        files.is_match(relative.to_string_lossy().replace('\\', "/"))
                    })
                }),
            })
    }

    pub fn has_globals(&self) -> bool {
        !self.globals.is_empty()
    }
}

/// A framework found at work somewhere in the scan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DetectedFramework {
    pub name: String,
    /// The directory whose config or manifest gave it away.
    pub directory: PathBuf,
}

/// What `text`, the config file `name`, assigns to `key`.
///
/// JSON is nobody's language and is read here; a config written as source
/// code belongs to whichever analyzer serves that language.
fn config_setting(name: &str, text: &str, key: &str) -> Option<Setting> {
    if name.ends_with(".json") {
        let json: serde_json::Value = serde_json::from_str(text).ok()?;
        return match json.get(key)? {
            serde_json::Value::String(value) => Some(Setting::Text(value.clone())),
            serde_json::Value::Bool(false) => Some(Setting::Off),
            _ => None,
        };
    }
    ANALYZERS
        .iter()
        .find_map(|analyzer| analyzer.config_setting(name, text, key))
}

/// A directory a config names, as a project-relative path — or nothing when
/// it points outside the project, which no definition is allowed to follow.
fn relative_directory(value: &str) -> Option<String> {
    let value = value.trim();
    let inside = !value.starts_with(['/', '\\'])
        && !value.contains(':')
        && value.split(['/', '\\']).all(|segment| segment != "..");
    match value.trim_start_matches("./").trim_end_matches('/') {
        _ if !inside => None,
        "" | "." => Some(".".to_string()),
        value => Some(value.to_string()),
    }
}

/// Every name a `{a,b}` pattern stands for. Groups may repeat and nest;
/// a pattern without one is returned as it is.
fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let mut depth = 0usize;
    let mut close = None;
    let mut commas = Vec::new();
    for (at, byte) in pattern.bytes().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at);
                    break;
                }
            }
            b',' if depth == 1 => commas.push(at),
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![pattern.to_string()]; // Unbalanced: a literal name.
    };
    let (head, tail) = (&pattern[..open], &pattern[close + 1..]);
    let mut starts = vec![open + 1];
    starts.extend(commas.iter().map(|comma| comma + 1));
    let ends = commas.iter().copied().chain(std::iter::once(close));
    starts
        .into_iter()
        .zip(ends)
        .flat_map(|(start, end)| expand_braces(&format!("{head}{}{tail}", &pattern[start..end])))
        .collect()
}

// ── loading ─────────────────────────────────────────────────────────────────

/// The two syntaxes a definitions file may be written in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Yaml,
    Json,
}

impl Format {
    /// By extension; anything that is not `.json` is read as YAML, which is
    /// also how a file with no extension at all gets a useful error.
    pub fn of(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("json") => Self::Json,
            _ => Self::Yaml,
        }
    }
}

/// Definitions from the text of a definitions file.
pub fn parse(text: &str, format: Format) -> Result<Vec<Framework>, String> {
    let file: FrameworkFile = match format {
        Format::Yaml => serde_yaml_ng::from_str(text).map_err(|error| error.to_string())?,
        Format::Json => serde_json::from_str(text).map_err(|error| error.to_string())?,
    };
    for framework in &file.frameworks {
        framework.validate()?;
    }
    Ok(file.frameworks)
}

/// Definitions from a file on disk, with the path in any error.
pub fn load(path: &Path) -> Result<Vec<Framework>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse(&text, Format::of(path)).map_err(|error| format!("{}: {error}", path.display()))
}

/// The project's own definitions file in `directory`, if it keeps one.
pub fn project_file(directory: &Path) -> Option<PathBuf> {
    PROJECT_FILES
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scan::TempTree;

    fn builtin(name: &str) -> Framework {
        Registry::default()
            .definitions()
            .find(|framework| framework.name == name)
            .unwrap_or_else(|| panic!("no built-in framework called {name}"))
            .clone()
    }

    fn signals(dependencies: &[&str], sections: &[&str]) -> ManifestSignals {
        ManifestSignals {
            dependencies: dependencies.iter().map(|d| d.to_string()).collect(),
            sections: sections.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn names(detected: &[Rooted]) -> Vec<&str> {
        detected.iter().map(|rooted| rooted.name.as_str()).collect()
    }

    #[test]
    fn the_built_in_table_is_valid() {
        let frameworks = parse(BUILTIN, Format::Yaml).expect("frameworks.yaml parses");
        assert!(frameworks.len() >= 40, "{} frameworks", frameworks.len());
        let mut seen = FxHashSet::default();
        for framework in &frameworks {
            assert!(
                seen.insert(framework.name.as_str()),
                "{} is defined twice",
                framework.name
            );
            let detect = &framework.detect;
            assert!(
                !(detect.config_files.is_empty()
                    && detect.dependencies.is_empty()
                    && detect.package_json_keys.is_empty()),
                "{} can never be detected",
                framework.name
            );
            assert!(
                !(framework.entry.is_empty()
                    && framework.directories.is_empty()
                    && framework.auto_imports.is_none()),
                "{} starts nothing",
                framework.name
            );
        }
        // The three that were code before they were data.
        for name in ["vite", "nitro", "wxt", "nuxt", "next"] {
            assert!(seen.contains(name), "{name}");
        }
    }

    #[test]
    fn no_config_file_name_gives_away_two_frameworks() {
        // `app.config.ts` is Nuxt's and Expo's both; a name like that detects
        // neither, or a Nuxt site is told it is a mobile app.
        let mut owner: rustc_hash::FxHashMap<String, String> = Default::default();
        for known in &Registry::default().frameworks {
            for name in &known.config_names {
                if let Some(other) = owner.insert(name.clone(), known.definition.name.clone()) {
                    panic!("{name} detects both {other} and {}", known.definition.name);
                }
            }
        }
        assert!(!owner.contains_key("app.config.ts"));
    }

    #[test]
    fn a_config_file_a_dependency_or_a_manifest_section_each_detect_a_framework() {
        let registry = Registry::default();

        let by_config = TempTree::new("framework-by-config");
        by_config.write("next.config.mjs", "export default {}\n");
        assert_eq!(
            names(&registry.detect(by_config.path(), &signals(&[], &[]), false)),
            vec!["next"]
        );

        let bare = TempTree::new("framework-by-manifest");
        bare.write("index.js", "");
        assert_eq!(
            names(&registry.detect(bare.path(), &signals(&["astro"], &[]), false)),
            vec!["astro"]
        );
        assert_eq!(
            names(&registry.detect(bare.path(), &signals(&[], &["jest"]), false)),
            vec!["jest"]
        );
        assert!(
            registry
                .detect(bare.path(), &signals(&["left-pad"], &["scripts"]), false)
                .is_empty()
        );
    }

    #[test]
    fn a_dependency_wildcard_matches_a_family_of_packages() {
        let tree = TempTree::new("framework-wildcard");
        tree.write("index.js", "");
        let detected = Registry::default().detect(
            tree.path(),
            &signals(&["@storybook/react-vite"], &[]),
            false,
        );
        assert_eq!(names(&detected), vec!["storybook"]);
    }

    #[test]
    fn a_config_in_a_subdirectory_is_found() {
        let tree = TempTree::new("framework-nested-config");
        tree.write(".storybook/main.ts", "export default {}\n");
        let detected = Registry::default().detect(tree.path(), &signals(&[], &[]), false);
        assert_eq!(names(&detected), vec!["storybook"]);
    }

    #[test]
    fn entry_globs_are_anchored_at_the_directory_the_framework_was_found_in() {
        let next = builtin("next").root(Path::new("/p/apps/web"), None);
        assert!(next.reaches(Path::new("/p/apps/web/app/blog/[slug]/page.tsx")));
        assert!(next.reaches(Path::new("/p/apps/web/src/pages/index.tsx")));
        assert!(next.reaches(Path::new("/p/apps/web/middleware.ts")));
        assert!(
            !next.reaches(Path::new("/p/apps/web/app/blog/helpers.ts")),
            "only the file names the router knows"
        );
        assert!(
            !next.reaches(Path::new("/p/apps/api/app/page.tsx")),
            "another package is another project"
        );
    }

    #[test]
    fn a_star_stays_inside_one_path_segment() {
        let electron = builtin("electron-vite").root(Path::new("/p"), None);
        assert!(electron.reaches(Path::new("/p/src/preload/webview.ts")));
        assert!(!electron.reaches(Path::new("/p/src/preload/lib/util.ts")));
    }

    #[test]
    fn nuxt_roots_its_auto_imported_directories_in_both_layouts() {
        let nuxt = builtin("nuxt").root(
            Path::new("/p"),
            Some(("nuxt.config.ts", "export default defineNuxtConfig({})")),
        );
        assert!(nuxt.reaches(Path::new("/p/components/Card.vue")));
        assert!(nuxt.reaches(Path::new("/p/composables/useThing.ts")));
        // Nuxt 4 puts the same tree under `app/`.
        assert!(nuxt.reaches(Path::new("/p/app/components/Card.vue")));
        assert!(!nuxt.reaches(Path::new("/p/lib/helper.ts")));
    }

    #[test]
    fn a_nuxt_config_that_moves_the_source_tree_is_followed() {
        let nuxt = builtin("nuxt").root(
            Path::new("/p"),
            Some((
                "nuxt.config.ts",
                "export default defineNuxtConfig({ srcDir: 'src/' })",
            )),
        );
        assert!(nuxt.reaches(Path::new("/p/src/components/Card.vue")));
        // The defaults stay in the list: a config may set other things.
        assert!(nuxt.reaches(Path::new("/p/components/Card.vue")));
    }

    #[test]
    fn wxt_entrypoints_are_rooted_wherever_the_config_puts_them() {
        let wxt = builtin("wxt");
        let default_layout = wxt.root(
            Path::new("/p"),
            Some(("wxt.config.ts", "export default defineConfig({})")),
        );
        assert!(default_layout.reaches(Path::new("/p/entrypoints/background.ts")));
        assert!(default_layout.reaches(Path::new("/p/utils/storage.ts")));

        // A project that moved the tree says so in `srcDir`, and can rename
        // the entrypoints directory too.
        let moved = wxt.root(
            Path::new("/p"),
            Some((
                "wxt.config.ts",
                "export default defineConfig({ srcDir: 'src', entrypointsDir: 'entries' })",
            )),
        );
        assert!(moved.reaches(Path::new("/p/src/entries/popup/main.ts")));
        assert!(moved.reaches(Path::new("/p/src/components/Button.tsx")));
        assert!(!moved.reaches(Path::new("/p/src/entrypoints/background.ts")));
        assert!(!moved.reaches(Path::new("/p/utils/storage.ts")));

        // A key named in a comment is not a key.
        let commented = wxt.root(
            Path::new("/p"),
            Some((
                "wxt.config.ts",
                "// srcDir: 'nowhere'\nexport default defineConfig({})",
            )),
        );
        assert!(commented.reaches(Path::new("/p/entrypoints/background.ts")));
        assert!(!commented.reaches(Path::new("/p/nowhere/entrypoints/background.ts")));
    }

    #[test]
    fn imports_false_turns_the_auto_import_directories_off() {
        let wxt = builtin("wxt");
        let off = wxt.root(
            Path::new("/p"),
            Some((
                "wxt.config.ts",
                "export default defineConfig({ srcDir: 'src', imports: false })",
            )),
        );
        // The entrypoints directory is the framework's contract either way…
        assert!(off.reaches(Path::new("/p/src/entrypoints/background.ts")));
        // …but nothing is auto-imported any more.
        assert!(!off.reaches(Path::new("/p/src/utils/storage.ts")));

        // `imports: { … }` customizes auto-imports without disabling them,
        // and a `false` of some longer word is not the keyword.
        for kept in [
            "defineConfig({ imports: { eslintrc: { enabled: true } } })",
            "defineConfig({ imports: falsework })",
        ] {
            let rooted = wxt.root(Path::new("/p"), Some(("wxt.config.ts", kept)));
            assert!(rooted.reaches(Path::new("/p/utils/storage.ts")), "{kept}");
        }
    }

    #[test]
    fn a_scoped_global_holds_only_in_the_files_the_framework_reads_it_from() {
        let remix = builtin("remix").root(
            Path::new("/p/apps/shop"),
            Some((
                "remix.config.js",
                "module.exports = { appDirectory: 'source' }",
            )),
        );
        assert!(remix.reads(Path::new("/p/apps/shop/source/routes/cart.tsx"), "loader"));
        assert!(remix.reads(Path::new("/p/apps/shop/source/root.tsx"), "links"));
        assert!(
            remix.reads(
                Path::new("/p/apps/shop/source/features/cart/route.tsx"),
                "loader"
            ),
            "a route named from routes.ts lives anywhere under the app directory"
        );
        assert!(
            !remix.reads(Path::new("/p/apps/shop/scripts/images.ts"), "loader"),
            "anybody's word outside the app directory"
        );
        assert!(
            !remix.reads(Path::new("/p/apps/shop/source/routes/cart.tsx"), "helper"),
            "not a name Remix has"
        );
        assert!(
            !remix.reads(Path::new("/p/apps/shop/app/routes/cart.tsx"), "loader"),
            "the config moved the routes"
        );
    }

    #[test]
    fn a_bare_global_holds_anywhere_in_its_own_project_and_nowhere_else() {
        let angular = builtin("angular").root(Path::new("/p/apps/admin"), None);
        assert!(angular.has_globals());
        assert!(angular.reads(
            Path::new("/p/apps/admin/src/app/orders/orders.component.ts"),
            "ngOnInit"
        ));
        assert!(
            !angular.reads(Path::new("/p/apps/api/src/orders.ts"), "ngOnInit"),
            "the package next door is not an Angular application"
        );
        assert!(!builtin("vite").root(Path::new("/p"), None).has_globals());
    }

    #[test]
    fn globals_are_written_as_names_or_as_names_with_their_files() {
        let yaml = "
frameworks:
  - name: house
    detect: { dependencies: [house] }
    globals:
      - onBoot
      - names: [screen, guard]
        files: ['screens/**/*.ts']
";
        let house = parse(yaml, Format::Yaml).unwrap().remove(0);
        assert_eq!(house.globals.len(), 2);
        let rooted = house.root(Path::new("/p"), None);
        assert!(rooted.reads(Path::new("/p/kit/start.ts"), "onBoot"));
        assert!(rooted.reads(Path::new("/p/screens/home/index.ts"), "guard"));
        assert!(!rooted.reads(Path::new("/p/kit/start.ts"), "guard"));

        for (broken, complaint) in [
            ("globals: ['']", "a global needs a name"),
            (
                "globals: [{ names: [x], files: ['../up/*.ts'] }]",
                "must stay inside",
            ),
            ("globals: [{ names: [x], files: ['a/{b'] }]", "glob 'a/{b'"),
        ] {
            let yaml = format!("frameworks: [{{ name: x, {broken} }}]");
            let error = parse(&yaml, Format::Yaml).unwrap_err();
            assert!(error.contains(complaint), "{broken}: {error}");
        }
    }

    #[test]
    fn a_python_framework_is_a_row_of_the_same_table() {
        // Nothing about detection or matching is JavaScript's: Django is
        // found by the file it cannot run without.
        let tree = TempTree::new("framework-django");
        tree.write("backend/manage.py", "");
        let registry = Registry::default();
        let detected = registry.detect(&tree.path().join("backend"), &signals(&[], &[]), false);
        assert_eq!(names(&detected), vec!["django"]);

        let django = &detected[0];
        let at = |path: &str| tree.path().join("backend").join(path);
        assert!(django.reaches(&at("shop/migrations/0007_add_index.py")));
        assert!(django.reaches(&at("shop/management/commands/reindex.py")));
        assert!(django.reaches(&at("shop/templatetags/shop_tags.py")));
        assert!(django.reaches(&at("shop/admin.py")));
        assert!(!django.reaches(&at("shop/services/pricing.py")));
        assert!(django.reads(&at("shop/urls.py"), "urlpatterns"));
        assert!(!django.reads(&at("shop/urls.py"), "helper"));
    }

    #[test]
    fn a_json_config_sets_a_variable_too() {
        let nest = builtin("nest").root(
            Path::new("/p"),
            Some(("nest-cli.json", r#"{ "sourceRoot": "server" }"#)),
        );
        assert!(nest.reaches(Path::new("/p/server/main.ts")));
        assert!(!nest.reaches(Path::new("/p/src/main.ts")));
    }

    #[test]
    fn a_variable_cannot_lead_out_of_the_project() {
        for escape in ["../elsewhere", "/etc", "a/../../b", "C:\\code"] {
            assert_eq!(relative_directory(escape), None, "{escape}");
        }
        assert_eq!(relative_directory("./src/"), Some("src".to_string()));
        assert_eq!(relative_directory("./"), Some(".".to_string()));

        let nuxt = builtin("nuxt").root(
            Path::new("/p"),
            Some(("nuxt.config.ts", "export default { srcDir: '../shared' }")),
        );
        assert!(!nuxt.reaches(Path::new("/shared/components/Card.vue")));
    }

    #[test]
    fn a_forced_framework_applies_at_a_scan_root_only() {
        let tree = TempTree::new("framework-forced");
        tree.write("pages/index.tsx", "");
        let mut registry = Registry::default();
        registry.force(["next".to_string()]);
        let nothing = signals(&[], &[]);
        assert_eq!(
            names(&registry.detect(tree.path(), &nothing, true)),
            vec!["next"]
        );
        assert!(registry.detect(tree.path(), &nothing, false).is_empty());
    }

    #[test]
    fn a_disabled_registry_detects_nothing() {
        let tree = TempTree::new("framework-disabled");
        tree.write("next.config.js", "module.exports = {}\n");
        let mut registry = Registry::default();
        registry.disable();
        assert!(
            registry
                .detect(tree.path(), &signals(&["next"], &[]), true)
                .is_empty()
        );
    }

    #[test]
    fn a_project_definition_adds_a_framework_or_replaces_a_built_in() {
        let yaml = "
frameworks:
  - name: house-router
    detect:
      dependencies: ['@acme/router']
    variables:
      screensDir: screens
    entry: ['${screensDir}/**/*.screen.tsx']
  - name: next
    detect:
      configFiles: ['next.config.js']
    directories: [everything]
";
        let mut registry = Registry::default();
        let before = registry.definitions().count();
        registry.extend(parse(yaml, Format::Yaml).unwrap());
        assert_eq!(registry.definitions().count(), before + 1);
        assert!(registry.knows("house-router"));
        assert_eq!(
            builtin_from(&registry, "next").directories,
            vec!["everything"]
        );

        let rooted = builtin_from(&registry, "house-router").root(Path::new("/p"), None);
        assert!(rooted.reaches(Path::new("/p/screens/home/index.screen.tsx")));
    }

    fn builtin_from(registry: &Registry, name: &str) -> Framework {
        registry
            .definitions()
            .find(|framework| framework.name == name)
            .unwrap()
            .clone()
    }

    #[test]
    fn json_and_yaml_say_the_same_thing() {
        let json = r#"{ "frameworks": [ { "name": "x", "detect": { "packageJsonKeys": ["x"] }, "entry": ["x/*.ts"] } ] }"#;
        let yaml = "frameworks:\n  - name: x\n    detect:\n      packageJsonKeys: [x]\n    entry: ['x/*.ts']\n";
        assert_eq!(
            parse(json, Format::Json).unwrap(),
            parse(yaml, Format::Yaml).unwrap()
        );
        assert_eq!(Format::of(Path::new("a/frameworks.JSON")), Format::Json);
        assert_eq!(Format::of(Path::new("a/frameworks.yml")), Format::Yaml);
    }

    #[test]
    fn a_definition_that_cannot_work_is_refused_with_its_name() {
        for (yaml, complaint) in [
            ("frameworks: [{ name: '' }]", "needs a name"),
            (
                "frameworks: [{ name: x, directories: ['../up'] }]",
                "must stay inside",
            ),
            (
                "frameworks: [{ name: x, entry: ['${missing}/a.ts'] }]",
                "does not declare",
            ),
            ("frameworks: [{ name: x, entry: ['a/{b'] }]", "glob 'a/{b'"),
            ("frameworks: [{ name: x, entrys: [] }]", "unknown field"),
        ] {
            let error = parse(yaml, Format::Yaml).unwrap_err();
            assert!(error.contains(complaint), "{yaml}: {error}");
        }
    }

    #[test]
    fn brace_alternatives_expand_to_every_name() {
        assert_eq!(
            expand_braces("vite.config.{js,ts}"),
            vec!["vite.config.js", "vite.config.ts"]
        );
        assert_eq!(
            expand_braces("{a,b}.{x,y}"),
            vec!["a.x", "a.y", "b.x", "b.y"]
        );
        assert_eq!(expand_braces("x{a,{b,c}}"), vec!["xa", "xb", "xc"]);
        assert_eq!(expand_braces("angular.json"), vec!["angular.json"]);
        assert_eq!(expand_braces("odd{name"), vec!["odd{name"]);
    }

    #[test]
    fn the_project_file_is_found_by_any_of_its_names() {
        let tree = TempTree::new("framework-project-file");
        assert_eq!(project_file(tree.path()), None);
        tree.write("basta.frameworks.yml", "frameworks: []\n");
        assert_eq!(
            project_file(tree.path()),
            Some(tree.path().join("basta.frameworks.yml"))
        );
        assert!(
            load(&tree.path().join("basta.frameworks.yml"))
                .unwrap()
                .is_empty()
        );
        let missing = load(&tree.path().join("nope.yaml")).unwrap_err();
        assert!(missing.contains("nope.yaml"), "{missing}");
    }
}
