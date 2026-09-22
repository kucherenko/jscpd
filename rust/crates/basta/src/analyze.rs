//! The run: walk, analyze, resolve, report.
//!
//! One pass over the filesystem, one analyzer call per file in parallel, then
//! a single-threaded resolution phase — the graph is a whole-project object
//! and cannot be built a file at a time.
//!
//! File discovery is jscpd's own walker, so `--ignore`, `.gitignore`
//! handling, symlink behavior and the format table are the ones a jscpd user
//! already knows. A dead-code run and a duplication run over the same
//! directory see exactly the same files.

use crate::classify;
use crate::config::BastaConfig;
use crate::entry;
use crate::finding::{CategoryCount, Finding, Report, Stats};
use crate::framework::DetectedFramework;
use crate::graph::{Graph, ModuleInput};
use crate::lang::{self, AnalyzeInput, Analyzer};
use crate::model::{FileFacts, Module, ModuleId, SymbolFlags, SymbolKind};
use crate::resolve::ModuleIndex;
use crate::rustc;
use cpd_finder::walker::{WalkConfig, walk};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Everything a run produced, plus the graph it was derived from so callers
/// that want more than the findings (the MCP server, tests) need not rebuild it.
pub struct RunResult {
    pub report: Report,
    pub graph: Graph,
    /// Frameworks found at work, which explain entry points no file names.
    /// Each directory is relative to its scan root, empty for the root itself.
    pub frameworks: Vec<DetectedFramework>,
}

/// Run dead-code detection over the configured paths.
pub fn run(config: &BastaConfig) -> RunResult {
    let roots = canonical_roots(&config.paths);
    let discovered = discover(config);

    // Read every file in parallel. A file that is not valid UTF-8 is kept as
    // a module that failed to parse — it is in the tree, so it belongs in the
    // count, and its references are as unknown as any parse failure's.
    let pool = cpd_finder::orchestrate::build_thread_pool(config.workers);
    let analyzed: Vec<Analyzed> = pool.install(|| {
        discovered
            .into_par_iter()
            .filter_map(|file| {
                let analyzer = lang::analyzer_for(&file.format)?;
                let display = display_path(&file.path, &roots);
                let source = match std::fs::read_to_string(&file.real_path) {
                    Ok(source) => Some(source),
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidData => None,
                    Err(_) => return None, // Gone between walk and read.
                };
                Some(Analyzed {
                    lines: source.as_deref().map_or(0, count_lines),
                    self_starting: source
                        .as_deref()
                        .is_some_and(|text| analyzer.is_self_starting(text)),
                    traits: analyzer.module_traits(&display),
                    analyzer,
                    source,
                    format: file.format,
                    display,
                    real_path: file.real_path,
                })
            })
            .collect()
    });

    // Order by display path so a run over the same tree always numbers the
    // modules the same way, whatever order the walker happened to finish in.
    let mut analyzed = analyzed;
    analyzed.sort_by(|a, b| a.display.cmp(&b.display));

    let modules: Vec<Module> = analyzed
        .iter()
        .enumerate()
        .map(|(index, file)| Module {
            id: ModuleId(index as u32),
            path: file.display.clone(),
            real_path: file.real_path.clone(),
            format: file.format.clone(),
            language: file.analyzer.language(),
            traits: file.traits,
            lines: file.lines,
            is_entry: file.self_starting,
            is_test: false,
            has_dynamic_access: false,
            parse_failed: false,
        })
        .collect();

    let entries = entry::detect(&modules, &roots, &config.entry, &config.frameworks);
    let mut index = ModuleIndex::new(roots.clone());
    for module in &modules {
        index.insert(module.real_path.clone(), module.id);
    }
    // A project that renames its own import paths must be read on its own
    // terms: without this, every `@/thing` import is invisible and the files
    // it names look unreachable.
    index.set_aliases(entry::path_aliases(&modules, &roots));
    // Roots the tree implies rather than the user names: a `src/` layout.
    let scanned: Vec<PathBuf> = modules.iter().map(|m| m.real_path.clone()).collect();
    index.set_import_roots(
        lang::ANALYZERS
            .iter()
            .flat_map(|analyzer| analyzer.import_roots(&scanned))
            .collect(),
    );

    // Parsing dominates the run, so it happens in parallel; everything after
    // this point needs the whole project at once.
    let facts: Vec<FileFacts> = pool.install(|| {
        analyzed
            .par_iter()
            .enumerate()
            .map(|(position, file)| match &file.source {
                Some(source) => file.analyzer.analyze(&AnalyzeInput {
                    module: ModuleId(position as u32),
                    format: &file.format,
                    path: &file.display,
                    source,
                }),
                None => FileFacts::unparsed(),
            })
            .collect()
    });

    // A declaration under a name the project's framework reads is used by the
    // framework. Marked here, between the per-file facts and the graph,
    // because it is the first point where both the name and the framework
    // are known.
    let mut facts = facts;
    if entries.has_framework_globals() {
        for (module, facts) in modules.iter().zip(&mut facts) {
            let readers = entries.global_readers(&module.real_path);
            if readers.is_empty() {
                continue;
            }
            let reads = |name: &str| readers.iter().any(|r| r.reads(&module.real_path, name));
            for symbol in &mut facts.symbols {
                let read = symbol.kind != SymbolKind::Import
                    && (reads(&symbol.name) || symbol.export_name().is_some_and(&reads));
                if read {
                    symbol.flags.insert(SymbolFlags::FRAMEWORK_GLOBAL);
                }
            }
        }
    }

    let total_lines: u32 = modules.iter().map(|m| m.lines).sum();
    let inputs: Vec<ModuleInput> = modules
        .into_iter()
        .zip(facts)
        .enumerate()
        .map(|(index, (module, facts))| ModuleInput {
            is_entry: module.is_entry || entries.is_entry(index),
            is_test: entries.is_test(index),
            module,
            facts,
        })
        .collect();

    let graph = Graph::build(inputs, &index);
    let findings = classify::findings(&graph, config);
    let statistics = stats(&graph, &findings, total_lines, entries.entry_count());
    let mut report = Report {
        findings,
        statistics,
    };
    // The compiler's findings for Rust join the graph's for everything
    // else, over the same roots, into one report — here, so that every
    // front end (the basta binary, jscpd --dead-code, --dashboard,
    // --health) sees the same thing.
    if let Some(diagnostics) = &config.rust_diagnostics {
        let base = diagnostics
            .base
            .clone()
            .or_else(|| roots.first().cloned())
            .unwrap_or_else(|| PathBuf::from("."));
        let rust_findings = rustc::findings(&diagnostics.text, &roots, &base, config.include_tests);
        let rust = rustc::sources(&roots, config.no_gitignore);
        report = rustc::merge(report, rust_findings, rust);
    }
    RunResult {
        report,
        graph,
        frameworks: entries
            .frameworks()
            .iter()
            .map(|framework| DetectedFramework {
                name: framework.name.clone(),
                directory: PathBuf::from(display_path(&framework.directory, &roots)),
            })
            .collect(),
    }
}

