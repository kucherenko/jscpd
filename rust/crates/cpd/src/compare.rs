//! The `--compare` mode: two folders compared function by function.
//!
//! One question covers two situations. A port to another language or
//! platform, the source first and the target second (`jscpd --compare
//! python-lib/ rust-lib/`, `jscpd --compare ios/ android/`): which functions
//! of the source have their counterpart in the target, and which are still
//! to port. Two implementations of one app that both live on: what both
//! have, and what only one of them has. The report shows both directions, so
//! a port reads the first side's numbers and a parity check reads both.
//!
//! Functions are paired by the `--semantic` model (see
//! [`cpd_semantic::compare`]), so every `--semantic-*` option applies. The
//! walk is the one of a clone run (`--ignore`, `--format`, `--pattern`,
//! .gitignore); no clone detection runs.

use crate::options::Options;
use crate::{Exit, fatal};
use cpd_finder::orchestrate::{RunConfig, build_thread_pool, prepare_scan_in};
use cpd_reporter::shared::{Style, write_report_file};
use cpd_semantic::UnitReader;
use cpd_semantic::compare::{CompareParams, Comparison, compare};
use cpd_semantic::search::{SemanticUnit, UnitSource};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The reporters `--compare` writes to.
const REPORTERS: &str = "console, console-full, json and markdown";

pub fn run(opts: &Options, paths: &[PathBuf], run_config: &RunConfig) -> Result<(), Exit> {
    let [left, right] = paths else {
        return Err(fatal(format!(
            "--compare takes two paths, the two sides to compare (got {}): the source first and the target second, e.g. jscpd --compare ios/ android/",
            paths.len()
        )));
    };
    let roots = [left, right].map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()));
    if roots[0].starts_with(&roots[1]) || roots[1].starts_with(&roots[0]) {
        return Err(fatal(format!(
            "--compare: {} and {} overlap; give two separate folders",
            left.display(),
            right.display()
        )));
    }
    let Some(semantic) = &opts.semantic else {
        unreachable!("--compare turns --semantic on");
    };
    // The scope may come from the flag or the config file's section.
    if semantic.scope != cpd_semantic::SemanticScope::default() {
        eprintln!(
            "Warning: --compare pairs the functions of the two sides whatever their languages; the semantic scope '{}' has no effect",
            semantic.scope.as_str()
        );
    }
    let embedder = cpd_semantic::embedder(semantic, paths, opts.silent)
        .map_err(|e| fatal(format!("--compare: {e}")))?;
    let reader = Arc::new(UnitReader::default());
    let config = RunConfig {
        // Only files jscpd finds functions in, unless --format says which.
        formats: match run_config.formats.is_empty() {
            true => cpd_tokenizer::formats::list_formats()
                .into_iter()
                .filter(|f| cpd_semantic::units::supports_units(f))
                .map(String::from)
                .collect(),
            false => run_config.formats.clone(),
        },
        // Every file counts, however small: a port's version of a function
        // is often shorter than the original. Size is judged per function.
        min_tokens: 0,
        min_lines: 0,
        skip_local: false,
        skip_isolated: Vec::new(),
        passes: vec![reader.clone()],
        ..run_config.clone()
    };
    let timer = std::time::Instant::now();
    let pool = build_thread_pool(opts.workers);
    prepare_scan_in(&pool, &config);
    let mut sides: [Vec<UnitSource>; 2] = [Vec::new(), Vec::new()];
    for source in reader.take_sources() {
        let file = Path::new(cpd_core::paths::clean_source_id(&source.id));
        if let Some(side) = roots.iter().position(|root| file.starts_with(root)) {
            sides[side].push(source);
        }
    }
    let params = CompareParams {
        thresholds: semantic.thresholds(),
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
    };
    let comparison = pool
        .install(|| compare([&sides[0], &sides[1]], embedder.as_ref(), &params))
        .map_err(|e| fatal(format!("--compare: {e}")))?;
    let report = Report::new(
        [left, right].map(|p| p.display().to_string()),
        &roots,
        &sides,
        &comparison,
    );
    write_reports(opts, &report).map_err(fatal)?;
    if !opts.silent && !opts.reporters.iter().all(|r| r == "silent") {
        eprintln!(
            "Compared in {:.3}s using {}",
            timer.elapsed().as_secs_f64(),
            semantic.model
        );
    }
    Ok(())
}

