//! What the server tells an editor about one file: its findings, turned into
//! diagnostics, hovers and code actions.

use super::position::{Encoding, LineIndex, path_to_uri};
use super::settings::{Analyses, Analysis};
use crate::index::host_file;
use cpd_core::models::{CpdClone, Fragment};
use cpd_reporter::rules::{SEMANTIC, SIMILAR_FUNCTION, rule_id};
use cpd_similarity::UnitKind;
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Uri,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// One finding in a file: a place, what is wrong with it, and where the
/// other copies are.
#[derive(Debug, Clone, PartialEq)]
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
    /// Code nothing runs: editors fade it rather than underline it.
    pub unnecessary: bool,
    /// More to say in a hover, such as why a dead-code finding is not
    /// certain.
    pub notes: Vec<String>,
    /// What the code of a `--similarity` or `--semantic` finding is: a
    /// function, a class.
    pub unit: UnitKind,
}

/// Another copy of a finding.
#[derive(Debug, Clone, PartialEq)]
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
    /// The length of the byte-order mark the file on disk starts with and
    /// this text leaves out, as editors do: offsets from a scan of the disk
    /// count it.
    pub bom: usize,
    hash: std::cell::OnceCell<u64>,
}

impl Text {
    /// The text of an editor's buffer.
    pub fn new(text: String) -> Self {
        let index = LineIndex::new(&text);
        Self {
            text,
            index,
            bom: 0,
            hash: std::cell::OnceCell::new(),
        }
    }

    /// The text of a file as read from the disk.
    pub fn from_disk(mut text: String) -> Self {
        let bom = match text.starts_with('\u{feff}') {
            true => '\u{feff}'.len_utf8(),
            false => 0,
        };
        text.drain(..bom);
        Self {
            bom,
            ..Self::new(text)
        }
    }

    /// A hash of the text, to tell whether a finding computed from the
    /// disk still fits it.
    pub fn hash(&self) -> u64 {
        *self.hash.get_or_init(|| text_hash(&self.text))
    }

    /// The range between two byte offsets of a scan whose copy of the file
    /// started with `bom` bytes of a byte-order mark.
    pub fn scan_range(&self, start: usize, end: usize, bom: usize, encoding: Encoding) -> Range {
        self.index.range(
            &self.text,
            start.saturating_sub(bom),
            end.saturating_sub(bom),
            encoding,
        )
    }
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// What a background analysis read of one file: a hash of its text, without
/// a byte-order mark, and the length of that mark. Its findings fit the file
/// only while the text in the editor, or on disk, has the same hash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub hash: u64,
    pub bom: usize,
}

impl Snapshot {
    /// The file at `path` as a run that started at `started` read it: the
    /// file as it is on disk now, when nothing wrote it since the start.
    /// A file written during the run may have been read before or after
    /// the write, so it has no snapshot, and its findings wait for the next
    /// run.
    pub fn of_disk(path: &Path, started: std::time::SystemTime) -> Option<Self> {
        let modified = std::fs::metadata(path).ok()?.modified().ok()?;
        if modified > started {
            return None;
        }
        let text = Text::from_disk(std::fs::read_to_string(path).ok()?);
        Some(Self {
            hash: text.hash(),
            bom: text.bom,
        })
    }

