//! The dead-code analysis: basta, the engine behind `--dead-code`, over a
//! project's files on disk. The import graph spans the project, so a save
//! can change what is unused anywhere; the analysis runs in the background
//! after the save, and its findings replace the project's last ones.

use super::findings::{Finding, Snapshot, Text};
use super::position::Encoding;
use super::project::Project;
use super::settings::Analysis;
use cpd_core::deadcode::{Category, Finding as DeadFinding};
use lsp_types::DiagnosticSeverity;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// basta's configuration for `project`, or `None` when the project has no
/// file basta reads (JavaScript, TypeScript, Python and their components)
/// or its dead-code options are wrong; the errors and warnings of the
/// options come along, for the editor to show. Rust is left to
/// rust-analyzer.
pub fn config_of(project: &Project) -> (Option<basta::config::BastaConfig>, Vec<String>) {
    let mut notes = Vec::new();
    let config = crate::dead_code::config_noting(
        &project.cli,
        &project.options,
        &project.options.paths,
        false,
        false,
        &mut notes,
    );
    (config.ok().flatten(), notes)
}

/// What a run found: the findings by file, and the files as the run read
/// them.
pub struct Run {
    pub findings: Vec<(PathBuf, DeadFinding)>,
    pub snapshots: HashMap<PathBuf, Snapshot>,
}

/// Run basta and resolve each finding to its file.
pub fn run(config: &basta::config::BastaConfig) -> Run {
    let started = std::time::SystemTime::now();
    let result = basta::analyze::run(config);
    let graph = &result.graph;
    // A report names a file relative to its root, and two roots can each
    // have a `src/util.js`; the graph knows each module's real path.
    let mut modules: HashMap<&str, Vec<&basta::model::Module>> = HashMap::new();
    for module in &graph.modules {
        modules
            .entry(module.path.as_str())
            .or_default()
            .push(module);
    }
    let mut files_taken: HashSet<basta::model::ModuleId> = HashSet::new();
    let findings: Vec<(PathBuf, DeadFinding)> = result
        .report
        .findings
        .into_iter()
        .filter_map(|finding| {
            let candidates = modules.get(finding.path.as_str())?;
            let module = match candidates.as_slice() {
                [only] => *only,
                many => *many.iter().find(|module| match finding.category {
                    // One unused-file finding per module, in order.
                    Category::UnusedFile => files_taken.insert(module.id),
                    _ => graph.symbols_of(module.id).iter().any(|symbol| {
                        symbol.name == finding.name && symbol.start.offset == finding.start.offset
                    }),
                })?,
            };
            let path = std::fs::canonicalize(&module.real_path)
                .unwrap_or_else(|_| module.real_path.clone());
            Some((path, finding))
        })
        .collect();
    let snapshots = findings
        .iter()
        .map(|(path, _)| path)
        .collect::<HashSet<_>>()
        .into_iter()
        .filter_map(|path| Some((path.clone(), Snapshot::of_disk(path, started)?)))
        .collect();
    Run {
        findings,
        snapshots,
    }
}

/// The findings of the file at `path`, with `text`: none while the text
/// differs from what the last run read, since their offsets would land on
/// other code.
pub fn dead_code_findings(
    path: &Path,
    text: &Text,
    project: &Project,
    encoding: Encoding,
) -> Vec<Finding> {
    let Some(snapshot) = project
        .dead_code_snapshots
        .get(path)
        .filter(|snapshot| snapshot.fits(text))
    else {
        return Vec::new();
    };
    project
        .dead_code
        .iter()
        .filter(|(file, _)| file == path)
        .map(|(_, finding)| {
            let range = match finding.category {
                // The file as a whole: its first line.
                Category::UnusedFile => {
                    let end = text
                        .index
                        .line_start(1)
                        .map_or(text.text.len(), |n| n.saturating_sub(1));
                    text.index.range(&text.text, 0, end, encoding)
                }
                _ => text.scan_range(
                    finding.start.offset as usize,
                    (finding.end.offset as usize).max(finding.start.offset as usize),
                    snapshot.bom,
                    encoding,
                ),
            };
            Finding {
                analysis: Analysis::DeadCode,
                rule: finding.category.as_str(),
                range,
                severity: DiagnosticSeverity::HINT,
                message: format!("{} ({}%)", finding.message, finding.confidence),
                targets: Vec::new(),
                format: finding.language.clone(),
                first_line: range.start.line,
                last_line: range.end.line,
                unnecessary: true,
                notes: finding
                    .reasons
                    .iter()
                    .map(|r| r.explain().to_string())
                    .collect(),
                unit: Default::default(),
            }
        })
        .collect()
}
