// What the stdout reporters actually print.
//
// The console reporters write straight to stdout with `println!`, which the
// test harness swallows. Each test here therefore runs one *scenario* in a
// child copy of this test binary (`--nocapture`), reads the child's stdout
// and checks the lines a user would see: clone headers, locations, snippets,
// `--blame` columns, summaries.

use cpd_core::deadcode::{Category, Finding, Reason, Stats, SymbolKind};
use cpd_core::models::{BlameEntry, CpdClone, Fragment, Location, StatRow, Statistics};
use cpd_finder::blame::BlameMap;
use cpd_reporter::context::ReportContext;
use cpd_reporter::deadcode::{DeadCodeContext, create_dead_code_reporter};
use cpd_reporter::reporter::{ReporterOptions, create_reporter};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const SCENARIO_ENV: &str = "CPD_CONSOLE_OUTPUT_SCENARIO";
const DIR_ENV: &str = "CPD_CONSOLE_OUTPUT_DIR";
const BEGIN: &str = "<<<cpd-console-output-begin>>>";
const END: &str = "<<<cpd-console-output-end>>>";

// ============================================================================
// Harness
// ============================================================================

/// Entry point of the child process; a no-op in a normal test run.
#[test]
fn scenario_child() {
    let Ok(name) = std::env::var(SCENARIO_ENV) else {
        return;
    };
    let dir = PathBuf::from(std::env::var(DIR_ENV).unwrap());
    println!("{BEGIN}");
    run_scenario(&name, &dir);
    println!("{END}");
}