/// Run the reporters `opts` names; the ones `--compare` has no use for get a
/// warning.
fn write_reports(opts: &Options, report: &Report) -> Result<(), String> {
    let style = Style::new(opts.no_colors);
    let mut ignored = Vec::new();
    for name in &opts.reporters {
        match name.as_str() {
            "console" | "console-full" | "full" | "consoleFull" if opts.silent => {}
            "console" => print!("{}", report.console(&style, false)),
            "console-full" | "full" | "consoleFull" => print!("{}", report.console(&style, true)),
            "json" => {
                let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
                write_report_file(&opts.output_dir, "jscpd-compare.json", json, &style, "JSON")
                    .map_err(|e| format!("json reporter: {e}"))?;
            }
            "markdown" => {
                write_report_file(
                    &opts.output_dir,
                    "jscpd-compare.md",
                    report.markdown(),
                    &style,
                    "Markdown",
                )
                .map_err(|e| format!("markdown reporter: {e}"))?;
            }
            "silent" | "time" | "threshold" => {}
            other => ignored.push(other),
        }
    }
    if !ignored.is_empty() {
        eprintln!(
            "Warning: --compare reports to {REPORTERS}; ignoring {}",
            ignored.join(", ")
        );
    }
    Ok(())
}

/// What `--compare` reports, and the JSON reporter's document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    sides: [Side; 2],
    /// Every pair: `a` on the first side, `b` on the second.
    pairs: Vec<PairEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Side {
    /// The path as given on the command line.
    path: String,
    /// Functions of at least --min-tokens tokens (30 by default with
    /// --compare) and --min-lines lines.
    functions: usize,
    /// Those of them with a counterpart on the other side.
    matched: usize,
    percentage: f64,
    files: Vec<FileEntry>,
    /// The functions that count and have no counterpart.
    unmatched: Vec<Function>,
    /// No function at all, of any size: a port not started yet.
    #[serde(skip)]
    empty: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    /// Relative to the side's path.
    file: String,
    functions: usize,
    matched: usize,
    /// The file of the other side holding most of this file's
    /// counterparts.
    #[serde(skip_serializing_if = "Option::is_none")]
    counterpart: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Function {
    file: String,
    name: String,
    start: u32,
    end: u32,
}

impl Function {
    fn lines(&self) -> u32 {
        self.end - self.start + 1
    }

    fn place(&self) -> String {
        format!("{}:{}", self.file, self.start)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairEntry {
    a: Function,
    b: Function,
    similarity: f64,
    /// `code` (the functions' code matched) or `name` (the names match and
    /// the code is similar enough).
    matched_by: &'static str,
}

impl Report {
    fn new(
        paths: [String; 2],
        roots: &[PathBuf; 2],
        sides: &[Vec<UnitSource>; 2],
        comparison: &Comparison,
    ) -> Self {
        let root_is_file = [&roots[0], &roots[1]].map(|root| root.is_file());
        let function = |index: usize| {
            let f = comparison.functions[index];
            let source = &sides[f.side][f.source];
            describe(
                &roots[f.side],
                root_is_file[f.side],
                &source.id,
                &source.units[f.unit],
            )
        };
        let paired = comparison.paired();
        let pairs: Vec<PairEntry> = comparison
            .pairs
            .iter()
            .map(|pair| PairEntry {
                a: function(pair.a),
                b: function(pair.b),
                similarity: (f64::from(pair.similarity) * 1000.0).round() / 1000.0,
                matched_by: pair.matched_by.as_str(),
            })
            .collect();
        let sides = [0, 1].map(|side| {
            // file -> (functions, matched, partner files and their counts)
            let mut files: BTreeMap<String, (usize, usize, BTreeMap<String, usize>)> =
                BTreeMap::new();
            let mut unmatched = Vec::new();
            for (index, f) in comparison.functions.iter().enumerate() {
                if f.side != side || !f.counted {
                    continue;
                }
                let described = function(index);
                let entry = files.entry(described.file.clone()).or_default();
                entry.0 += 1;
                if paired[index] {
                    entry.1 += 1;
                } else {
                    unmatched.push(described);
                }
            }
            for pair in &pairs {
                let (own, other) = match side {
                    0 => (&pair.a, &pair.b),
                    _ => (&pair.b, &pair.a),
                };
                if let Some(entry) = files.get_mut(&own.file) {
                    *entry.2.entry(other.file.clone()).or_default() += 1;
                }
            }
            let functions: usize = files.values().map(|f| f.0).sum();
            let matched: usize = files.values().map(|f| f.1).sum();
            Side {
                path: paths[side].clone(),
                functions,
                matched,
                percentage: percentage(matched, functions),
                files: files
                    .into_iter()
                    .map(|(file, (functions, matched, partners))| FileEntry {
                        file,
                        functions,
                        matched,
                        // Most counterparts first; a tie goes to the first
                        // file by name.
                        counterpart: partners
                            .into_iter()
                            .max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0)))
                            .map(|(file, _)| file),
                    })
                    .collect(),
                unmatched,
                empty: !comparison.functions.iter().any(|f| f.side == side),
            }
        });
        Report { sides, pairs }
    }

    /// The console report; `full` adds every pair. A side with no
    /// functions, such as the target of a port not started yet, leaves
    /// nothing to list: the report is the other side's total and a note.
    fn console(&self, style: &Style, full: bool) -> String {
        let mut out = String::new();
        let [left, right] = &self.sides;
        match (left.empty, right.empty) {
            (true, true) => {
                return format!("No functions in {} or {} yet\n", left.path, right.path);
            }
            (false, true) | (true, false) => {
                let (side, empty) = match left.empty {
                    true => (right, left),
                    false => (left, right),
                };
                return format!(
                    "{} 0 of {} functions in {} have a counterpart in {}\n{} has no functions yet\n",
                    style.bold("  0%"),
                    side.functions,
                    side.path,
                    empty.path,
                    empty.path,
                );
            }
            (false, false) => {}
        }
        for (side, other) in [(left, right), (right, left)] {
            out.push_str(&format!(
                "{} {} of {} functions in {} have a counterpart in {}\n",
                style.bold(&format!("{:>3}%", side.percentage.round())),
                side.matched,
                side.functions,
                side.path,
                other.path,
            ));
        }
        for side in &self.sides {
            if side.files.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(&style.bold(&side.path));
            out.push('\n');
            let rows: Vec<[String; 3]> = side
                .files
                .iter()
                .map(|f| {
                    [
                        f.file.clone(),
                        format!("{} / {}", f.matched, f.functions),
                        f.counterpart
                            .as_deref()
                            .map_or(String::new(), |c| format!("→ {c}")),
                    ]
                })
                .collect();
            push_table(&mut out, &rows, style);
        }
        for side in &self.sides {
            if side.unmatched.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(&style.bold(&format!(
                "Only in {} ({}):",
                side.path,
                side.unmatched.len()
            )));
            out.push('\n');
            let rows: Vec<[String; 3]> = side
                .unmatched
                .iter()
                .map(|f| [f.place(), f.name.clone(), format!("{} lines", f.lines())])
                .collect();
            push_table(&mut out, &rows, style);
        }
        if full && !self.pairs.is_empty() {
            out.push('\n');
            out.push_str(&style.bold(&format!("Pairs ({}):", self.pairs.len())));
            out.push('\n');
            let rows: Vec<[String; 3]> = self
                .pairs
                .iter()
                .map(|p| {
                    [
                        format!("{} {}", p.a.place(), p.a.name),
                        format!("{} {}", p.b.place(), p.b.name),
                        match p.matched_by {
                            "name" => format!("{:.2} by name", p.similarity),
                            _ => format!("{:.2}", p.similarity),
                        },
                    ]
                })
                .collect();
            push_table(&mut out, &rows, style);
        }
        out
    }

    fn markdown(&self) -> String {
        let [left, right] = &self.sides;
        let mut out = format!(
            "# {} compared with {}\n\n",
            code(&left.path),
            code(&right.path)
        );
        out.push_str("| Side | Functions | With a counterpart | % |\n|---|--:|--:|--:|\n");
        for side in &self.sides {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                code(&side.path),
                side.functions,
                side.matched,
                side.percentage
            ));
        }
        for side in &self.sides {
            if side.files.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\n## {}\n\n| File | With a counterpart | Counterpart file |\n|---|--:|---|\n",
                code(&side.path)
            ));
            for f in &side.files {
                out.push_str(&format!(
                    "| {} | {} / {} | {} |\n",
                    code(&f.file),
                    f.matched,
                    f.functions,
                    f.counterpart.as_deref().map_or(String::new(), code)
                ));
            }
        }
        for side in &self.sides {
            if side.unmatched.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\n## Only in {} ({})\n\n| Function | Place | Lines |\n|---|---|--:|\n",
                code(&side.path),
                side.unmatched.len()
            ));
            for f in &side.unmatched {
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    code(&f.name),
                    code(&f.place()),
                    f.lines()
                ));
            }
        }
        if !self.pairs.is_empty() {
            out.push_str(&format!(
                "\n## Pairs ({})\n\n| {} | {} | Similarity | Matched by |\n|---|---|--:|---|\n",
                self.pairs.len(),
                code(&left.path),
                code(&right.path)
            ));
            for p in &self.pairs {
                out.push_str(&format!(
                    "| {} {} | {} {} | {:.2} | {} |\n",
                    code(&p.a.name),
                    code(&p.a.place()),
                    code(&p.b.name),
                    code(&p.b.place()),
                    p.similarity,
                    p.matched_by
                ));
            }
        }
        out
    }
}