/// A file read and classified, before it is parsed.
struct Analyzed {
    /// `None` when the bytes were not UTF-8: the file counts, but cannot be
    /// analyzed.
    source: Option<String>,
    format: String,
    display: String,
    real_path: PathBuf,
    analyzer: &'static dyn Analyzer,
    traits: crate::model::ModuleTraits,
    lines: u32,
    self_starting: bool,
}

fn discover(config: &BastaConfig) -> Vec<cpd_finder::walker::DiscoveredFile> {
    // An empty `--format` means every format basta can analyze, not every
    // format jscpd can tokenize: walking a Java tree only to drop it is
    // wasted I/O, and the walker filters by extension for free.
    let formats = if config.formats.is_empty() {
        lang::supported_formats()
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        config.formats.clone()
    };
    let walk_config = WalkConfig {
        paths: if config.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            config.paths.clone()
        },
        extensions: formats,
        ignore_patterns: config.ignore.clone(),
        max_size: config.max_size,
        follow_symlinks: config.follow_symlinks,
        no_gitignore: config.no_gitignore,
        formats_exts: config.formats_exts.clone(),
        formats_names: Default::default(),
        pattern: None,
    };
    walk(&walk_config)
}

fn canonical_roots(paths: &[PathBuf]) -> Vec<PathBuf> {
    let paths: Vec<PathBuf> = if paths.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
    } else {
        paths.to_vec()
    };
    paths
        .into_iter()
        .map(|path| {
            let canonical = std::fs::canonicalize(&path).unwrap_or(path);
            if canonical.is_file() {
                canonical
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or(canonical)
            } else {
                canonical
            }
        })
        .collect()
}

