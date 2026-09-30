//! What the server tells an editor about one file: its findings, turned into
//! diagnostics, hovers and code actions.

use super::index::{ScanIndex, host_file};
use super::position::{Encoding, LineIndex, path_to_uri};
use super::settings::{Analyses, Analysis};
use cpd_core::models::{CpdClone, Fragment};
use cpd_reporter::rules::{SIMILAR_FUNCTION, rule_id};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Uri,
};
use std::path::{Path, PathBuf};

/// One finding in a file: a place, what is wrong with it, and where the
/// other copies are.
#[derive(Debug, Clone)]
pub struct Finding {
    pub analysis: Analysis,
    pub rule: &'static str,
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub targets: Vec<Target>,
    /// The format of the code, for comment syntax and code fences.
    pub format: String,
    /// The lines of the finding, from 0.
    pub first_line: u32,
    pub last_line: u32,
}

/// Another copy of a finding.
#[derive(Debug, Clone)]
pub struct Target {
    pub uri: Uri,
    pub path: PathBuf,
    pub range: Range,
    /// `src/b.ts:10-25`, relative to the project.
    pub label: String,
}

/// A text and its line index.
pub struct Text {
    pub text: String,
    pub index: LineIndex,
}

impl Text {
    pub fn new(text: String) -> Self {
        let index = LineIndex::new(&text);
        Self { text, index }
    }
}

/// What the findings of one project are made with: its analyses, its roots
/// (the other copies are labelled relative to them) and the position
/// encoding of the client.
pub struct Scope<'a> {
    pub analyses: &'a Analyses,
    pub roots: &'a [PathBuf],
    pub encoding: Encoding,
}

/// The findings of the clone analyses in the file `path`, whose text is
/// `text`, from `index`. `other_text` gives the text of another file, for
/// the ranges of the other copies.
pub fn clone_findings(
    path: &Path,
    text: &Text,
    index: &ScanIndex,
    scope: &Scope,
    other_text: &mut dyn FnMut(&Path) -> Option<std::rc::Rc<Text>>,
) -> Vec<Finding> {
    let id = path.to_string_lossy();
    let mut findings: Vec<Finding> = Vec::new();
    for clone in index.clones() {
        let rule = rule_id(clone);
        let analysis = match rule {
            SIMILAR_FUNCTION => Analysis::Ast,
            _ => Analysis::Clones,
        };
        if !scope.analyses.has(analysis) {
            continue;
        }
        let a = host_file(&clone.fragment_a.source_id, &clone.format);
        let b = host_file(&clone.fragment_b.source_id, &clone.format);
        for (here, there, there_id) in [
            (&clone.fragment_a, &clone.fragment_b, b),
            (&clone.fragment_b, &clone.fragment_a, a),
        ] {
            let here_id = host_file(&here.source_id, &clone.format);
            if here_id != id {
                continue;
            }
            let there_path = PathBuf::from(there_id);
            let target = match there_id == id {
                true => target(&there_path, text, there, scope),
                false => other_text(&there_path)
                    .and_then(|other| target(&there_path, &other, there, scope)),
            };
            let Some(target) = target else { continue };
            findings.push(finding(clone, rule, analysis, here, text, target, scope));
        }
    }
    merge(findings)
}

fn finding(
    clone: &CpdClone,
    rule: &'static str,
    analysis: Analysis,
    here: &Fragment,
    text: &Text,
    target: Target,
    scope: &Scope,
) -> Finding {
    let encoding = scope.encoding;
    let start = here.start.offset as usize;
    let end = here.end.offset as usize;
    let first_line = here.start.line.saturating_sub(1);
    let last_line = here.end.line.saturating_sub(1);
    let (range, severity, message) = match analysis {
        // A function: its first line, not its whole body.
        Analysis::Ast => {
            let line_end = text
                .index
                .line_start(first_line as usize + 1)
                .map_or(text.text.len(), |next| next.saturating_sub(1));
            (
                text.index
                    .range(&text.text, start, line_end.max(start), encoding),
                DiagnosticSeverity::INFORMATION,
                format!(
                    "Same structure as the function at {}{}",
                    target.label,
                    clone
                        .similarity_rounded()
                        .map_or(String::new(), |s| format!(" ({s:.2})"))
                ),
            )
        }
        _ => (
            text.index.range(&text.text, start, end, encoding),
            match scope.analyses.warning_tokens {
                Some(limit) if clone.token_count >= limit => DiagnosticSeverity::WARNING,
                _ => DiagnosticSeverity::INFORMATION,
            },
            format!(
                "Duplicated in {} ({} tokens)",
                target.label, clone.token_count
            ),
        ),
    };
    Finding {
        analysis,
        rule,
        range,
        severity,
        message,
        targets: vec![target],
        format: clone.format.clone(),
        first_line,
        last_line,
    }
}

