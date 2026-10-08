//! Projects: how the `.jscpd.json` files split a workspace. With none, the
//! whole workspace, all its folders, is one project. Each `.jscpd.json`
//! makes its folder a project with that config, and one inside the folder of
//! another splits its subfolder out, so a file belongs to the project of the
//! nearest config above it. The files under no config form one more project,
//! with the defaults. Clones are found within a project, never across two.

use super::findings::Snapshot;
use super::settings::{Analyses, Analysis};
use crate::cli::{Cli, ConfigDiagnostic, ConfigFile, config_from_json};
use crate::index::ScanIndex;
use crate::options::Options;
use cpd_finder::orchestrate::RunConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const CONFIG_NAME: &str = ".jscpd.json";

/// Where a project's files are, before its config is read.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The folder of its `.jscpd.json`; `None` for the project of the files
    /// under no config.
    pub config_dir: Option<PathBuf>,
    pub roots: Vec<PathBuf>,
    /// Folders of other projects inside `roots`.
    pub excluded: Vec<PathBuf>,
}

/// The folders of every `.jscpd.json` in `folders`, skipping what
/// `.gitignore` skips, parents before their subfolders.
pub fn find_config_dirs(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for folder in folders {
        let mut builder = ignore::WalkBuilder::new(folder);
        builder.hidden(false).filter_entry(|entry| {
            entry.file_name() != ".git" && entry.file_name() != "node_modules"
        });
        for entry in builder.build().flatten() {
            if entry.file_name() == CONFIG_NAME
                && entry.file_type().is_some_and(|t| t.is_file())
                && let Some(dir) = entry.path().parent()
            {
                dirs.push(std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf()));
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// The projects of a workspace with these `folders` and config folders.
pub fn plan(folders: &[PathBuf], config_dirs: &[PathBuf]) -> Vec<Plan> {
    let mut plans: Vec<Plan> = config_dirs
        .iter()
        .map(|dir| Plan {
            config_dir: Some(dir.clone()),
            roots: vec![dir.clone()],
            excluded: config_dirs
                .iter()
                .filter(|other| *other != dir && other.starts_with(dir))
                .cloned()
                .collect(),
        })
        .collect();
    // The folders no config covers from their root, for the files under no
    // config; a config inside one of them takes its subfolder out.
    let rest: Vec<PathBuf> = folders
        .iter()
        .filter(|folder| !config_dirs.iter().any(|dir| folder.starts_with(dir)))
        .cloned()
        .collect();
    if !rest.is_empty() {
        plans.push(Plan {
            config_dir: None,
            excluded: config_dirs
                .iter()
                .filter(|dir| rest.iter().any(|folder| dir.starts_with(folder)))
                .cloned()
                .collect(),
            roots: rest,
        });
    }
    plans
}

/// Deep-merge `overlay` into `base`: objects merge key by key, and any other
/// value of `overlay` replaces the one in `base`.
pub fn merge_json(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base), serde_json::Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => *base = overlay.clone(),
    }
}

/// A project: its files, its options and, once scanned, its index.
pub struct Project {
    pub plan: Plan,
    /// The command line with the flags this project's config sets taken
    /// out: what the modes this server borrows from read their options from.
    pub cli: Cli,
    pub options: Options,
    pub analyses: Analyses,
    pub run: RunConfig,
    /// What reading the config said: unknown keys, bad values.
    pub diagnostics: Vec<ConfigDiagnostic>,
    /// Why the project cannot run (a key in its config), or `None`.
    pub refused: Option<String>,
    pub index: Option<ScanIndex>,
    /// basta's configuration, when the dead-code analysis is on and its
    /// options are right.
    pub dead_code_config: Option<basta::config::BastaConfig>,
    /// The last dead-code run's findings, by file, and the files as it read
    /// them.
    pub dead_code: Vec<(PathBuf, cpd_core::deadcode::Finding)>,
    pub dead_code_snapshots: HashMap<PathBuf, Snapshot>,
    /// The model and thresholds of the semantic analysis, when it is on.
    pub semantic_options: Option<cpd_semantic::SemanticOptions>,
    /// The last semantic run's pairs, and the files as it read them.
    pub semantic: Vec<cpd_core::models::CpdClone>,
    pub semantic_snapshots: HashMap<PathBuf, Snapshot>,
}

impl Project {
    /// The project of `plan`: its `.jscpd.json` with the editor's `settings`
    /// merged on top, and the command line's flags for what neither sets.
    pub fn new(plan: Plan, cli: &Cli, defaults: &[Analysis], settings: &serde_json::Value) -> Self {
        let base = plan
            .config_dir
            .clone()
            .or_else(|| plan.roots.first().cloned())
            .unwrap_or_default();
        let config_path = base.join(CONFIG_NAME);
        // A file that does not parse, often one the user is typing into,
        // leaves the project on the defaults and says why.
        let mut unparsed = None;
        let mut value = match &plan.config_dir {
            Some(_) => match std::fs::read_to_string(&config_path)
                .map_err(|e| (None, e.to_string()))
                .and_then(|text| {
                    serde_json::from_str(&text).map_err(|e| (Some(e.line()), e.to_string()))
                }) {
                Ok(value) => value,
                Err((line, error)) => {
                    unparsed = Some(ConfigDiagnostic::ParseError {
                        source: config_path.clone(),
                        line,
                        error,
                    });
                    serde_json::json!({})
                }
            },
            None => serde_json::json!({}),
        };
        if !value.is_object() {
            value = serde_json::json!({});
        }
        merge_json(&mut value, settings);
        let mut result = config_from_json(value, &config_path, &base);
        result.diagnostics.extend(unparsed);
        let refused = result
            .diagnostics
            .iter()
            .find(|d| d.stops_any_run())
            .map(|d| d.to_string());
        let config = result.config;
        let cli = cli_as_defaults(cli, &config);
        let mut options = Options::from_cli_and_config(&cli, &config);
        options.paths = scan_roots(&plan, &config);
        // Relative folders are the config folder's, as for a CLI run started
        // there; the server's own working directory is not the project's.
        for group in &mut options.skip_isolated {
            for folder in group.iter_mut() {
                if Path::new(folder.as_str()).is_relative() {
                    *folder = base.join(folder.as_str()).to_string_lossy().into_owned();
                }
            }
        }
        let analyses = Analyses::resolve(defaults, config.lsp.as_ref(), options.similarity);
        let mut run = crate::run_config(&options, &options.paths);
        // The switches, not `--kind`, choose what the server shows.
        run.kinds.clear();
        run.similarity = analyses
            .has(Analysis::Ast)
            .then_some(analyses.ast_similarity);
        // The semantic section's model and thresholds, whether or not the
        // file turns `--semantic` on for the CLI's own runs.
        let semantic_options = analyses
            .has(Analysis::Semantic)
            .then(|| crate::options::semantic_options(&cli, &config, cli.semantic_model.clone()));
        Self {
            plan,
            cli,
            options,
            analyses,
            run,
            diagnostics: result.diagnostics,
            refused,
            index: None,
            dead_code_config: None,
            dead_code: Vec::new(),
            dead_code_snapshots: HashMap::new(),
            semantic_options,
            semantic: Vec::new(),
            semantic_snapshots: HashMap::new(),
        }
    }

    /// Whether the file at `path` belongs to this project.
    pub fn owns(&self, path: &Path) -> bool {
        self.plan.roots.iter().any(|root| path.starts_with(root))
            && !self.plan.excluded.iter().any(|dir| path.starts_with(dir))
    }

    /// How deep this project's folder is: the project of the nearest config
    /// above a file is the deepest one that owns it.
    pub fn depth(&self) -> usize {
        self.plan
            .config_dir
            .as_ref()
            .map_or(0, |dir| dir.components().count())
    }
}

/// The roots a project scans: its config's `path` entries, resolved against
/// its folder, or its folders.
fn scan_roots(plan: &Plan, config: &ConfigFile) -> Vec<PathBuf> {
    match (&plan.config_dir, &config.path) {
        (Some(_), Some(paths)) if !paths.is_empty() => paths
            .iter()
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p)))
            .collect(),
        _ => plan.roots.clone(),
    }
}