/// The scan-root-relative path a report shows, with `/` separators so a
/// report reads the same on every platform.
fn display_path(path: &Path, roots: &[PathBuf]) -> String {
    let relative = roots
        .iter()
        .find_map(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn count_lines(source: &str) -> u32 {
    if source.is_empty() {
        return 0;
    }
    let newlines = source.bytes().filter(|b| *b == b'\n').count();
    let trailing = u32::from(!source.ends_with('\n'));
    newlines as u32 + trailing
}

fn stats(graph: &Graph, findings: &[Finding], total_lines: u32, entry_points: usize) -> Stats {
    let mut by_category: Vec<CategoryCount> = Vec::new();
    for finding in findings {
        match by_category
            .iter_mut()
            .find(|c| c.category == finding.category)
        {
            Some(existing) => {
                existing.count += 1;
                existing.lines += finding.lines;
            }
            None => by_category.push(CategoryCount {
                category: finding.category,
                count: 1,
                lines: finding.lines,
            }),
        }
    }
    by_category.sort_by_key(|c| c.category);

    // Lines are counted once per file even when several findings overlap, so
    // the percentage cannot exceed 100 on a file whose every declaration is
    // dead.
    let dead_lines = by_category
        .iter()
        .map(|c| c.lines)
        .sum::<u32>()
        .min(total_lines);
    let unparsed_files: Vec<String> = graph
        .modules
        .iter()
        .filter(|m| m.parse_failed)
        .map(|m| m.path.clone())
        .collect();
    Stats {
        files: graph.modules.len() as u32,
        unparsed: unparsed_files.len() as u32,
        unparsed_files,
        reachable_files: graph.reachable_module_count() as u32,
        symbols: graph.symbols.len() as u32,
        entry_points: entry_points as u32,
        by_category,
        dead_lines,
        total_lines,
        percentage: if total_lines == 0 {
            0.0
        } else {
            (f64::from(dead_lines) / f64::from(total_lines)) * 100.0
        },
        detection_date: now_iso8601(),
    }
}

pub(crate) fn now_iso8601() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs();
    chrono::DateTime::from_timestamp(seconds as i64, duration.subsec_millis() * 1_000_000)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|| seconds.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::deadcode::Category;

    /// Write a tree under a fresh temporary directory and scan it.
    fn scan(name: &str, files: &[(&str, &str)], config: BastaConfig) -> (Report, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "basta-analyze-{}-{name}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&root).ok();
        for (path, source) in files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, source).unwrap();
        }
        let config = BastaConfig {
            paths: vec![root.clone()],
            ..config
        };
        (run(&config).report, root)
    }

    fn cleanup(root: &Path) {
        std::fs::remove_dir_all(root).ok();
    }

    fn everything() -> BastaConfig {
        BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            ..BastaConfig::default()
        }
    }

    #[test]
    fn a_real_typescript_tree_is_walked_analyzed_and_reported() {
        let (report, root) = scan(
            "ts",
            &[
                (
                    "src/index.ts",
                    "import { used } from './api';\nimport { spare } from './api';\nused();\n",
                ),
                (
                    "src/api.ts",
                    "export function used() {}\nexport function spare() {}\nexport function neverImported() {}\n",
                ),
                ("src/orphan.ts", "export const x = 1;\n"),
            ],
            everything(),
        );
        let messages: Vec<&str> = report.findings.iter().map(|f| f.message.as_str()).collect();
        assert!(
            messages.iter().any(|m| m.contains("src/orphan.ts")),
            "{messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("neverImported")),
            "{messages:?}"
        );
        assert!(
            report.findings.iter().all(|f| f.path.starts_with("src/")),
            "paths are scan-root-relative: {:?}",
            report.findings.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        cleanup(&root);
    }

    #[test]
    fn a_python_package_resolves_through_its_own_imports() {
        let (report, root) = scan(
            "py",
            &[
                ("app/__init__.py", ""),
                ("app/__main__.py", "from .core import run\n\nrun()\n"),
                (
                    "app/core.py",
                    "def run():\n    pass\n\n\ndef never_called():\n    pass\n",
                ),
            ],
            everything(),
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.name == "never_called" && f.path == "app/core.py"),
            "{:?}",
            report
                .findings
                .iter()
                .map(|f| &f.message)
                .collect::<Vec<_>>()
        );
        assert!(
            !report.findings.iter().any(|f| f.name == "run"),
            "`run` is imported and called"
        );
        cleanup(&root);
    }

    #[test]
    fn statistics_describe_the_run() {
        let (report, root) = scan(
            "stats",
            &[
                ("src/index.ts", "import './a';\n"),
                ("src/a.ts", "export const a = 1;\n"),
                ("src/orphan.ts", "export const b = 2;\n"),
            ],
            everything(),
        );
        let stats = &report.statistics;
        assert_eq!(stats.files, 3);
        assert_eq!(stats.unparsed, 0);
        assert_eq!(stats.reachable_files, 2, "orphan.ts is not reached");
        assert!(stats.entry_points >= 1);
        assert!(stats.symbols >= 2);
        assert_eq!(stats.total_lines, 3);
        assert!(stats.dead_lines <= stats.total_lines);
        assert!(stats.percentage > 0.0 && stats.percentage <= 100.0);
        assert!(
            stats.detection_date.ends_with('Z'),
            "{}",
            stats.detection_date
        );
        assert_eq!(stats.total_findings() as usize, report.findings.len());
        cleanup(&root);
    }

    #[test]
    fn formats_basta_cannot_analyze_are_never_walked() {
        let (report, root) = scan(
            "mixed",
            &[
                ("src/index.ts", "export const a = 1;\n"),
                ("src/Thing.java", "class Thing { void m() {} }\n"),
                ("README.md", "# hello\n"),
            ],
            everything(),
        );
        assert_eq!(
            report.statistics.files, 1,
            "only the TypeScript file is basta's business"
        );
        cleanup(&root);
    }

    #[test]
    fn the_ignore_option_keeps_files_out_of_the_scan() {
        let config = BastaConfig {
            ignore: vec!["**/generated/**".to_string()],
            ..everything()
        };
        let (report, root) = scan(
            "ignore",
            &[
                ("src/index.ts", "export const a = 1;\n"),
                ("src/generated/big.ts", "export const b = 2;\n"),
            ],
            config,
        );
        assert_eq!(report.statistics.files, 1);
        assert!(
            report
                .findings
                .iter()
                .all(|f| !f.path.contains("generated"))
        );
        cleanup(&root);
    }

    #[test]
    fn an_unreadable_file_does_not_fail_the_run() {
        let (report, root) = scan(
            "broken",
            &[
                ("src/index.ts", "import './broken';\n"),
                ("src/broken.ts", "function ( { { {\n"),
            ],
            everything(),
        );
        assert_eq!(report.statistics.files, 2);
        assert_eq!(report.statistics.unparsed, 1);
        assert_eq!(
            report.statistics.unparsed_files,
            vec!["src/broken.ts"],
            "a reader has to be able to tell a broken fixture from a real gap"
        );
        assert!(
            !report.findings.iter().any(|f| f.path == "src/broken.ts"),
            "a file basta could not read is never a finding"
        );
        cleanup(&root);
    }

    #[test]
    fn a_file_that_is_not_utf8_counts_as_unparsed_rather_than_vanishing() {
        let root =
            std::env::temp_dir().join(format!("basta-analyze-latin1-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/index.ts"), "export const a = 1;\n").unwrap();
        std::fs::write(root.join("src/legacy.js"), b"var caf\xe9 = 1;\n").unwrap();
        let report = run(&BastaConfig {
            paths: vec![root.clone()],
            ..everything()
        })
        .report;
        assert_eq!(report.statistics.files, 2, "the file is in the tree");
        assert_eq!(report.statistics.unparsed_files, vec!["src/legacy.js"]);
        assert!(!report.findings.iter().any(|f| f.path == "src/legacy.js"));
        cleanup(&root);
    }

    #[test]
    fn an_empty_scan_produces_an_empty_report_rather_than_an_error() {
        let (report, root) = scan("empty", &[("notes.txt", "nothing here\n")], everything());
        assert!(report.findings.is_empty());
        assert_eq!(report.statistics.files, 0);
        assert_eq!(report.statistics.percentage, 0.0);
        cleanup(&root);
    }

    #[test]
    fn module_ids_do_not_depend_on_the_order_the_walker_finished_in() {
        let files = [
            ("src/index.ts", "import './a';\nimport './b';\n"),
            ("src/a.ts", "export const a = 1;\n"),
            ("src/b.ts", "export const b = 2;\n"),
            ("src/c.ts", "export const c = 3;\n"),
        ];
        let (first, root) = scan("order", &files, everything());
        let (second, _) = scan("order", &files, everything());
        let names =
            |r: &Report| -> Vec<String> { r.findings.iter().map(|f| f.fingerprint()).collect() };
        assert_eq!(names(&first), names(&second));
        cleanup(&root);
    }

    #[test]
    fn counting_lines_matches_what_an_editor_shows() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("a\n"), 1);
        assert_eq!(count_lines("a"), 1);
        assert_eq!(count_lines("a\nb\n"), 2);
        assert_eq!(count_lines("a\nb"), 2);
    }

    #[test]
    fn display_paths_are_relative_and_use_forward_slashes() {
        let roots = vec![PathBuf::from("/p")];
        assert_eq!(display_path(Path::new("/p/src/a.ts"), &roots), "src/a.ts");
        assert_eq!(
            display_path(Path::new("/elsewhere/a.ts"), &roots),
            "/elsewhere/a.ts",
            "a path outside every root keeps its own name"
        );
    }

    #[test]
    fn a_name_the_framework_reads_is_used_and_keeps_what_it_calls_alive() {
        let files = [
            (
                "package.json",
                r#"{ "dependencies": { "next": "15.0.0" } }"#,
            ),
            (
                "pages/orders.tsx",
                "import { loadOrders } from '../lib/orders';\n\
                 export async function getServerSideProps() {\n  return { props: { orders: loadOrders() } };\n}\n\
                 export function formatForExport() {\n  return 'csv';\n}\n\
                 export default function Orders() {\n  return null;\n}\n",
            ),
            (
                "lib/orders.ts",
                "export function loadOrders() {\n  return [];\n}\n\
                 export function getServerSideProps() {\n  return 'not a page';\n}\n",
            ),
        ];
        let strict = || BastaConfig {
            include_entry_exports: true,
            ..everything()
        };
        let names = |report: &Report| -> Vec<String> {
            report
                .findings
                .iter()
                .map(|finding| format!("{}:{}", finding.path, finding.name))
                .collect()
        };

        let (report, root) = scan("framework-globals", &files, strict());
        let found = names(&report);
        cleanup(&root);
        assert!(
            !found.contains(&"pages/orders.tsx:getServerSideProps".to_string()),
            "Next calls it by name: {found:?}"
        );
        assert!(
            found.contains(&"pages/orders.tsx:formatForExport".to_string()),
            "an export Next has no name for is still nobody's: {found:?}"
        );
        assert!(
            found.contains(&"lib/orders.ts:getServerSideProps".to_string()),
            "the name means nothing outside pages/: {found:?}"
        );
        assert!(
            !found.contains(&"lib/orders.ts:loadOrders".to_string()),
            "reached through the function the framework calls: {found:?}"
        );

        // Without the framework the name is one more export nothing imports.
        let mut off = strict();
        off.frameworks.disable();
        let (report, root) = scan("framework-globals-off", &files, off);
        let found = names(&report);
        cleanup(&root);
        assert!(
            found.contains(&"pages/orders.tsx:getServerSideProps".to_string()),
            "{found:?}"
        );
    }

    #[test]
    fn a_file_named_by_a_path_string_is_doubted_not_declared_dead() {
        // How a framework registers a file it loads itself: by path, with no
        // extension, relative to a directory only the runtime knows.
        let files = [
            ("package.json", r#"{ "main": "./src/index.ts" }"#),
            (
                "src/index.ts",
                "import { resolve } from 'node:path';\n\
                 export const handler = resolve(process.cwd(), 'runtime/handlers/island');\n",
            ),
            (
                "src/runtime/handlers/island.ts",
                "export default function island() {\n  return 'rendered';\n}\n",
            ),
            (
                "src/runtime/handlers/retired.ts",
                "export default function retired() {\n  return 'gone';\n}\n",
            ),
        ];
        let unused_files =
            |config: BastaConfig| -> Vec<(String, u8, Vec<cpd_core::deadcode::Reason>)> {
                let (report, root) = scan("path-string", &files, config);
                cleanup(&root);
                report
                    .findings
                    .into_iter()
                    .filter(|f| f.category == Category::UnusedFile)
                    .map(|f| (f.path, f.confidence, f.reasons))
                    .collect()
            };

        let by_default = unused_files(BastaConfig::default());
        assert_eq!(
            by_default.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
            ["src/runtime/handlers/retired.ts"],
            "the file a string names is below the default floor: {by_default:?}"
        );

        let everything = unused_files(everything());
        let island = everything
            .iter()
            .find(|f| f.0.ends_with("island.ts"))
            .expect("still reported on request, with the reason");
        assert_eq!(island.1, 55);
        assert_eq!(island.2, [cpd_core::deadcode::Reason::PathAppearsInString]);
    }
}
