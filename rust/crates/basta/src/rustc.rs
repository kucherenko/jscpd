//! Rust dead code, read from the compiler.
//!
//! rustc already knows what is never used: every `cargo build` prints
//! `function `x` is never used`. That analysis has real name resolution,
//! trait dispatch and macro expansion behind it, which no scan of the
//! source could match, so basta does not try. It reads the compiler's
//! verdicts instead, from `cargo check --message-format=json` (or `build`,
//! `clippy`, `test --no-run`), and turns them into findings that go through
//! the same reporters, categories and confidence floor as every other
//! language.
//!
//! basta never runs cargo itself. Running it would execute the scanned
//! project's build scripts and procedural macros, and everything else basta
//! does is safe to point at a repository nobody has read. The diagnostics
//! are piped in or read from a file, which also lets CI reuse a check it
//! already runs:
//!
//! ```text
//! cargo check --all-targets --message-format=json | basta . --rust-diagnostics -
//! ```
//!
//! What the compiler does not say, this module does not invent. A `pub`
//! item of a library crate is never reported (it may be used by a crate
//! outside the workspace), and code that only exists under a feature or a
//! target the check did not enable is reported as dead because, for that
//! build, it is. Every finding says which lint it came from.

use crate::finding::{CategoryCount, Finding, Report, Stats};
use cpd_core::deadcode::{Category, SymbolKind};
use cpd_core::models::Location;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The lints that mean "nothing uses this". Anything else the compiler
/// prints — type errors, style lints — is not dead code and is skipped.
const DEAD_CODE_LINTS: &[&str] = &["dead_code", "unused_imports"];

/// Read every dead-code diagnostic in `text`, one cargo JSON message per
/// line, into findings under `roots`.
///
/// cargo writes each crate's `manifest_path` absolute. A relative one — a
/// file edited by hand, or an artifact copied between machines — is taken
/// relative to `base`: the directory the diagnostics were read from, or,
/// for a pipe, the scan root (a pipe has no directory of its own, and the
/// crate being scanned is the only thing such a path can mean).
///
/// Lines that are not JSON (cargo's own progress output, a stray warning)
/// are skipped: `cargo check 2>&1 | basta …` must not fail on them.
pub fn findings(text: &str, roots: &[PathBuf], base: &Path, include_tests: bool) -> Vec<Finding> {
    let mut sources: FxHashMap<PathBuf, Option<String>> = FxHashMap::default();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(message) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };
        if message.reason != "compiler-message" {
            continue;
        }
        let Some(finding) = finding_of(&message, roots, base, include_tests, &mut sources) else {
            continue;
        };
        out.push(finding);
    }
    // One diagnostic per item, whatever `--all-targets` did: the same
    // function is checked once for the library and once for its tests.
    out.sort_by(|a, b| {
        (&a.path, a.start.line, a.start.column, &a.name).cmp(&(
            &b.path,
            b.start.line,
            b.start.column,
            &b.name,
        ))
    });
    out.dedup_by(|a, b| a.path == b.path && a.start == b.start && a.name == b.name);
    out
}

/// The Rust files under `roots` and their lines together, for the report's
/// counts. Walked with the same ignore rules as a scan.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Sources {
    pub files: u32,
    pub lines: u32,
}

/// Fold compiler findings into a report basta produced for the other
/// languages of the same tree.
pub fn merge(mut report: Report, findings: Vec<Finding>, rust: Sources) -> Report {
    let total_lines = report.statistics.total_lines + rust.lines;
    report.findings.extend(findings);
    report.findings.sort_by(|a, b| {
        (a.category != Category::UnusedFile)
            .cmp(&(b.category != Category::UnusedFile))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.start.line.cmp(&b.start.line))
            .then_with(|| a.name.cmp(&b.name))
    });
    let detection_date = report.statistics.detection_date.clone();
    let files = report.statistics.files + rust.files;
    let unparsed = report.statistics.unparsed;
    let unparsed_files = std::mem::take(&mut report.statistics.unparsed_files);
    let reachable_files = report.statistics.reachable_files;
    let symbols = report.statistics.symbols;
    let entry_points = report.statistics.entry_points;
    let mut statistics = stats(&report.findings, total_lines);
    statistics.detection_date = detection_date;
    statistics.files = files;
    statistics.unparsed = unparsed;
    statistics.unparsed_files = unparsed_files;
    statistics.reachable_files = reachable_files;
    statistics.symbols = symbols;
    statistics.entry_points = entry_points;
    report.statistics = statistics;
    report
}