/// `cli` with the flags a project's config sets taken out, so the command
/// line holds the defaults and the config wins over them. The CLI's own
/// runs keep the flags first; a language server is started by an editor,
/// and its projects configure themselves.
fn cli_as_defaults(cli: &Cli, config: &ConfigFile) -> Cli {
    let mut cli = cli.clone();
    macro_rules! defaults {
        ($($field:ident),* $(,)?) => {
            $(if config.$field.is_some() {
                cli.$field = Default::default();
            })*
        };
    }
    defaults!(
        min_tokens,
        min_lines,
        max_lines,
        max_gap_lines,
        similarity,
        min_nodes,
        mode,
        ignore,
        ignore_pattern,
        pattern,
        kind,
        max_size,
        no_gitignore,
        follow_symlinks,
        ignore_case,
        ignore_identifiers,
        ignore_literals,
        ignore_annotations,
        formats_exts,
        formats_names,
        cross_formats,
        skip_local,
        skip_isolated,
        sarif_error_tokens,
        min_confidence,
        entry,
        include_tests,
        include_entry_exports,
        dead_code_categories,
    );
    if config.format.is_some() {
        cli.format.clear();
    }
    if config.mode.is_some() {
        cli.skip_comments = false;
    }
    if let Some(semantic) = &config.semantic {
        if semantic.scope.is_some() {
            cli.semantic_scope = None;
        }
        if semantic.provider.is_some() {
            cli.semantic_provider = None;
        }
        if semantic.threshold.is_some() {
            cli.semantic_threshold = None;
        }
        if semantic.same_threshold.is_some() {
            cli.semantic_same_threshold = None;
        }
        if semantic.model.is_some() {
            cli.semantic_model = None;
        }
        if semantic.url.is_some() {
            cli.semantic_url = None;
        }
    }
    // The dead-code section wins over the command line like the flat keys
    // it replaces.
    if let Some(crate::cli::DeadCodeSetting::Section(section)) = &config.dead_code {
        if section.categories.is_some() {
            cli.dead_code_categories.clear();
        }
        if section.min_confidence.is_some() {
            cli.min_confidence = None;
        }
        if section.entry.is_some() {
            cli.entry.clear();
        }
        if section.include_tests.is_some() {
            cli.include_tests = false;
        }
        if section.include_entry_exports.is_some() {
            cli.include_entry_exports = false;
        }
        if section.rust_diagnostics.is_some() {
            cli.rust_diagnostics = None;
        }
    }
    // The paths of a project are its folders.
    cli.paths.clear();
    cli
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    /// A fresh folder with `config` as its `.jscpd.json` and a `src`
    /// folder, canonical like the folders of a workspace.
    fn config_dir(name: &str, config: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jscpd-lsp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join(CONFIG_NAME), config).unwrap();
        std::fs::canonicalize(dir).unwrap()
    }

    fn project_of(dir: &Path, config_dir: Option<&Path>, args: &[&str]) -> Project {
        use clap::Parser;
        let cli = Cli::parse_from(["jscpd", "--lsp"].iter().chain(args));
        let plan = Plan {
            config_dir: config_dir.map(Path::to_path_buf),
            roots: vec![dir.to_path_buf()],
            excluded: Vec::new(),
        };
        Project::new(plan, &cli, &[Analysis::Clones], &serde_json::json!({}))
    }

    /// The flags an editor starts the server with, each set against what
    /// the config below says.
    const FLAGS: &[&str] = &[
        "--min-nodes",
        "30",
        "--format",
        "javascript",
        "--mode",
        "weak",
        "--semantic-model",
        "model-of-the-flag",
        "--semantic-url",
        "http://flag.invalid/v1",
        "--semantic-threshold",
        "0.5",
        "--semantic-same-threshold",
        "0.55",
        "--entry",
        "flag.js",
        "--min-confidence",
        "10",
        "--include-tests",
    ];

    const CONFIG: &str = r#"{
        "minNodes": 12,
        "format": ["python"],
        "mode": "strict",
        "path": ["src"],
        "skipIsolated": [["src/a", "src/b"]],
        "semantic": {"model": "model-of-the-config", "url": "http://config.invalid/v1", "threshold": 0.8, "sameThreshold": 0.85},
        "deadCode": {"entry": ["config.js"], "minConfidence": 90, "includeTests": false},
        "lsp": {"semantic": {"enabled": true}}
    }"#;

    #[test]
    fn a_project_config_wins_over_the_flags_of_the_server() {
        let dir = config_dir("config-wins", CONFIG);
        let project = project_of(&dir, Some(&dir), FLAGS);
        let options = &project.options;
        assert_eq!(options.min_nodes, 12);
        assert_eq!(options.formats, ["python"]);
        assert_eq!(options.mode, cpd_tokenizer::tokenizer::Mode::Strict);
        // Folders of the config are the config folder's.
        assert_eq!(options.paths, [dir.join("src")]);
        let folder = |f: &str| dir.join(f).to_string_lossy().into_owned();
        assert_eq!(
            options.skip_isolated,
            [vec![folder("src/a"), folder("src/b")]]
        );
        let semantic = project.semantic_options.as_ref().expect("semantic is on");
        assert_eq!(semantic.model, "model-of-the-config");
        assert_eq!(semantic.url, "http://config.invalid/v1");
        assert_eq!(semantic.threshold, 0.8);
        assert_eq!(semantic.same_threshold, Some(0.85));
        assert_eq!(options.entry, ["config.js"]);
        assert_eq!(options.min_confidence, Some(90));
        assert!(!options.include_tests);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_a_config_the_flags_of_the_server_stay() {
        let dir = config_dir("flags-stay", "{}");
        let project = project_of(&dir, None, FLAGS);
        let options = &project.options;
        assert_eq!(options.min_nodes, 30);
        assert_eq!(options.formats, ["javascript"]);
        assert_eq!(options.mode, cpd_tokenizer::tokenizer::Mode::Weak);
        assert_eq!(
            options.paths,
            std::slice::from_ref(&dir),
            "the project's folder"
        );
        assert_eq!(options.entry, ["flag.js"]);
        assert_eq!(options.min_confidence, Some(10));
        assert!(options.include_tests);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_config_that_is_not_an_object_is_left_out() {
        let dir = config_dir("not-object", "[1, 2]");
        let project = project_of(&dir, Some(&dir), &["--min-tokens", "33"]);
        assert_eq!(project.options.min_tokens, 33, "the flag");
        assert!(project.refused.is_none());
        assert_eq!(project.options.paths, std::slice::from_ref(&dir));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_workspace_without_configs_is_one_project() {
        let plans = plan(&[p("/w/a"), p("/w/b")], &[]);
        assert_eq!(
            plans,
            [Plan {
                config_dir: None,
                roots: vec![p("/w/a"), p("/w/b")],
                excluded: vec![],
            }]
        );
    }

    #[test]
    fn each_config_is_a_project_and_a_nested_one_splits_its_folder_out() {
        let plans = plan(
            &[p("/w/app"), p("/w/lib")],
            &[p("/w/app"), p("/w/app/packages/ui"), p("/w/lib/sub")],
        );
        assert_eq!(plans.len(), 4);
        assert_eq!(plans[0].config_dir, Some(p("/w/app")));
        assert_eq!(plans[0].excluded, [p("/w/app/packages/ui")]);
        assert_eq!(plans[1].config_dir, Some(p("/w/app/packages/ui")));
        assert!(plans[1].excluded.is_empty());
        // /w/lib has no config at its root: its files outside /w/lib/sub are
        // the files under no config.
        assert_eq!(plans[3].config_dir, None);
        assert_eq!(plans[3].roots, [p("/w/lib")]);
        assert_eq!(plans[3].excluded, [p("/w/lib/sub")]);
    }

    #[test]
    fn editor_settings_merge_into_the_config_key_by_key() {
        let mut config = serde_json::json!({"minTokens": 50, "lsp": {"clones": {"enabled": true}}});
        merge_json(
            &mut config,
            &serde_json::json!({"minTokens": 30, "lsp": {"complexity": {"enabled": true}}}),
        );
        assert_eq!(
            config,
            serde_json::json!({"minTokens": 30, "lsp": {"clones": {"enabled": true}, "complexity": {"enabled": true}}})
        );
    }

    #[test]
    fn a_config_that_does_not_parse_leaves_the_defaults_and_says_why() {
        use clap::Parser;
        let dir = std::env::temp_dir().join(format!("jscpd-lsp-unparsed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(CONFIG_NAME), "{\n  \"minTokens\": 20,\n}\n").unwrap();
        let cli = Cli::parse_from(["jscpd", "--lsp"]);
        let plan = Plan {
            config_dir: Some(dir.clone()),
            roots: vec![dir.clone()],
            excluded: Vec::new(),
        };
        let project = Project::new(plan, &cli, &[Analysis::Clones], &serde_json::json!({}));
        assert_eq!(project.options.min_tokens, 50, "the defaults");
        assert!(
            matches!(
                project.diagnostics.as_slice(),
                [ConfigDiagnostic::ParseError { line: Some(3), .. }]
            ),
            "{:?}",
            project.diagnostics
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