/// `text` as a Markdown code span inside a table cell: a `|` would end the
/// cell, and a backtick the span (a Kotlin function can be named
/// `` `rounds cents` ``), so the first is escaped and the second makes the
/// span use two backticks.
fn code(text: &str) -> String {
    let text = text.replace('|', "\\|");
    match text.contains('`') {
        true => format!("`` {text} ``"),
        false => format!("`{text}`"),
    }
}

/// A function as the report names it: its file relative to the side's root,
/// or its file name when the side is one file.
fn describe(root: &Path, root_is_file: bool, source_id: &str, unit: &SemanticUnit) -> Function {
    let file = Path::new(cpd_core::paths::clean_source_id(source_id));
    let relative = match root_is_file {
        true => file.file_name().map(PathBuf::from),
        false => file.strip_prefix(root).ok().map(Path::to_path_buf),
    };
    Function {
        file: relative
            .unwrap_or_else(|| file.to_path_buf())
            .to_string_lossy()
            .into_owned(),
        name: unit.name.clone(),
        start: unit.start.line,
        end: unit.end.line,
    }
}

fn percentage(part: usize, whole: usize) -> f64 {
    match whole {
        0 => 0.0,
        _ => (part as f64 * 10000.0 / whole as f64).round() / 100.0,
    }
}