/// What the compiler's findings are a share of.
pub fn sources(roots: &[PathBuf], no_gitignore: bool) -> Sources {
    let mut found = Sources::default();
    for root in roots {
        let mut walk = ignore::WalkBuilder::new(root);
        walk.git_ignore(!no_gitignore).filter_entry(|entry| {
            !(entry.file_type().is_some_and(|t| t.is_dir())
                && matches!(
                    entry.file_name().to_string_lossy().as_ref(),
                    ".git" | "target" | "node_modules"
                ))
        });
        for entry in walk.build().flatten() {
            let path = entry.path();
            if entry.file_type().is_some_and(|t| t.is_file())
                && path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(path)
            {
                found.files += 1;
                found.lines += text.lines().count() as u32;
            }
        }
    }
    found
}

fn stats(findings: &[Finding], total_lines: u32) -> Stats {
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
    let dead_lines = by_category
        .iter()
        .map(|c| c.lines)
        .sum::<u32>()
        .min(total_lines);
    Stats {
        files: 0,
        unparsed: 0,
        unparsed_files: Vec::new(),
        reachable_files: 0,
        symbols: 0,
        entry_points: 0,
        by_category,
        dead_lines,
        total_lines,
        percentage: if total_lines == 0 {
            0.0
        } else {
            (f64::from(dead_lines) / f64::from(total_lines)) * 100.0
        },
        detection_date: crate::analyze::now_iso8601(),
    }
}

// ── cargo's JSON ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    /// The crate's `Cargo.toml`. In a workspace the spans' paths are
    /// relative to the workspace root cargo ran in; in a single crate, to
    /// this directory. Both are tried.
    #[serde(default)]
    manifest_path: Option<PathBuf>,
    #[serde(default)]
    target: Option<Target>,
    #[serde(default)]
    message: Option<Diagnostic>,
}