/// The other copy of a finding, in the file `path` with `text`.
fn target(path: &Path, text: &Text, fragment: &Fragment, scope: &Scope) -> Option<Target> {
    let uri = path_to_uri(path)?;
    let range = text.index.range(
        &text.text,
        fragment.start.offset as usize,
        fragment.end.offset as usize,
        scope.encoding,
    );
    let relative = scope
        .roots
        .iter()
        .find_map(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let label = format!(
        "{}:{}-{}",
        relative.to_string_lossy().replace('\\', "/"),
        fragment.start.line,
        fragment.end.line
    );
    Some(Target {
        uri,
        path: path.to_path_buf(),
        range,
        label,
    })
}

/// One finding per place: the copies of a block copied three times make one
/// finding with two targets.
fn merge(findings: Vec<Finding>) -> Vec<Finding> {
    let mut merged: Vec<Finding> = Vec::new();
    for finding in findings {
        match merged
            .iter_mut()
            .find(|m| m.rule == finding.rule && m.range == finding.range)
        {
            Some(existing) => {
                existing.targets.extend(finding.targets);
                existing.severity = existing.severity.min(finding.severity);
                existing.message = match existing.analysis {
                    Analysis::Ast => format!(
                        "Same structure as {} functions: {}",
                        existing.targets.len(),
                        labels(&existing.targets)
                    ),
                    _ => format!(
                        "Duplicated in {} places: {}",
                        existing.targets.len(),
                        labels(&existing.targets)
                    ),
                };
            }
            None => merged.push(finding),
        }
    }
    merged.sort_by_key(|f| (f.range.start.line, f.range.start.character));
    merged
}

fn labels(targets: &[Target]) -> String {
    let shown: Vec<&str> = targets.iter().take(3).map(|t| t.label.as_str()).collect();
    match targets.len() > 3 {
        true => format!("{} and {} more", shown.join(", "), targets.len() - 3),
        false => shown.join(", "),
    }
}

/// The diagnostic of a finding. `related` is whether the client shows
/// related information.
pub fn diagnostic(finding: &Finding, related: bool) -> Diagnostic {
    Diagnostic {
        range: finding.range,
        severity: Some(finding.severity),
        code: Some(NumberOrString::String(finding.rule.to_string())),
        source: Some("jscpd".to_string()),
        message: finding.message.clone(),
        related_information: related.then(|| {
            finding
                .targets
                .iter()
                .map(|t| DiagnosticRelatedInformation {
                    location: Location::new(t.uri.clone(), t.range),
                    message: match finding.analysis {
                        Analysis::Ast => "The similar function".to_string(),
                        _ => "The other copy".to_string(),
                    },
                })
                .collect()
        }),
        ..Diagnostic::default()
    }
}

/// How a line comment opens and closes in `format`, for the markers of
/// "Ignore this clone"; `None` for a format without comments we know.
pub fn comment_syntax(format: &str) -> Option<(&'static str, &'static str)> {
    Some(match format {
        "actionscript" | "apex" | "arduino" | "bicep" | "c" | "c-header" | "cfscript" | "clike"
        | "cpp" | "cpp-header" | "csharp" | "d" | "dart" | "flow" | "fsharp" | "glsl" | "gml"
        | "go" | "groovy" | "haxe" | "hlsl" | "java" | "javascript" | "jolie" | "json5" | "jsx"
        | "kotlin" | "kusto" | "less" | "n4js" | "objectivec" | "odin" | "opencl" | "openqasm"
        | "pascal" | "php" | "processing" | "protobuf" | "qsharp" | "reason" | "rescript"
        | "rust" | "sass" | "scala" | "scss" | "solidity" | "stylus" | "swift" | "tsx"
        | "typescript" | "vala" | "verilog" | "wgsl" | "zig" => ("// ", ""),
        "awk" | "bash" | "cmake" | "coffeescript" | "crystal" | "docker" | "elixir"
        | "gdscript" | "graphql" | "hcl" | "julia" | "makefile" | "nginx" | "nim" | "nix"
        | "perl" | "powershell" | "puppet" | "python" | "r" | "ruby" | "tcl" | "toml" | "yaml" => {
            ("# ", "")
        }
        "ada" | "agda" | "applescript" | "elm" | "haskell" | "idris" | "lua" | "plsql"
        | "purescript" | "sql" | "vhdl" => ("-- ", ""),
        "asm6502" | "armasm" | "autohotkey" | "autoit" | "clojure" | "ini" | "lisp" | "llvm"
        | "nasm" | "racket" | "scheme" => ("; ", ""),
        "erlang" | "latex" | "matlab" | "oz" | "prolog" => ("% ", ""),
        "basic" | "vbnet" | "visual-basic" => ("' ", ""),
        "css" => ("/* ", " */"),
        "markdown" | "markup" | "vue" | "svelte" | "astro" => ("<!-- ", " -->"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_with_a_line_comment_has_its_syntax() {
        assert_eq!(comment_syntax("typescript"), Some(("// ", "")));
        assert_eq!(comment_syntax("python"), Some(("# ", "")));
        assert_eq!(comment_syntax("css"), Some(("/* ", " */")));
        assert_eq!(comment_syntax("csv"), None);
        for format in cpd_tokenizer::formats::list_formats() {
            if let Some((open, close)) = comment_syntax(format) {
                assert!(!open.is_empty(), "{format}");
                assert!(close.is_empty() || close.starts_with(' '), "{format}");
            }
        }
    }
}