/// A fresh directory for one scenario's source files.
fn scenario_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cpd-console-output-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run scenario `name` in a child process and return what it printed.
fn capture(name: &str) -> String {
    let dir = scenario_dir(name);
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "scenario_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SCENARIO_ENV, name)
        .env(DIR_ENV, &dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "scenario {name} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let start = stdout.find(BEGIN).expect("scenario started") + BEGIN.len() + 1;
    let end = stdout.find(END).expect("scenario finished");
    let _ = std::fs::remove_dir_all(&dir);
    stdout[start..end].to_string()
}

fn lines(output: &str) -> Vec<&str> {
    output.lines().collect()
}

fn has_line(output: &str, expected: &str) -> bool {
    output.lines().any(|line| line == expected)
}

// ============================================================================
// Fixtures
// ============================================================================

fn stats(clones: u64, new_clones: u64) -> Statistics {
    let row = StatRow {
        lines: 100,
        tokens: 500,
        sources: 2,
        clones,
        duplicated_lines: 20,
        duplicated_tokens: 100,
        percentage: 20.0,
        percentage_tokens: 20.0,
        new_duplicated_lines: 0,
        new_clones,
    };
    Statistics {
        total: row.clone(),
        formats: HashMap::from([("javascript".to_string(), row)]),
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

/// `count` numbered lines of JavaScript: `const v1 = 1;` ...
fn numbered_lines(count: u32) -> String {
    (1..=count)
        .map(|i| format!("const v{i} = {i};\n"))
        .collect()
}

/// Write `text` as `name` under `dir` and return a fragment over `start..=end`.
fn fragment(dir: &Path, name: &str, text: &str, start: u32, end: u32) -> Fragment {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    Fragment::new(
        path.to_string_lossy().into_owned(),
        Location::new(start, 0, 0),
        Location::new(end, 13, 0),
        [0, 10],
    )
}

fn clone_of(dir: &Path, lines_a: (u32, u32), lines_b: (u32, u32), total: u32) -> CpdClone {
    let text = numbered_lines(total);
    CpdClone::exact(
        "javascript",
        fragment(dir, "a.js", &text, lines_a.0, lines_a.1),
        fragment(dir, "b.js", &text, lines_b.0, lines_b.1),
        50,
    )
}

fn options(no_colors: bool) -> ReporterOptions {
    let mut opts = ReporterOptions::new(std::env::temp_dir());
    opts.no_colors = no_colors;
    opts
}

fn run_clone_reporter(name: &str, opts: &ReporterOptions, clones: &[CpdClone], stats: &Statistics) {
    let reporter = create_reporter(name, opts).unwrap();
    let ctx = ReportContext::new(stats, Duration::ZERO);
    reporter.report(clones, &ctx, &opts.output_dir).unwrap();
}

fn blame_of(author: &str) -> (String, String, i64) {
    (
        "0123456789abcdef".to_string(),
        author.to_string(),
        1_700_000_000,
    )
}

fn dead_finding(category: Category, path: &str, name: &str, start: u32, end: u32) -> Finding {
    Finding {
        category,
        path: path.to_string(),
        name: name.to_string(),
        exported_as: None,
        symbol_kind: if name.is_empty() {
            None
        } else {
            Some(SymbolKind::Function)
        },
        parent: None,
        language: "js".into(),
        start: Location::new(start, 0, 0),
        end: Location::new(end, 0, 0),
        lines: end - start + 1,
        confidence: 85,
        reasons: vec![Reason::DynamicAccess],
        message: format!("`{name}` is never used"),
    }
}

fn dead_stats(unparsed: usize) -> Stats {
    Stats {
        files: 12,
        unparsed: unparsed as u32,
        unparsed_files: (0..unparsed).map(|i| format!("broken/f{i}.js")).collect(),
        reachable_files: 10,
        symbols: 140,
        entry_points: 2,
        by_category: Vec::new(),
        dead_lines: 30,
        total_lines: 600,
        percentage: 5.0,
        detection_date: "2026-01-01T00:00:00Z".to_string(),
    }
}

// ============================================================================
// Scenarios (run in the child)
// ============================================================================

fn run_scenario(name: &str, dir: &Path) {
    match name {
        "console" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("console", &options(true), &[clone], &stats(1, 0));
        }
        "console-colors" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("console", &options(false), &[clone], &stats(1, 0));
        }
        "console-empty" => run_clone_reporter("console", &options(true), &[], &stats(0, 0)),
        "console-new" => {
            let mut clone = clone_of(dir, (2, 5), (7, 10), 12);
            clone.is_new = true;
            run_clone_reporter("console", &options(true), &[clone], &stats(1, 1));
        }
        "console-hostile-name" => {
            let text = numbered_lines(4);
            let a = fragment(dir, "evil\x1b[2Jname.js", &text, 1, 4);
            let b = fragment(dir, "b.js", &text, 1, 4);
            let clone = CpdClone::exact("javascript", a, b, 50);
            for name in ["console", "console-full"] {
                run_clone_reporter(
                    name,
                    &options(true),
                    std::slice::from_ref(&clone),
                    &stats(1, 0),
                );
            }
        }
        "console-full" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("console-full", &options(true), &[clone], &stats(1, 0));
        }
        "console-full-long" => {
            let clone = clone_of(dir, (1, 25), (1, 25), 25);
            run_clone_reporter("console-full", &options(true), &[clone], &stats(1, 0));
        }
        "console-full-blame" | "console-full-blame-long" => {
            let long = name.ends_with("-long");
            let (end, total) = if long { (24, 30) } else { (4, 12) };
            let clone = clone_of(dir, (2, end), (6, end + 4), total);
            let key = |f: &Fragment| f.source_id.clone();
            let mut opts = options(true);
            opts.blame = true;
            let mut blame: BlameMap = HashMap::new();
            // Line 2 of a.js and line 6 of b.js: one author; line 3 and 7:
            // different authors; line 4 and 8: no blame for b.js.
            blame.insert(
                key(&clone.fragment_a),
                HashMap::from([
                    (2, blame_of("Alice")),
                    (3, blame_of("Alice")),
                    (4, blame_of("Carol")),
                ]),
            );
            blame.insert(
                key(&clone.fragment_b),
                HashMap::from([(6, blame_of("Alice")), (7, blame_of("Bob"))]),
            );
            opts.blame_data = blame;
            run_clone_reporter("console-full", &opts, &[clone], &stats(1, 0));
        }
        "console-full-blame-missing-file" => {
            let mut clone = clone_of(dir, (2, 5), (7, 10), 12);
            clone.fragment_b.source_id = dir.join("gone.js").to_string_lossy().into_owned();
            let mut opts = options(true);
            opts.blame = true;
            run_clone_reporter("console-full", &opts, &[clone], &stats(1, 0));
        }
        "console-full-blame-on-fragments" => {
            // Blame carried on the fragments only (no blame map): the
            // console-full rows look names up in the map, so they stay empty.
            let mut clone = clone_of(dir, (2, 3), (2, 3), 4);
            let entry = BlameEntry {
                commit_sha: "deadbeef1234".to_string(),
                author: "Bob Smith".to_string(),
                timestamp: 1_700_000_000,
            };
            clone.fragment_a.blame = Some(entry.clone());
            clone.fragment_b.blame = Some(entry);
            let mut opts = options(true);
            opts.blame = true;
            run_clone_reporter("console-full", &opts, &[clone], &stats(1, 0));
        }
        "xcode" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("xcode", &options(true), &[clone], &stats(1, 0));
        }
        "ai" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("ai", &options(true), &[clone], &stats(1, 0));
        }
        "silent" => {
            let clone = clone_of(dir, (2, 5), (7, 10), 12);
            run_clone_reporter("silent", &options(true), &[clone], &stats(1, 0));
        }
        "deadcode-console" | "deadcode-console-full" | "deadcode-console-full-gone" => {
            let full = name != "deadcode-console";
            std::fs::create_dir_all(dir.join("src")).unwrap();
            if name != "deadcode-console-full-gone" {
                std::fs::write(dir.join("src/api.js"), numbered_lines(40)).unwrap();
            }
            let mut member = dead_finding(Category::UnusedMember, "src/api.js", "render", 3, 4);
            member.parent = Some("Widget".into());
            member.confidence = 100;
            member.reasons.clear();
            let mut renamed = dead_finding(Category::UnusedExport, "src/api.js", "inner", 20, 35);
            renamed.exported_as = Some("outer".into());
            renamed.confidence = 40;
            let findings = vec![
                dead_finding(Category::UnusedFile, "src/orphan.js", "", 1, 24),
                renamed,
                member,
            ];
            let stats = dead_stats(12);
            let roots = vec![dir.to_path_buf()];
            let ctx = DeadCodeContext::new(&stats, Duration::from_millis(1500)).with_roots(&roots);
            let reporter_name = if full { "console-full" } else { "console" };
            let reporter = create_dead_code_reporter(reporter_name, &options(true)).unwrap();
            reporter.report(&findings, &ctx, dir).unwrap();
        }
        "deadcode-console-empty" => {
            let stats = dead_stats(0);
            let ctx = DeadCodeContext::new(&stats, Duration::from_millis(12));
            for name in ["console", "console-full"] {
                let reporter = create_dead_code_reporter(name, &options(true)).unwrap();
                reporter.report(&[], &ctx, dir).unwrap();
            }
        }
        "dashboard" => {
            use cpd_reporter::dashboard::*;
            let view = DashboardView {
                health: cpd_core::health::Health {
                    score: None,
                    grade: None,
                    size: cpd_core::health::Size {
                        lines: 0,
                        files: 0,
                        class: "XS",
                    },
                    dimensions: vec![],
                    skipped: vec![],
                },
                project: ProjectView {
                    files: 3,
                    lines: 12_500,
                    tokens: 40_000,
                    formats: vec![
                        FormatLines {
                            format: "typescript".to_string(),
                            lines: 9000,
                        },
                        FormatLines {
                            format: "evil\nformat".to_string(),
                            lines: 3500,
                        },
                    ],
                    largest_files: vec![LargeFile {
                        path: "src/big.ts".to_string(),
                        format: "typescript".to_string(),
                        lines: 400,
                        tokens: 2000,
                        bytes: 2048,
                    }],
                },
                duplication: DuplicationView {
                    percentage: 12.5,
                    clones: 4,
                    exact: 3,
                    renamed: 0,
                    similar: 1,
                    formats: vec![FormatDuplication {
                        format: "typescript".to_string(),
                        percentage: 12.5,
                        duplicated_lines: 50,
                        clones: 4,
                    }],
                },
                complexity: ComplexityView {
                    total: 120,
                    mean: 40.0,
                    files: vec![ComplexFile {
                        path: "src/hairy\nfake row.ts".to_string(),
                        complexity: 42,
                        lines: 300,
                        bytes: 4096,
                    }],
                },
                dead_code: Some(DeadCodeView {
                    percentage: 2.5,
                    findings: 2,
                    files: 1,
                    by_category: vec![CategoryFindings {
                        category: "unused-export",
                        count: 2,
                    }],
                    largest: vec![
                        LargeFinding {
                            category: "unused-export",
                            path: "src/a.ts".to_string(),
                            name: "helper".to_string(),
                            line: 4,
                            lines: 30,
                        },
                        LargeFinding {
                            category: "unused-file",
                            path: "src/orphan.ts".to_string(),
                            name: String::new(),
                            line: 1,
                            lines: 20,
                        },
                    ],
                }),
            };
            let style = cpd_reporter::shared::Style::new(true);
            print_dashboard(&view, 5, Duration::from_millis(3), &style);
        }
        other => panic!("unknown scenario {other}"),
    }
}