/// The build target the diagnostic came from: a `["test"]` kind is the
/// crate's test harness, which is how the compiler says the code is test
/// code without basta having to guess from the path.
#[derive(Deserialize, Default)]
struct Target {
    #[serde(default)]
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct Diagnostic {
    message: String,
    #[serde(default)]
    code: Option<Code>,
    #[serde(default)]
    spans: Vec<Span>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: u32,
    column_start: u32,
    is_primary: bool,
    /// Set when the diagnostic points into a macro expansion rather than a
    /// line a person wrote.
    #[serde(default)]
    expansion: Option<serde_json::Value>,
}

fn finding_of(
    message: &CargoMessage,
    roots: &[PathBuf],
    base: &Path,
    include_tests: bool,
    sources: &mut FxHashMap<PathBuf, Option<String>>,
) -> Option<Finding> {
    let diagnostic = message.message.as_ref()?;
    // Test code, by the compiler's own account: the test harness target,
    // or a `#[cfg(test)]` module of any target. Dead code inside a test is
    // reported only on request, as for every other language, and with less
    // confidence when it is: a test helper is often kept for the next test.
    let is_test = message
        .target
        .as_ref()
        .is_some_and(|t| t.kind.iter().any(|k| k == "test"));
    if is_test && !include_tests {
        return None;
    }
    let lint = diagnostic.code.as_ref()?.code.as_str();
    if !DEAD_CODE_LINTS.contains(&lint) {
        return None;
    }
    let span = diagnostic.spans.iter().find(|s| s.is_primary)?;
    // A span inside a macro expansion names generated code; a person cannot
    // delete that line, and the item it belongs to is reported on its own.
    if span.expansion.is_some() {
        return None;
    }
    let (kind, name) = parse_message(&diagnostic.message)?;
    let category = match kind {
        SymbolKind::Import => Category::UnusedImport,
        SymbolKind::Method | SymbolKind::Property | SymbolKind::EnumMember => {
            Category::UnusedMember
        }
        _ => Category::UnusedSymbol,
    };

    let manifest_dir = message
        .manifest_path
        .as_deref()
        .and_then(Path::parent)
        .map(|dir| base.join(dir))
        .unwrap_or_else(|| base.to_path_buf());
    // cargo writes a span's path relative to where it ran: the workspace
    // root for a workspace, the crate for a single crate. The manifest's
    // directory is tried first, then its ancestors, and the first that
    // holds the file wins — without this a workspace crate's findings
    // pointed at `arena-core/arena-core/tests/…`, a file that does not
    // exist.
    let absolute = std::iter::once(manifest_dir.as_path())
        .chain(manifest_dir.ancestors().skip(1))
        .map(|dir| dir.join(&span.file_name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| manifest_dir.join(&span.file_name));
    let absolute = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    // Only the files of the scan: a workspace's diagnostics may cover more
    // crates than `basta packages/one` was asked about.
    if !roots.is_empty() && !roots.iter().any(|root| absolute.starts_with(root)) {
        return None;
    }
    let path = roots
        .iter()
        .find_map(|root| absolute.strip_prefix(root).ok())
        .unwrap_or(&absolute)
        .to_string_lossy()
        .replace('\\', "/");

    // The span covers the name; the finding is the whole item. Its extent is
    // read from the file, which basta has and the compiler did not send.
    let source = sources
        .entry(absolute.clone())
        .or_insert_with(|| std::fs::read_to_string(&absolute).ok());
    let (end_line, end_column) = source
        .as_deref()
        .map(|text| item_end(text, span.line_start, kind))
        .unwrap_or((span.line_start, span.column_start));
    let lines = end_line.saturating_sub(span.line_start) + 1;

    let noun = kind.noun();
    let message = match category {
        Category::UnusedImport => format!(
            "{path}:{} imports `{name}` and never uses it",
            span.line_start
        ),
        _ => format!("{noun} `{name}` in {path} is never used (rustc `{lint}`)"),
    };
    Some(Finding {
        category,
        path,
        name,
        exported_as: None,
        symbol_kind: Some(kind),
        parent: None,
        language: "rust".to_string(),
        start: Location {
            line: span.line_start,
            column: span.column_start.saturating_sub(1),
            offset: 0,
        },
        end: Location {
            line: end_line,
            column: end_column,
            offset: 0,
        },
        lines,
        // The compiler resolved every name. What it cannot see — a feature
        // or a target the check did not build — is on the reader to know,
        // and the message names the lint so that they do.
        confidence: if is_test { 85 } else { 100 },
        reasons: if is_test {
            vec![cpd_core::deadcode::Reason::InTestFile]
        } else {
            Vec::new()
        },
        message,
    })
}

/// `function `never_called` is never used` → the kind and the name.
///
/// The compiler's wording is stable across releases for these lints; a
/// message this does not recognise is skipped rather than guessed at.
fn parse_message(message: &str) -> Option<(SymbolKind, String)> {
    if let Some(rest) = message.strip_prefix("unused import: `") {
        let import = rest.strip_suffix('`')?;
        // `use a::{B, C}` reports the group; the last path segment of a
        // single import is its bound name.
        let name = import
            .rsplit("::")
            .next()
            .unwrap_or(import)
            .trim_matches(|c| c == '{' || c == '}')
            .to_string();
        return Some((SymbolKind::Import, name));
    }
    if message.starts_with("unused imports: ") {
        let list = message.trim_start_matches("unused imports: ");
        return Some((SymbolKind::Import, list.replace('`', "")));
    }
    let (head, _) = message.split_once(" is never ")?;
    let (kind, name) = head.rsplit_once(" `")?;
    let name = name.strip_suffix('`')?.to_string();
    let kind = match kind {
        "function" => SymbolKind::Function,
        "method" | "associated function" => SymbolKind::Method,
        "struct" | "union" => SymbolKind::Class,
        "enum" => SymbolKind::Enum,
        "variant" | "variants" => SymbolKind::EnumMember,
        "field" | "fields" => SymbolKind::Property,
        "trait" => SymbolKind::Interface,
        "type alias" => SymbolKind::TypeAlias,
        "constant" | "static" => SymbolKind::Variable,
        "associated constant" => SymbolKind::Property,
        _ => return None,
    };
    Some((kind, name))
}

/// The last line and column of the item whose name sits on `line`.
///
/// Rust items end at a matching `}` or at the first `;` outside braces, and
/// the compiler's span stops at the name, so this walks forward from the
/// line the name is on. An import ends at its `;`. Strings and comments are
/// skipped so a brace inside them does not close the item early.
fn item_end(text: &str, line: u32, kind: SymbolKind) -> (u32, u32) {
    // Seek to the line by counting newlines in the raw bytes: `str::lines`
    // strips a `\r`, and on a CRLF checkout that put the scan one byte
    // short per line, which cut every extent off early.
    let mut offset = 0usize;
    for _ in 1..line {
        match text[offset..].find('\n') {
            Some(at) => offset += at + 1,
            None => return (line, 0),
        }
    }
    let bytes = text.as_bytes();
    let (mut depth, mut at) = (0i32, offset);
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut block_comment = 0u32;
    while at < bytes.len() {
        let b = bytes[at];
        let next = bytes.get(at + 1).copied();
        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
        } else if block_comment > 0 {
            if b == b'*' && next == Some(b'/') {
                block_comment -= 1;
                at += 1;
            } else if b == b'/' && next == Some(b'*') {
                block_comment += 1;
                at += 1;
            }
        } else if in_string {
            if b == b'\\' {
                at += 1;
            } else if b == b'"' {
                in_string = false;
            }
        } else {
            match b {
                b'/' if next == Some(b'/') => in_line_comment = true,
                b'/' if next == Some(b'*') => {
                    block_comment = 1;
                    at += 1;
                }
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth <= 0 {
                        return position(text, at);
                    }
                }
                b';' if depth == 0 => return position(text, at),
                _ => {}
            }
        }
        at += 1;
    }
    let _ = kind;
    position(text, bytes.len().saturating_sub(1))
}

