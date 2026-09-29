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
use cpd_semantic::compare::{CompareParams, Comparison, compare, name_key};
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
    /// Mean similarity of the pairs of this file's functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<f64>,
    /// How many of those pairs are of the `low` level.
    low_pairs: usize,
}

impl FileEntry {
    /// The mean similarity of the file's pairs, with the number of low ones
    /// when there are any: `0.62, 1 low`.
    fn similarity_text(&self) -> String {
        match (self.similarity, self.low_pairs) {
            (None, _) => String::new(),
            (Some(mean), 0) => format!("{mean:.2}"),
            (Some(mean), low) => format!("{mean:.2}, {low} low"),
        }
    }

    /// [`Self::similarity_text`] for the console, the low count in red.
    fn similarity_cell(&self, style: &Style) -> Cell {
        let plain = self.similarity_text();
        let shown = match (self.similarity, self.low_pairs) {
            (Some(mean), low) if low > 0 => {
                format!("{mean:.2}, {}", style.paint(&format!("{low} low"), RED))
            }
            _ => plain.clone(),
        };
        Cell { plain, shown }
    }
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
    /// `high`, `medium` or `low`, on the scale of the model; see
    /// [`cpd_semantic::compare::Level`].
    level: &'static str,
    /// Whether the two functions have different names once case and
    /// underscores are ignored: a pair a reader could not guess.
    renamed: bool,
    /// `code` (the functions' code matched) or `name` (the names match and
    /// the code is similar enough).
    matched_by: &'static str,
}