// ============================================================================
// console
// ============================================================================

#[test]
fn console_names_each_clone_with_its_two_locations_and_size() {
    let out = capture("console");
    let lines = lines(&out);
    assert_eq!(lines[0], "Clone found (javascript)");
    assert!(
        lines[1].starts_with(" - ") && lines[1].ends_with("a.js [2:1 - 5:14] (4 lines, 50 tokens)"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].starts_with("   ") && lines[2].ends_with("b.js [7:1 - 10:14]"),
        "{}",
        lines[2]
    );
    assert!(!out.contains('\x1b'), "--no-colors: {out:?}");
    assert!(has_line(&out, "Found 1 clones."), "{out}");
}

#[test]
fn console_prints_the_statistics_table() {
    let out = capture("console");
    let row = |label: &str| {
        out.lines()
            .find(|l| l.split('│').nth(1).is_some_and(|c| c.trim() == label))
            .unwrap_or_else(|| panic!("no {label} row in\n{out}"))
            .split('│')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let header = row("Format");
    assert_eq!(
        header,
        [
            "Format",
            "Files analyzed",
            "Total lines",
            "Total tokens",
            "Clones found",
            "Duplicated lines",
            "Duplicated tokens"
        ]
    );
    let expected = ["2", "100", "500", "1", "20 (20.00%)", "100 (20.00%)"];
    assert_eq!(row("javascript")[1..], expected);
    assert_eq!(row("Total:")[1..], expected);
}

#[test]
fn console_colours_its_output_unless_told_not_to() {
    let out = capture("console-colors");
    assert!(out.contains("\x1b["), "colours on by default: {out:?}");
    // Colour codes wrap the text but leave it intact.
    assert!(out.contains("Clone found (javascript)"), "{out:?}");
}

#[test]
fn console_without_clones_says_so() {
    let out = capture("console-empty");
    assert!(has_line(&out, "No duplicates found."), "{out}");
    assert!(has_line(&out, "Found 0 clones."), "{out}");
    assert!(!out.contains("Clone found"), "{out}");
}

#[test]
fn console_marks_clones_new_to_the_baseline() {
    let out = capture("console-new");
    assert!(has_line(&out, "Clone found (javascript) [NEW]"), "{out}");
    assert!(has_line(&out, "Found 1 clones (1 new)."), "{out}");
}

#[test]
#[ignore = "known bug: console reporters print file names raw, so control characters (ANSI escapes) in a name reach the terminal even with --no-colors"]
fn console_never_passes_control_characters_from_file_names_to_the_terminal() {
    let out = capture("console-hostile-name");
    assert!(out.contains("Clone found"), "{out}");
    assert!(!out.contains('\x1b'), "raw ESC in the output: {out:?}");
}

// ============================================================================
// console-full
// ============================================================================

#[test]
fn console_full_shows_the_code_of_both_fragments() {
    let out = capture("console-full");
    let lines = lines(&out);
    assert_eq!(lines[0], "Clone found (javascript)");
    assert!(lines[1].ends_with("a.js [2:1 - 5:14]"), "{}", lines[1]);
    assert!(lines[2].ends_with("b.js [7:1 - 10:14]"), "{}", lines[2]);
    let snippet: Vec<&str> = lines[3..11].to_vec();
    assert_eq!(
        snippet,
        [
            "   2 │ const v2 = 2;",
            "   3 │ const v3 = 3;",
            "   4 │ const v4 = 4;",
            "   5 │ const v5 = 5;",
            "   7 │ const v7 = 7;",
            "   8 │ const v8 = 8;",
            "   9 │ const v9 = 9;",
            "  10 │ const v10 = 10;",
        ]
    );
}

#[test]
fn console_full_cuts_long_snippets_at_twenty_lines() {
    let out = capture("console-full-long");
    let shown: Vec<&str> = out.lines().filter(|l| l.contains(" │ const v")).collect();
    assert_eq!(shown.len(), 40, "20 lines of each fragment: {out}");
    assert!(shown.contains(&"  20 │ const v20 = 20;"));
    assert!(!out.contains("const v21 ="), "{out}");
    assert_eq!(
        out.lines().filter(|l| *l == "     … 5 more lines").count(),
        2,
        "{out}"
    );
}

/// The `--blame` rows of console-full output, split into their columns:
/// `[line a, author a, marker, line b, author b, code]`.
fn blame_rows(out: &str) -> Vec<Vec<String>> {
    out.lines()
        .filter(|l| l.contains(" %02 "))
        .map(|l| l.split(" %02 ").map(|c| c.trim().to_string()).collect())
        .collect()
}

#[test]
fn console_full_blame_puts_authors_beside_each_line() {
    let out = capture("console-full-blame");
    let rows = blame_rows(&out);
    assert_eq!(
        rows,
        [
            ["2", "Alice", "==", "6", "Alice", "const v6 = 6;"],
            ["3", "Alice", "<=", "7", "Bob", "const v7 = 7;"],
            ["4", "Carol", "<=", "8", "", "const v8 = 8;"],
        ],
        "{out}"
    );
    // --blame replaces the plain snippet rather than adding to it.
    assert!(!out.contains(" │ const"), "{out}");
}

#[test]
fn console_full_blame_cuts_long_snippets_at_twenty_lines() {
    let out = capture("console-full-blame-long");
    let rows = blame_rows(&out);
    assert_eq!(rows.len(), 20, "{out}");
    assert_eq!(rows[19][0], "21");
    assert_eq!(rows[19][3], "25");
    assert!(has_line(&out, "     … 3 more lines"), "{out}");
}

#[test]
fn console_full_blame_skips_the_snippet_when_a_file_is_gone() {
    let out = capture("console-full-blame-missing-file");
    assert!(blame_rows(&out).is_empty(), "{out}");
    assert!(
        out.contains("gone.js [7:1 - 10:14]"),
        "the locations stay: {out}"
    );
    assert!(has_line(&out, "Found 1 clones."), "{out}");
}

#[test]
fn console_full_blame_without_a_blame_map_leaves_authors_empty() {
    let out = capture("console-full-blame-on-fragments");
    let rows = blame_rows(&out);
    assert_eq!(
        rows,
        [
            ["2", "", "<=", "2", "", "const v2 = 2;"],
            ["3", "", "<=", "3", "", "const v3 = 3;"],
        ],
        "{out}"
    );
}

// ============================================================================
// xcode, ai, silent
// ============================================================================

#[test]
fn xcode_prints_one_warning_per_clone_in_xcode_syntax() {
    let out = capture("xcode");
    let lines = lines(&out);
    assert_eq!(lines.len(), 2, "{out}");
    let (location, message) = lines[0].split_once(": warning: ").expect(lines[0]);
    assert!(location.ends_with("a.js:2:0"), "{location}");
    assert!(
        message.starts_with("Found 3 lines (2-5) duplicated on file ")
            && message.ends_with("b.js (7-10)"),
        "{message}"
    );
    assert_eq!(lines[1], "Found 1 clones.");
}

#[test]
fn ai_prints_compact_clone_lines_and_a_total() {
    let out = capture("ai");
    let lines = lines(&out);
    assert_eq!(lines[0], "Clones:");
    // The shared folder is written once, then the two file names.
    assert!(lines[1].ends_with("/ a.js:2-5 ~ b.js:7-10"), "{}", lines[1]);
    assert_eq!(lines[2], "---");
    assert_eq!(lines[3], "1 clones · 20.0% duplication");
}

#[test]
fn silent_prints_only_the_summary_sentence() {
    let out = capture("silent");
    assert_eq!(
        lines(&out),
        [
            "Duplications detection: Found 1 exact clones with 20(20.00%) duplicated lines in 2 (1 formats) files."
        ]
    );
}

// ============================================================================
// dead code: console and console-full
// ============================================================================

#[test]
fn dead_code_console_groups_findings_by_category() {
    let out = capture("deadcode-console");
    let titles: Vec<&str> = out.lines().filter(|l| l.starts_with("Unused ")).collect();
    assert_eq!(
        titles,
        [
            "Unused files (1)",
            "Unused exports (1)",
            "Unused members (1)"
        ],
        "{out}"
    );
    assert!(
        has_line(&out, " - src/orphan.js  high 85%  24 lines"),
        "a whole file is named by its path: {out}"
    );
    assert!(
        has_line(
            &out,
            " - function src/api.js:20:1 inner (exported as outer)  low 40%  16 lines"
        ),
        "{out}"
    );
    assert!(
        has_line(
            &out,
            " - function src/api.js:3:1 Widget.render  certain 100%  2 lines"
        ),
        "{out}"
    );
    assert!(
        out.contains("   ↳ "),
        "reasons below uncertain findings: {out}"
    );
    assert!(
        !out.contains(" │ "),
        "the plain console shows no code: {out}"
    );
}

#[test]
fn dead_code_console_trailer_counts_and_lists_unparsed_files() {
    let out = capture("deadcode-console");
    assert!(
        has_line(
            &out,
            "Found 3 dead code findings in 12 files (5.0% of 600 lines)."
        ),
        "{out}"
    );
    assert!(
        has_line(
            &out,
            "12 file(s) could not be parsed; their references are unknown:"
        ),
        "{out}"
    );
    let listed = out
        .lines()
        .filter(|l| l.starts_with("   - broken/"))
        .count();
    assert_eq!(listed, 10, "{out}");
    assert!(
        has_line(&out, "   ... and 2 more (see the JSON report)"),
        "{out}"
    );
    assert!(has_line(&out, "Done in 1.50s"), "{out}");
}

#[test]
fn dead_code_console_full_shows_the_code_of_each_declaration() {
    let out = capture("deadcode-console-full");
    let code: Vec<String> = out
        .lines()
        .filter(|l| l.contains("const v"))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    // `inner` spans lines 20-35: ten lines are shown, then an ellipsis;
    // `render` spans 3-4; the unused file gets no snippet.
    let mut expected: Vec<String> = (20..=29)
        .map(|i| format!("{i} const v{i} = {i};"))
        .collect();
    expected.extend((3..=4).map(|i| format!("{i} const v{i} = {i};")));
    assert_eq!(code, expected, "{out}");
    assert_eq!(
        out.lines().filter(|l| l.trim() == "...").count(),
        1,
        "{out}"
    );
}

#[test]
fn dead_code_console_full_survives_a_file_that_is_gone() {
    let out = capture("deadcode-console-full-gone");
    assert!(!out.contains("const v"), "{out}");
    assert!(out.contains("Found 3 dead code findings"), "{out}");
}

#[test]
fn dead_code_console_without_findings_says_so() {
    let out = capture("deadcode-console-empty");
    assert_eq!(
        out.lines().filter(|l| *l == "No dead code found.").count(),
        2,
        "console and console-full: {out}"
    );
    assert!(
        has_line(&out, "Analyzed 12 files, 140 declarations, 2 entry points."),
        "{out}"
    );
    assert!(has_line(&out, "Done in 12ms"), "{out}");
}

// ============================================================================
// dashboard
// ============================================================================

/// The rows of the table under `title` in dashboard output, columns split on
/// runs of two or more spaces.
fn dashboard_table(out: &str, title: &str) -> Vec<Vec<String>> {
    out.lines()
        .skip_while(|l| l.trim() != title)
        .skip(2) // the title and the header row
        .take_while(|l| l.starts_with("    "))
        .map(|l| {
            l.split("  ")
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string)
                .collect()
        })
        .collect()
}