    /// Whether a finding computed from this snapshot fits `text`.
    pub fn fits(&self, text: &Text) -> bool {
        self.hash == text.hash()
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
/// `text`, among `clones`. `other_text` gives the text of another file, for
/// the ranges of the other copies. Clones of the index count their offsets
/// in the texts the server holds; clones a background run found on disk
/// come with the `snapshots` of the files it read, and one whose file no
/// longer fits its snapshot is left out until the next run.
pub fn clone_findings<'c>(
    path: &Path,
    text: &Text,
    clones: impl Iterator<Item = &'c CpdClone>,
    scope: &Scope,
    other_text: &mut dyn FnMut(&Path) -> Option<std::rc::Rc<Text>>,
    snapshots: Option<&HashMap<PathBuf, Snapshot>>,
) -> Vec<Finding> {
    let id = path.to_string_lossy();
    // The byte-order mark to take off the offsets of a file, or `None` when
    // the clone no longer fits it.
    let shift = |file: &Path, text: &Text| -> Option<usize> {
        match snapshots {
            None => Some(text.bom),
            Some(snapshots) => snapshots
                .get(file)
                .filter(|snapshot| snapshot.fits(text))
                .map(|snapshot| snapshot.bom),
        }
    };
    let Some(here_bom) = shift(path, text) else {
        return Vec::new();
    };
    let mut findings: Vec<Finding> = Vec::new();
    for clone in clones {
        let rule = rule_id(clone);
        let analysis = match rule {
            SIMILAR_FUNCTION => Analysis::Ast,
            SEMANTIC => Analysis::Semantic,
            _ => Analysis::Clones,
        };
        if !scope.analyses.has(analysis) {
            continue;
        }
        let a = host_file(&clone.fragment_a.source_id);
        let b = host_file(&clone.fragment_b.source_id);
        for (here, there, there_id) in [
            (&clone.fragment_a, &clone.fragment_b, b),
            (&clone.fragment_b, &clone.fragment_a, a),
        ] {
            let here_id = host_file(&here.source_id);
            if here_id != id {
                continue;
            }
            let there_path = PathBuf::from(there_id);
            let target = match there_id == id {
                true => target(&there_path, text, here_bom, there, scope),
                false => other_text(&there_path).and_then(|other| {
                    let bom = shift(&there_path, &other)?;
                    target(&there_path, &other, bom, there, scope)
                }),
            };
            let Some(target) = target else { continue };
            findings.push(finding(
                clone, rule, analysis, here, text, here_bom, target, scope,
            ));
        }
    }
    merge(findings)
}

#[allow(clippy::too_many_arguments)]
fn finding(
    clone: &CpdClone,
    rule: &'static str,
    analysis: Analysis,
    here: &Fragment,
    text: &Text,
    bom: usize,
    target: Target,
    scope: &Scope,
) -> Finding {
    let encoding = scope.encoding;
    let start = (here.start.offset as usize).saturating_sub(bom);
    let end = (here.end.offset as usize).saturating_sub(bom);
    let first_line = here.start.line.saturating_sub(1);
    let last_line = here.end.line.saturating_sub(1);
    let unit = clone.unit.unwrap_or_default();
    let (range, severity, message) = match analysis {
        // A function or a class: its first line, not its whole body.
        Analysis::Ast | Analysis::Semantic => {
            let line_end = text
                .index
                .line_start(first_line as usize + 1)
                .map_or(text.text.len(), |next| next.saturating_sub(1));
            (
                text.index
                    .range(&text.text, start, line_end.max(start), encoding),
                DiagnosticSeverity::INFORMATION,
                format!(
                    "{} the {} at {}{}",
                    match analysis {
                        Analysis::Semantic => "Does the same job as",
                        _ => "Same structure as",
                    },
                    unit.noun(),
                    target.label,
                    clone
                        .similarity_rounded()
                        .map_or(String::new(), |s| format!(" ({s:.2})"))
                ),
            )
        }
        _ => (
            text.index.range(&text.text, start, end, encoding),
            // A warning, as in SARIF; `warningTokens` keeps the warnings for
            // the big clones and lowers the rest to information.
            match scope.analyses.warning_tokens {
                Some(limit) if clone.token_count < limit => DiagnosticSeverity::INFORMATION,
                _ => DiagnosticSeverity::WARNING,
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
        unnecessary: false,
        notes: Vec::new(),
        unit,
    }
}

/// The other copy of a finding, in the file `path` with `text`.
fn target(
    path: &Path,
    text: &Text,
    bom: usize,
    fragment: &Fragment,
    scope: &Scope,
) -> Option<Target> {
    let uri = path_to_uri(path)?;
    let range = text.scan_range(
        fragment.start.offset as usize,
        fragment.end.offset as usize,
        bom,
        scope.encoding,
    );
    let label = format!(
        "{}:{}-{}",
        label_path(path, scope.roots),
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

/// `path` as a message names it: relative to its root, after the name of
/// the root when a project has several, so `app/src/util.js` and
/// `lib/src/util.js` stay apart.
pub fn label_path(path: &Path, roots: &[PathBuf]) -> String {
    let (root, relative) = roots
        .iter()
        .find_map(|root| Some((root, path.strip_prefix(root).ok()?)))
        .map_or((None, path), |(root, relative)| (Some(root), relative));
    let relative = relative.to_string_lossy().replace('\\', "/");
    match (roots.len() > 1, root.and_then(|root| root.file_name())) {
        (true, Some(name)) => format!("{}/{relative}", name.to_string_lossy()),
        _ => relative,
    }
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
                let (count, places) = (existing.targets.len(), labels(&existing.targets));
                let units = existing.unit.plural();
                existing.message = match existing.analysis {
                    Analysis::Semantic => format!("Does the same job as {count} {units}: {places}"),
                    Analysis::Ast => format!("Same structure as {count} {units}: {places}"),
                    _ => format!("Duplicated in {count} places: {places}"),
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
        tags: finding
            .unnecessary
            .then(|| vec![lsp_types::DiagnosticTag::UNNECESSARY]),
        related_information: related.then(|| {
            finding
                .targets
                .iter()
                .map(|t| DiagnosticRelatedInformation {
                    location: Location::new(t.uri.clone(), t.range),
                    message: match finding.analysis {
                        Analysis::Ast => format!("The similar {}", finding.unit.noun()),
                        Analysis::Semantic => {
                            format!("The {} that does the same job", finding.unit.noun())
                        }
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
/// The block comment of a format, for a marker beside code on its line;
/// `None` for formats without one.
pub fn block_comment_syntax(format: &str) -> Option<(&'static str, &'static str)> {
    Some(match format {
        "actionscript" | "apex" | "arduino" | "bicep" | "c" | "c-header" | "cfscript" | "clike"
        | "cpp" | "cpp-header" | "csharp" | "css" | "d" | "dart" | "flow" | "glsl" | "gml"
        | "go" | "groovy" | "haxe" | "hlsl" | "java" | "javascript" | "jolie" | "json5" | "jsx"
        | "kotlin" | "less" | "n4js" | "objectivec" | "odin" | "opencl" | "openqasm" | "php"
        | "processing" | "protobuf" | "reason" | "rescript" | "rust" | "sass" | "scala"
        | "scss" | "solidity" | "stylus" | "swift" | "tsx" | "typescript" | "vala" | "verilog"
        | "wgsl" => ("/* ", " */"),
        "markdown" | "markup" | "vue" | "svelte" | "astro" => ("<!-- ", " -->"),
        _ => return None,
    })
}

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