/// 1-based line and 0-based column of byte `at`.
fn position(text: &str, at: usize) -> (u32, u32) {
    let before = &text[..at.min(text.len())];
    let line = before.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
    let column = before.rsplit('\n').next().map_or(0, str::len) as u32;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scan::TempTree;

    fn message(
        manifest: &Path,
        file: &str,
        line: u32,
        column: u32,
        code: &str,
        text: &str,
    ) -> String {
        serde_json::json!({
            "reason": "compiler-message",
            "manifest_path": manifest.join("Cargo.toml"),
            "message": {
                "message": text,
                "code": { "code": code },
                "level": "warning",
                "spans": [{ "file_name": file, "line_start": line, "line_end": line,
                            "column_start": column, "column_end": column + 4, "is_primary": true }]
            }
        })
        .to_string()
    }

    #[test]
    fn the_compilers_wording_is_read_into_kinds_and_names() {
        for (text, kind, name) in [
            (
                "function `never_called` is never used",
                SymbolKind::Function,
                "never_called",
            ),
            (
                "struct `Parcel` is never constructed",
                SymbolKind::Class,
                "Parcel",
            ),
            ("method `heavy` is never used", SymbolKind::Method, "heavy"),
            (
                "associated function `new` is never used",
                SymbolKind::Method,
                "new",
            ),
            ("field `label` is never read", SymbolKind::Property, "label"),
            (
                "variant `Closed` is never constructed",
                SymbolKind::EnumMember,
                "Closed",
            ),
            (
                "constant `LIMIT` is never used",
                SymbolKind::Variable,
                "LIMIT",
            ),
            (
                "type alias `Alias` is never used",
                SymbolKind::TypeAlias,
                "Alias",
            ),
            (
                "trait `Greeter` is never used",
                SymbolKind::Interface,
                "Greeter",
            ),
            (
                "unused import: `std::collections::HashMap`",
                SymbolKind::Import,
                "HashMap",
            ),
        ] {
            assert_eq!(
                parse_message(text),
                Some((kind, name.to_string())),
                "{text}"
            );
        }
        assert_eq!(parse_message("unused variable: `x`"), None);
        assert_eq!(parse_message("mismatched types"), None);
    }

    #[test]
    fn an_items_extent_is_read_from_the_source() {
        let text = "use a::B;\n\nfn multi(\n    a: u32,\n) -> u32 {\n    let s = \"}\"; // }\n    a\n}\n\nconst LIMIT: u32 = 5;\n";
        assert_eq!(item_end(text, 1, SymbolKind::Import), (1, 8));
        assert_eq!(
            item_end(text, 3, SymbolKind::Function),
            (8, 0),
            "braces in strings and comments do not close it"
        );
        assert_eq!(item_end(text, 10, SymbolKind::Variable), (10, 20));
    }

    #[test]
    fn a_crlf_checkout_measures_the_same_extents() {
        let lf = "use a::B;\n\nfn multi(\n    a: u32,\n) -> u32 {\n    a\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(
            item_end(&crlf, 3, SymbolKind::Function).0,
            item_end(lf, 3, SymbolKind::Function).0
        );
        assert_eq!(item_end(&crlf, 3, SymbolKind::Function).0, 7);
    }

    #[test]
    fn diagnostics_become_findings_with_the_right_category_and_size() {
        let tree = TempTree::new("rustc-findings");
        tree.write("Cargo.toml", "[package]\nname = \"demo\"\n")
            .write(
                "src/lib.rs",
                "use std::collections::HashMap;\n\nfn never_called() -> u32 {\n    1\n}\n\nstruct Parcel { weight: u32 }\nimpl Parcel { fn heavy(&self) -> bool { self.weight > 1 } }\n",
            );
        let root = tree.path().canonicalize().unwrap();
        let text = [
            message(
                &root,
                "src/lib.rs",
                1,
                5,
                "unused_imports",
                "unused import: `std::collections::HashMap`",
            ),
            message(
                &root,
                "src/lib.rs",
                3,
                4,
                "dead_code",
                "function `never_called` is never used",
            ),
            message(
                &root,
                "src/lib.rs",
                8,
                18,
                "dead_code",
                "method `heavy` is never used",
            ),
            // The same function, reported again by the test target.
            message(
                &root,
                "src/lib.rs",
                3,
                4,
                "dead_code",
                "function `never_called` is never used",
            ),
            // Not dead code.
            message(
                &root,
                "src/lib.rs",
                4,
                5,
                "unused_variables",
                "unused variable: `x`",
            ),
            "   Compiling demo v0.1.0".to_string(),
        ]
        .join("\n");
        let found = findings(&text, std::slice::from_ref(&root), Path::new("."), false);
        let summary: Vec<(String, Category, u32)> = found
            .iter()
            .map(|f| (f.name.clone(), f.category, f.lines))
            .collect();
        assert_eq!(
            summary,
            [
                ("HashMap".to_string(), Category::UnusedImport, 1),
                ("never_called".to_string(), Category::UnusedSymbol, 3),
                ("heavy".to_string(), Category::UnusedMember, 1),
            ]
        );
        assert!(
            found
                .iter()
                .all(|f| f.language == "rust" && f.confidence == 100 && f.path == "src/lib.rs")
        );
        assert!(
            found[1].message.contains("rustc `dead_code`"),
            "{}",
            found[1].message
        );
    }

    #[test]
    fn a_diagnostic_outside_the_scan_or_inside_a_macro_is_left_out() {
        let tree = TempTree::new("rustc-scope");
        tree.write("crates/a/Cargo.toml", "")
            .write("crates/a/src/lib.rs", "fn dead() {}\n")
            .write("crates/b/Cargo.toml", "")
            .write("crates/b/src/lib.rs", "fn dead() {}\n");
        let root = tree.path().canonicalize().unwrap();
        let only_a = root.join("crates/a");
        let mut in_macro: serde_json::Value = serde_json::from_str(&message(
            &root.join("crates/a"),
            "src/lib.rs",
            1,
            4,
            "dead_code",
            "function `dead` is never used",
        ))
        .unwrap();
        in_macro["message"]["spans"][0]["expansion"] =
            serde_json::json!({ "macro_decl_name": "m!" });
        let text = [
            message(
                &root.join("crates/a"),
                "src/lib.rs",
                1,
                4,
                "dead_code",
                "function `dead` is never used",
            ),
            message(
                &root.join("crates/b"),
                "src/lib.rs",
                1,
                4,
                "dead_code",
                "function `dead` is never used",
            ),
            in_macro.to_string(),
        ]
        .join("\n");
        let found = findings(&text, std::slice::from_ref(&only_a), Path::new("."), false);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].path, "src/lib.rs");
    }

    #[test]
    fn a_relative_manifest_path_is_taken_from_where_the_diagnostics_live() {
        // A file committed next to the crate, or an artifact copied off a CI
        // runner: cargo wrote absolute paths that mean nothing here.
        let tree = TempTree::new("rustc-relative");
        tree.write("app/Cargo.toml", "")
            .write("app/src/lib.rs", "fn dead() {}\n");
        let root = tree.path().canonicalize().unwrap();
        let text = message(
            Path::new("."),
            "src/lib.rs",
            1,
            4,
            "dead_code",
            "function `dead` is never used",
        );
        // Resolved against the wrong place, the path falls outside the scan.
        assert!(
            findings(
                &text,
                std::slice::from_ref(&root),
                &std::env::temp_dir(),
                false
            )
            .is_empty()
        );
        let found = findings(&text, std::slice::from_ref(&root), &root.join("app"), false);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].path, "app/src/lib.rs");
    }

    #[test]
    fn test_code_is_reported_only_on_request_and_with_less_confidence() {
        let tree = TempTree::new("rustc-tests");
        tree.write("Cargo.toml", "")
            .write("src/lib.rs", "fn helper() {}\n")
            .write("tests/it.rs", "use crate::common;\n");
        let root = tree.path().canonicalize().unwrap();
        let mut lib: serde_json::Value = serde_json::from_str(&message(
            &root,
            "src/lib.rs",
            1,
            4,
            "dead_code",
            "function `helper` is never used",
        ))
        .unwrap();
        lib["target"] = serde_json::json!({ "kind": ["lib"], "name": "demo" });
        let mut test: serde_json::Value = serde_json::from_str(&message(
            &root,
            "tests/it.rs",
            1,
            5,
            "unused_imports",
            "unused import: `crate::common`",
        ))
        .unwrap();
        test["target"] = serde_json::json!({ "kind": ["test"], "name": "it" });
        let text = format!("{lib}\n{test}\n");

        let by_default = findings(&text, std::slice::from_ref(&root), Path::new("."), false);
        assert_eq!(
            by_default
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["helper"],
            "the test harness's finding is not reported unless asked for"
        );

        let with_tests = findings(&text, std::slice::from_ref(&root), Path::new("."), true);
        let common = with_tests
            .iter()
            .find(|f| f.name == "common")
            .expect("reported on request");
        assert_eq!(common.confidence, 85);
        assert_eq!(common.reasons, [cpd_core::deadcode::Reason::InTestFile]);
        let helper = with_tests.iter().find(|f| f.name == "helper").unwrap();
        assert_eq!(helper.confidence, 100);
    }

    #[test]
    fn a_workspace_crates_paths_are_relative_to_the_workspace_root() {
        // cargo run at a workspace root writes `arena-core/tests/it.rs`, not
        // `tests/it.rs`, while manifest_path names the crate: joining the
        // two naively gave `arena-core/arena-core/tests/it.rs`.
        let tree = TempTree::new("rustc-workspace");
        tree.write("Cargo.toml", "[workspace]\n")
            .write("arena-core/Cargo.toml", "")
            .write("arena-core/src/lib.rs", "fn dead() {}\n");
        let root = tree.path().canonicalize().unwrap();
        let text = message(
            &root.join("arena-core"),
            "arena-core/src/lib.rs",
            1,
            4,
            "dead_code",
            "function `dead` is never used",
        );
        let found = findings(&text, std::slice::from_ref(&root), Path::new("."), false);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].path, "arena-core/src/lib.rs");
        assert_eq!(found[0].lines, 1, "the extent was read from the real file");
    }

    #[test]
    fn compiler_findings_fold_into_a_report_and_its_percentage() {
        let tree = TempTree::new("rustc-merge");
        tree.write("src/lib.rs", "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n");
        let root = tree.path().canonicalize().unwrap();
        let rust = sources(std::slice::from_ref(&root), true);
        assert_eq!(rust, Sources { files: 1, lines: 4 });
        let base = Report {
            findings: Vec::new(),
            statistics: Stats {
                total_lines: 6,
                ..crate::rustc::stats(&[], 6)
            },
        };
        let text = message(
            &root,
            "src/lib.rs",
            2,
            4,
            "dead_code",
            "function `b` is never used",
        );
        let merged = merge(
            base,
            findings(&text, std::slice::from_ref(&root), Path::new("."), false),
            rust,
        );
        assert_eq!(merged.statistics.total_lines, 10);
        assert_eq!(merged.statistics.files, 1);
        assert_eq!(merged.statistics.dead_lines, 1);
        assert_eq!(merged.findings.len(), 1);
        assert_eq!(
            merged.statistics.by_category[0].category,
            Category::UnusedSymbol
        );
    }
}