#[test]
fn dashboard_prints_every_section_with_its_numbers() {
    let out = capture("dashboard");
    for section in ["Project", "Duplication", "Complexity", "Dead code"] {
        assert!(
            out.lines()
                .any(|l| l.starts_with(&format!("── {section} ─"))),
            "{section}: {out}"
        );
    }
    assert!(
        has_line(&out, "  3 files · 12.5K lines · 40.0K tokens · 2 formats"),
        "{out}"
    );
    assert!(
        has_line(
            &out,
            "  12.50% duplicated lines · 4 clones (3 exact · 1 similar)"
        ),
        "{out}"
    );
    assert!(has_line(&out, "  120 total · 40.0 mean per file"), "{out}");
    assert!(
        has_line(&out, "  2.50% unused lines · 2 findings in 1 file"),
        "{out}"
    );
    assert!(has_line(&out, "  2 unused-export"), "{out}");
    assert!(
        out.lines().last().unwrap().starts_with("time: 3.0"),
        "{out}"
    );

    assert_eq!(
        dashboard_table(&out, "Largest code files:")[0][..2],
        ["400", "2000"]
    );
    assert_eq!(
        dashboard_table(&out, "By format:"),
        [["12.5", "50", "4", "typescript"]]
    );
    assert_eq!(
        dashboard_table(&out, "Largest findings:"),
        [
            ["30", "unused-export", "src/a.ts:4 helper"],
            ["20", "unused-file", "src/orphan.ts"]
        ]
    );
}

#[test]
fn dashboard_keeps_newlines_in_names_from_forging_rows() {
    let out = capture("dashboard");
    assert!(
        has_line(&out, "  largest: typescript 9.0K, evil format 3.5K lines"),
        "{out}"
    );
    let complex = dashboard_table(&out, "Most complex files:");
    assert_eq!(complex.len(), 1, "one file, one row: {out}");
    assert_eq!(complex[0][0], "42");
    assert_eq!(complex[0].last().unwrap(), "src/hairy fake row.ts");
}