impl PairEntry {
    /// The similarity and its level, for a report: `0.87 high`.
    fn score(&self) -> String {
        format!("{:.2} {}", self.similarity, self.level)
    }
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
            .map(|pair| {
                let (a, b) = (function(pair.a), function(pair.b));
                PairEntry {
                    renamed: name_key(&a.name) != name_key(&b.name),
                    a,
                    b,
                    similarity: round(f64::from(pair.similarity), 1000.0),
                    level: pair.level.as_str(),
                    matched_by: pair.matched_by.as_str(),
                }
            })
            .collect();
        let sides = [0, 1].map(|side| {
            // file -> (functions, matched, partner files and their counts,
            // the similarities of its pairs, how many of them are low)
            #[derive(Default)]
            struct Tally {
                functions: usize,
                matched: usize,
                partners: BTreeMap<String, usize>,
                similarities: Vec<f64>,
                low: usize,
            }
            let mut files: BTreeMap<String, Tally> = BTreeMap::new();
            let mut unmatched = Vec::new();
            for (index, f) in comparison.functions.iter().enumerate() {
                if f.side != side || !f.counted {
                    continue;
                }
                let described = function(index);
                let entry = files.entry(described.file.clone()).or_default();
                entry.functions += 1;
                if paired[index] {
                    entry.matched += 1;
                } else {
                    unmatched.push(described);
                }
            }
            for (pair, found) in pairs.iter().zip(&comparison.pairs) {
                let (own, other, own_index) = match side {
                    0 => (&pair.a, &pair.b, found.a),
                    _ => (&pair.b, &pair.a, found.b),
                };
                // Only the pairs of functions the file's numbers count: a
                // short function paired by name does not set the file's
                // similarity or counterpart.
                if !comparison.functions[own_index].counted {
                    continue;
                }
                if let Some(entry) = files.get_mut(&own.file) {
                    *entry.partners.entry(other.file.clone()).or_default() += 1;
                    entry.similarities.push(pair.similarity);
                    entry.low += usize::from(pair.level == "low");
                }
            }
            // By file, then line: nested functions come out of the
            // extractors after the function around them.
            unmatched
                .sort_by(|x: &Function, y: &Function| (&x.file, x.start).cmp(&(&y.file, y.start)));
            let functions: usize = files.values().map(|f| f.functions).sum();
            let matched: usize = files.values().map(|f| f.matched).sum();
            Side {
                path: paths[side].clone(),
                functions,
                matched,
                percentage: percentage(matched, functions),
                files: files
                    .into_iter()
                    .map(|(file, tally)| FileEntry {
                        file,
                        functions: tally.functions,
                        matched: tally.matched,
                        // Most counterparts first; a tie goes to the first
                        // file by name.
                        counterpart: tally
                            .partners
                            .into_iter()
                            .max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0)))
                            .map(|(file, _)| file),
                        similarity: (!tally.similarities.is_empty()).then(|| {
                            let sum: f64 = tally.similarities.iter().sum();
                            round(sum / tally.similarities.len() as f64, 100.0)
                        }),
                        low_pairs: tally.low,
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
                let note = format!("No functions in {} or {} yet", left.path, right.path);
                return format!("{}\n", style.paint(&note, YELLOW));
            }
            (false, true) | (true, false) => {
                let (side, empty) = match left.empty {
                    true => (right, left),
                    false => (left, right),
                };
                let note = format!("{} has no functions yet", empty.path);
                return format!(
                    "{} 0 of {} functions in {} have a counterpart in {}\n{}\n",
                    style.bold(&style.paint("  0%", RED)),
                    side.functions,
                    style.bold(&side.path),
                    style.bold(&empty.path),
                    style.paint(&note, YELLOW),
                );
            }
            (false, false) => {}
        }
        for (side, other) in [(left, right), (right, left)] {
            let share = format!("{:>3}%", side.percentage.round());
            out.push_str(&format!(
                "{} {} of {} functions in {} have a counterpart in {}\n",
                style.bold(&style.paint(&share, share_color(side.matched, side.functions))),
                side.matched,
                side.functions,
                style.bold(&side.path),
                style.bold(&other.path),
            ));
        }
        for side in &self.sides {
            if side.files.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(&style.bold(&side.path));
            out.push('\n');
            let rows: Vec<Vec<Cell>> = side
                .files
                .iter()
                .map(|f| {
                    let paired = format!("{} / {}", f.matched, f.functions);
                    vec![
                        Cell::new(&f.file, |t| style.paint(t, GREEN)),
                        Cell::new(&paired, |t| {
                            style.paint(t, share_color(f.matched, f.functions))
                        }),
                        f.similarity_cell(style),
                        Cell::new(f.counterpart.as_deref().unwrap_or_default(), |t| {
                            style.dim(t)
                        }),
                    ]
                })
                .collect();
            push_table(
                &mut out,
                &["file", "paired", "similarity", "counterpart"],
                &rows,
                style,
            );
        }
        let renamed: Vec<&PairEntry> = self.pairs.iter().filter(|p| p.renamed).collect();
        if !renamed.is_empty() {
            out.push('\n');
            let title = format!("Paired under other names ({}):", renamed.len());
            out.push_str(&style.bold(&style.paint(&title, CYAN)));
            out.push('\n');
            self.push_pairs(&mut out, &renamed, style);
        }
        for side in &self.sides {
            if side.unmatched.is_empty() {
                continue;
            }
            out.push('\n');
            let title = format!("Only in {} ({}):", side.path, side.unmatched.len());
            out.push_str(&style.bold(&style.paint(&title, YELLOW)));
            out.push('\n');
            // One heading per file, its functions under it; the columns line
            // up across files. `unmatched` is in file order already.
            let rows: Vec<Vec<Cell>> = side
                .unmatched
                .iter()
                .map(|f| {
                    vec![
                        Cell::new(&f.start.to_string(), |t| style.dim(t)),
                        Cell::new(&f.name, |t| style.bold(t)),
                        Cell::new(&format!("{} lines", f.lines()), |t| style.dim(t)),
                    ]
                })
                .collect();
            let lines = table_lines(&[], &rows, "    ");
            let mut at = 0;
            while at < side.unmatched.len() {
                let file = &side.unmatched[at].file;
                let count = side.unmatched[at..]
                    .iter()
                    .take_while(|f| &f.file == file)
                    .count();
                out.push_str(&format!(
                    "  {} {}\n",
                    style.paint(file, GREEN),
                    style.dim(&format!("({count})"))
                ));
                for line in &lines[at..at + count] {
                    out.push_str(line);
                    out.push('\n');
                }
                at += count;
            }
        }
        if full && !self.pairs.is_empty() {
            out.push('\n');
            out.push_str(&style.bold(&format!("Pairs ({}):", self.pairs.len())));
            out.push('\n');
            let all: Vec<&PairEntry> = self.pairs.iter().collect();
            self.push_pairs(&mut out, &all, style);
        }
        out
    }

    /// A table of `pairs`: both functions, the similarity with its level,
    /// and how the pair was found when not by code.
    fn push_pairs(&self, out: &mut String, pairs: &[&PairEntry], style: &Style) {
        let [left, right] = &self.sides;
        let function = |f: &Function| Cell {
            plain: format!("{} {}", f.place(), f.name),
            shown: format!("{} {}", style.paint(&f.place(), GREEN), style.bold(&f.name)),
        };
        let rows: Vec<Vec<Cell>> = pairs
            .iter()
            .map(|p| {
                let level = match p.level {
                    "high" => GREEN,
                    "medium" => YELLOW,
                    _ => RED,
                };
                vec![
                    function(&p.a),
                    function(&p.b),
                    Cell {
                        plain: p.score(),
                        shown: format!("{:.2} {}", p.similarity, style.paint(p.level, level)),
                    },
                    match p.matched_by {
                        "name" => Cell::new("by name", |t| style.paint(t, CYAN)),
                        _ => Cell::new("", str::to_string),
                    },
                ]
            })
            .collect();
        push_table(
            out,
            &[&left.path, &right.path, "similarity", ""],
            &rows,
            style,
        );
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
                "\n## {}\n\n| File | With a counterpart | Similarity | Counterpart file |\n|---|--:|--:|---|\n",
                code(&side.path)
            ));
            for f in &side.files {
                out.push_str(&format!(
                    "| {} | {} / {} | {} | {} |\n",
                    code(&f.file),
                    f.matched,
                    f.functions,
                    f.similarity_text(),
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
        let renamed: Vec<&PairEntry> = self.pairs.iter().filter(|p| p.renamed).collect();
        let all: Vec<&PairEntry> = self.pairs.iter().collect();
        for (title, pairs) in [("Paired under other names", &renamed), ("Pairs", &all)] {
            if pairs.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "\n## {title} ({})\n\n| {} | {} | Similarity | Level | Matched by |\n|---|---|--:|---|---|\n",
                pairs.len(),
                code(&left.path),
                code(&right.path)
            ));
            for p in pairs {
                out.push_str(&format!(
                    "| {} {} | {} {} | {:.2} | {} | {} |\n",
                    code(&p.a.name),
                    code(&p.a.place()),
                    code(&p.b.name),
                    code(&p.b.place()),
                    p.similarity,
                    p.level,
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

/// `value` rounded to a multiple of `1 / scale`.
fn round(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}

fn percentage(part: usize, whole: usize) -> f64 {
    match whole {
        0 => 0.0,
        _ => (part as f64 * 10000.0 / whole as f64).round() / 100.0,
    }
}

/// ANSI colours of the console report.
const RED: u8 = 31;
const GREEN: u8 = 32;
const YELLOW: u8 = 33;
const CYAN: u8 = 36;

/// Green when every function has a counterpart, red when none has, yellow
/// in between, and yellow when no function counts at all.
fn share_color(matched: usize, functions: usize) -> u8 {
    match (matched, functions) {
        (_, 0) => YELLOW,
        (m, f) if m == f => GREEN,
        (0, _) => RED,
        _ => YELLOW,
    }
}

/// A table cell: the text that sets the column's width, and the same text
/// as printed, colours included.
struct Cell {
    plain: String,
    shown: String,
}

impl Cell {
    fn new(text: &str, paint: impl Fn(&str) -> String) -> Self {
        Cell {
            plain: text.to_string(),
            shown: match text.is_empty() {
                true => String::new(),
                false => paint(text),
            },
        }
    }
}

/// Rows of columns, indented, every column but the last padded to its
/// widest cell. `header` names the columns in a dimmed first row; empty for
/// none.
fn push_table(out: &mut String, header: &[&str], rows: &[Vec<Cell>], style: &Style) {
    let header_line = (!header.is_empty() && !rows.is_empty()).then(|| {
        let names = header_cells(header);
        let widths = column_widths(&[&names[..]], rows);
        style.dim(&table_line(&names, &widths, "  "))
    });
    for line in header_line
        .into_iter()
        .chain(table_lines(header, rows, "  "))
    {
        out.push_str(&line);
        out.push('\n');
    }
}

/// The widest cell of each column, among `rows` and `extra` rows.
fn column_widths(extra: &[&[Cell]], rows: &[Vec<Cell>]) -> Vec<usize> {
    let columns = rows.first().map_or(0, Vec::len);
    (0..columns)
        .map(|k| {
            rows.iter()
                .map(Vec::as_slice)
                .chain(extra.iter().copied())
                .filter_map(|r| r.get(k))
                .map(|c| c.plain.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect()
}

/// `cells` after `indent`, padded to `widths` by their plain text and
/// printed as shown; empty cells at the end are dropped.
fn table_line(cells: &[Cell], widths: &[usize], indent: &str) -> String {
    let Some(last) = cells.iter().rposition(|c| !c.plain.is_empty()) else {
        return String::new();
    };
    let mut text = indent.to_string();
    for (k, cell) in cells[..=last].iter().enumerate() {
        text.push_str(&cell.shown);
        if k < last {
            let pad = widths[k] - cell.plain.chars().count() + 2;
            text.extend(std::iter::repeat_n(' ', pad));
        }
    }
    text
}

/// The lines of `rows`, one per row, with columns as wide as the widest
/// cell of `rows` or of `header`.
fn table_lines(header: &[&str], rows: &[Vec<Cell>], indent: &str) -> Vec<String> {
    let names = header_cells(header);
    let widths = column_widths(&[&names[..]], rows);
    rows.iter()
        .map(|row| table_line(row, &widths, indent))
        .collect()
}

/// Column names as cells, printed as they are.
fn header_cells(header: &[&str]) -> Vec<Cell> {
    header
        .iter()
        .map(|h| Cell {
            plain: h.to_string(),
            shown: h.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::Location;
    use cpd_semantic::compare::{FunctionRef, Level, MatchedBy, Pair};

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
    /// Python file (one under another name, one at a low level), and a
    /// Python helper with no Java counterpart.
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
                    unit("_add_version_bits", 5),
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
                    level: Level::High,
                    matched_by: MatchedBy::Code,
                },
                Pair {
                    a: 1,
                    b: 4,
                    similarity: 0.65,
                    level: Level::Low,
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
        assert!(report.pairs[0].renamed && !report.pairs[1].renamed);
        assert_eq!(java.files[0].similarity, Some(0.78));
        assert_eq!(java.files[0].low_pairs, 1);
    }

    #[test]
    fn console_report_shows_both_directions() {
        let text = report().console(&Style::new(true), false);
        assert_eq!(
            text,
            [
                " 67% 2 of 3 functions in java/ have a counterpart in python/",
                " 67% 2 of 3 functions in python/ have a counterpart in java/",
                "",
                "java/",
                "  file         paired  similarity   counterpart",
                "  QrCode.java  2 / 3   0.78, 1 low  qrcodegen.py",
                "",
                "python/",
                "  file          paired  similarity   counterpart",
                "  qrcodegen.py  2 / 3   0.78, 1 low  QrCode.java",
                "",
                "Paired under other names (1):",
                "  java/                       python/                           similarity",
                "  QrCode.java:10 drawVersion  qrcodegen.py:5 _add_version_bits  0.90 high",
                "",
                "Only in java/ (1):",
                "  QrCode.java (1)",
                "    50  makeKanji  10 lines",
                "",
                "Only in python/ (1):",
                "  qrcodegen.py (1)",
                "    40  helper  10 lines",
                "",
            ]
            .join("\n")
        );
        let full = report().console(&Style::new(true), true);
        assert!(full.starts_with(&text));
        assert!(full.ends_with(
            &[
                "Pairs (2):",
                "  java/                       python/                           similarity",
                "  QrCode.java:10 drawVersion  qrcodegen.py:5 _add_version_bits  0.90 high",
                "  QrCode.java:30 applyMask    qrcodegen.py:20 _apply_mask       0.65 low    by name",
                "",
            ]
            .join("\n")
        ));
    }

    #[test]
    fn console_colors_levels_shares_and_paths() {
        let text = report().console(&Style::new(false), true);
        // Levels: high green, low red; the low count of a file in red.
        assert!(text.contains("0.90 \x1b[32mhigh\x1b[39m"), "{text}");
        assert!(text.contains("0.65 \x1b[31mlow\x1b[39m"), "{text}");
        assert!(text.contains("0.78, \x1b[31m1 low\x1b[39m"), "{text}");
        // A share short of every function is yellow; paths are green.
        assert!(
            text.contains("\x1b[1m\x1b[33m 67%\x1b[39m\x1b[22m"),
            "{text}"
        );
        assert!(
            text.contains("  \x1b[32mQrCode.java\x1b[39m \x1b[90m(1)\x1b[39m\n"),
            "{text}"
        );
        // The colours do not move the columns: without them, the text is
        // the plain report.
        let plain = report().console(&Style::new(true), true);
        let stripped = regex::Regex::new("\x1b\\[[0-9;]*m")
            .unwrap()
            .replace_all(&text, "");
        assert_eq!(stripped, plain);
    }

    #[test]
    fn a_file_counts_only_the_pairs_of_its_counted_functions() {
        // QrCode.java's one counted function is unpaired; a short helper of
        // the same file paired by name. The file says 0 / 1 and nothing
        // about similarity or a counterpart.
        let sides = [
            vec![source(
                "/p/java/QrCode.java",
                vec![unit("makeKanji", 10), unit("clear", 30)],
            )],
            vec![source("/p/python/qrcodegen.py", vec![unit("clear", 5)])],
        ];
        let f = |side, unit, counted| FunctionRef {
            side,
            source: 0,
            unit,
            counted,
        };
        let comparison = Comparison {
            functions: vec![f(0, 0, true), f(0, 1, false), f(1, 0, true)],
            pairs: vec![Pair {
                a: 1,
                b: 2,
                similarity: 0.6,
                level: Level::Medium,
                matched_by: MatchedBy::Name,
            }],
        };
        let report = Report::new(
            ["java/".into(), "python/".into()],
            &[PathBuf::from("/p/java"), PathBuf::from("/p/python")],
            &sides,
            &comparison,
        );
        let file = &report.sides[0].files[0];
        assert_eq!((file.matched, file.functions), (0, 1));
        assert_eq!((file.similarity, file.counterpart.as_deref()), (None, None));
        // The Python side counts its function, and its pair.
        let other = &report.sides[1].files[0];
        assert_eq!((other.matched, other.similarity), (1, Some(0.6)));
    }

    #[test]
    fn nothing_to_count_is_not_shown_as_complete() {
        assert_eq!(share_color(0, 0), YELLOW);
        assert_eq!(share_color(3, 3), GREEN);
        assert_eq!(share_color(0, 3), RED);
        assert_eq!(share_color(1, 3), YELLOW);
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
        assert_eq!(json["pairs"][0]["level"], "high");
        assert_eq!(json["pairs"][0]["renamed"], true);
        assert_eq!(json["sides"][0]["files"][0]["similarity"], 0.78);
        assert_eq!(json["sides"][0]["files"][0]["lowPairs"], 1);
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
        assert!(md.contains("| `QrCode.java` | 2 / 3 | 0.78, 1 low | `qrcodegen.py` |"));
        assert!(md.contains(
            "## Paired under other names (1)\n\n| `java/` | `python/` | Similarity | Level | Matched by |\n|---|---|--:|---|---|\n| `drawVersion` `QrCode.java:10` | `_add_version_bits` `qrcodegen.py:5` | 0.90 | high | code |\n"
        ));
        assert!(md.contains(
            "| `applyMask` `QrCode.java:30` | `_apply_mask` `qrcodegen.py:20` | 0.65 | low | name |"
        ));
    }
}
