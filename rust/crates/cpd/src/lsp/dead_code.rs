//! The dead-code analysis: basta, the engine behind `--dead-code`, over a
//! project's files on disk. The import graph spans the project, so a save
//! can change what is unused anywhere; the analysis runs in the background
//! after the save, and its findings replace the project's last ones.

use super::findings::{Finding, Text};
use super::position::Encoding;
use super::project::Project;
use super::settings::Analysis;
use cpd_core::deadcode::{Category, Finding as DeadFinding};
use lsp_types::DiagnosticSeverity;
use std::path::{Path, PathBuf};

/// basta's configuration for `project`, or `None` when the project has no
/// file basta reads (JavaScript, TypeScript, Python and their components).
pub fn config_of(project: &Project) -> Option<basta::config::BastaConfig> {
    crate::dead_code::config(
        &project.cli,
        &project.options,
        &project.options.paths,
        false,
    )
    .ok()
    .flatten()
}

/// Run basta and resolve each finding to its file. Findings in the folders
/// of other projects are left to those projects.
pub fn run(config: &basta::config::BastaConfig) -> Vec<(PathBuf, DeadFinding)> {
    let result = basta::analyze::run(config);
    result
        .report
        .findings
        .into_iter()
        .filter_map(|finding| {
            // Paths are relative to the scan root they were found under.
            let path = config
                .paths
                .iter()
                .map(|root| root.join(&finding.path))
                .find(|path| path.exists())?;
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            Some((path, finding))
        })
        .collect()
}

/// The findings of the file at `path`, with `text`.
pub fn dead_code_findings(
    path: &Path,
    text: &Text,
    project: &Project,
    encoding: Encoding,
) -> Vec<Finding> {
    project
        .dead_code
        .iter()
        .filter(|(file, _)| file == path)
        .map(|(_, finding)| {
            let (start, end) = match finding.category {
                // The file as a whole: its first line.
                Category::UnusedFile => (
                    0,
                    text.index
                        .line_start(1)
                        .map_or(text.text.len(), |n| n.saturating_sub(1)),
                ),
                _ => (finding.start.offset as usize, finding.end.offset as usize),
            };
            let range = text
                .index
                .range(&text.text, start, end.max(start), encoding);
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
            }
        })
        .collect()
}