/// Rows of three columns, indented, the first two padded to their widest
/// cell and the third dimmed.
fn push_table(out: &mut String, rows: &[[String; 3]], style: &Style) {
    let width = |k: usize| rows.iter().map(|r| r[k].chars().count()).max().unwrap_or(0);
    let (first, second) = (width(0), width(1));
    for [a, b, c] in rows {
        let line = format!("  {a:<first$}  {b:<second$}  {}", style.dim(c));
        out.push_str(line.trim_end());
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::Location;
    use cpd_semantic::compare::{FunctionRef, MatchedBy, Pair};

    fn unit(name: &str, line: u32) -> SemanticUnit {
        SemanticUnit {
            grammar: "java",
            name: name.to_string(),
            start: Location::new(line, 0, 0),
            end: Location::new(line + 9, 0, 0),
            range: [0, 59],
            token_count: 60,
            text: String::new(),
        }
    }

    fn source(id: &str, units: Vec<SemanticUnit>) -> UnitSource {
        UnitSource {
            id: id.to_string(),
            format: "java".to_string(),
            units,
            path_label: Default::default(),
        }
    }

    /// Java `QrCode.java` with three functions, two of them ported to one
    /// Python file, and a Python helper with no Java counterpart.
    fn report() -> Report {
        let sides = [
            vec![source(
                "/p/java/QrCode.java",
                vec![
                    unit("drawVersion", 10),
                    unit("applyMask", 30),
                    unit("makeKanji", 50),
                ],
            )],
            vec![source(
                "/p/python/qrcodegen.py",
                vec![
                    unit("_draw_version", 5),
                    unit("_apply_mask", 20),
                    unit("helper", 40),
                ],
            )],
        ];
        let f = |side, unit| FunctionRef {
            side,
            source: 0,
            unit,
            counted: true,
        };
        let comparison = Comparison {
            functions: vec![f(0, 0), f(0, 1), f(0, 2), f(1, 0), f(1, 1), f(1, 2)],
            pairs: vec![
                Pair {
                    a: 0,
                    b: 3,
                    similarity: 0.9012,
                    matched_by: MatchedBy::Code,
                },
                Pair {
                    a: 1,
                    b: 4,
                    similarity: 0.65,
                    matched_by: MatchedBy::Name,
                },
            ],
        };
        Report::new(
            ["java/".into(), "python/".into()],
            &[PathBuf::from("/p/java"), PathBuf::from("/p/python")],
            &sides,
            &comparison,
        )
    }

    #[test]
    fn totals_files_and_unmatched_functions_per_side() {
        let report = report();
        let [java, python] = &report.sides;
        assert_eq!(
            (java.functions, java.matched, java.percentage),
            (3, 2, 66.67)
        );
        assert_eq!(java.files.len(), 1);
        assert_eq!(java.files[0].file, "QrCode.java");
        assert_eq!(java.files[0].counterpart.as_deref(), Some("qrcodegen.py"));
        let names = |fs: &[Function]| fs.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&java.unmatched), vec!["makeKanji"]);
        assert_eq!(names(&python.unmatched), vec!["helper"]);
        assert_eq!(report.pairs[0].similarity, 0.901);
        assert_eq!(report.pairs[1].matched_by, "name");
    }

    #[test]
    fn console_report_shows_both_directions() {
        let text = report().console(&Style::new(true), false);
        assert_eq!(
            text,
            " 67% 2 of 3 functions in java/ have a counterpart in python/\n 67% 2 of 3 functions in python/ have a counterpart in java/\n\njava/\n  QrCode.java  2 / 3  → qrcodegen.py\n\npython/\n  qrcodegen.py  2 / 3  → QrCode.java\n\nOnly in java/ (1):\n  QrCode.java:50  makeKanji  10 lines\n\nOnly in python/ (1):\n  qrcodegen.py:40  helper  10 lines\n"
        );
        let full = report().console(&Style::new(true), true);
        assert!(full.ends_with(
            "Pairs (2):\n  QrCode.java:10 drawVersion  qrcodegen.py:5 _draw_version  0.90\n  QrCode.java:30 applyMask    qrcodegen.py:20 _apply_mask   0.65 by name\n"
        ));
    }

    #[test]
    fn an_empty_side_gets_a_note_instead_of_a_report() {
        let sides = [
            vec![source("/p/java/QrCode.java", vec![unit("drawVersion", 10)])],
            Vec::new(),
        ];
        let comparison = Comparison {
            functions: vec![FunctionRef {
                side: 0,
                source: 0,
                unit: 0,
                counted: true,
            }],
            pairs: Vec::new(),
        };
        let report = Report::new(
            ["java/".into(), "rust/".into()],
            &[PathBuf::from("/p/java"), PathBuf::from("/p/rust")],
            &sides,
            &comparison,
        );
        let style = Style::new(true);
        let expected = "  0% 0 of 1 functions in java/ have a counterpart in rust/\nrust/ has no functions yet\n";
        assert_eq!(report.console(&style, false), expected);
        assert_eq!(report.console(&style, true), expected);
        // JSON keeps the list of what is left to port.
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["sides"][0]["unmatched"][0]["name"], "drawVersion");
        assert!(json["sides"][1].get("empty").is_none());

        let none = Report::new(
            ["java/".into(), "rust/".into()],
            &[PathBuf::from("/p/java"), PathBuf::from("/p/rust")],
            &[Vec::new(), Vec::new()],
            &Comparison::default(),
        );
        assert_eq!(
            none.console(&style, false),
            "No functions in java/ or rust/ yet\n"
        );
    }

    #[test]
    fn json_report_uses_camel_case_keys() {
        let json = serde_json::to_value(report()).unwrap();
        assert_eq!(json["sides"][0]["path"], "java/");
        assert_eq!(json["sides"][1]["unmatched"][0]["name"], "helper");
        assert_eq!(json["pairs"][1]["matchedBy"], "name");
        assert_eq!(json["pairs"][0]["b"]["file"], "qrcodegen.py");
    }

    #[test]
    fn markdown_code_spans_survive_pipes_and_backticks() {
        assert_eq!(code("drawVersion"), "`drawVersion`");
        assert_eq!(code("operator|"), "`operator\\|`");
        assert_eq!(code("`rounds cents`"), "`` `rounds cents` ``");
    }

    #[test]
    fn markdown_report_has_totals_and_lists() {
        let md = report().markdown();
        assert!(md.starts_with("# `java/` compared with `python/`\n\n| Side | Functions |"));
        assert!(md.contains("| `java/` | 3 | 2 | 66.67 |"));
        assert!(md.contains("## Only in `python/` (1)"));
        assert!(md.contains(
            "| `drawVersion` `QrCode.java:10` | `_draw_version` `qrcodegen.py:5` | 0.90 | code |"
        ));
    }
}
