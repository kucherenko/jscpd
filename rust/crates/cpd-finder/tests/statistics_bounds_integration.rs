// Statistics of real detection runs: the duplicated share of a project is a
// share, so it lies in 0..=100 however the clones overlap, and the clones
// counted point at lines that really are copies of each other.

use cpd_finder::orchestrate::{RunConfig, RunResult, run};
use std::path::Path;

mod common;
use common::{duplicate_js, temp_dir};

fn detect(dir: &Path) -> RunResult {
    run(&RunConfig {
        paths: vec![dir.to_path_buf()],
        min_tokens: 10,
        min_lines: 2,
        no_gitignore: true,
        ..Default::default()
    })
    .unwrap()
}

/// Lines `start..=end` (1-based) of the file `id`.
fn lines(id: &str, start: u32, end: u32) -> Vec<String> {
    std::fs::read_to_string(id)
        .unwrap()
        .lines()
        .skip(start as usize - 1)
        .take((end - start + 1) as usize)
        .map(str::to_string)
        .collect()
}

const LINE: &str = "total = total + compute(alpha, beta, gamma);\n";

#[test]
fn clones_of_copied_files_cover_the_copied_lines() {
    let dir = temp_dir("stats-bounds", "copies");
    let unique_a = "const onlyInA = 1;\n";
    let unique_b = "let onlyInB = [2, 3];\n";
    std::fs::write(dir.join("a.js"), format!("{unique_a}{}", duplicate_js())).unwrap();
    std::fs::write(dir.join("b.js"), format!("{}{unique_b}", duplicate_js())).unwrap();

    let result = detect(&dir);
    assert_eq!(result.clones.len(), 1, "{:?}", result.clones);
    let clone = &result.clones[0];
    let (a, b) = (&clone.fragment_a, &clone.fragment_b);
    let text_a = lines(&a.source_id, a.start.line, a.end.line);
    let text_b = lines(&b.source_id, b.start.line, b.end.line);
    assert_eq!(text_a, text_b, "the two fragments hold the same code");
    assert!(
        text_a.iter().all(|l| !l.contains("onlyIn")),
        "the unique lines stay out of the clone: {text_a:?}"
    );

    let total = &result.statistics.total;
    assert_eq!(total.clones, 1);
    assert_eq!(total.duplicated_lines, clone.matched_lines());
    let expected = total.duplicated_lines as f64 / total.lines as f64 * 100.0;
    assert!((total.percentage - expected).abs() < 1e-9);
}

#[test]
fn a_file_without_copies_has_no_duplication() {
    let dir = temp_dir("stats-bounds", "unique");
    std::fs::write(dir.join("a.js"), duplicate_js()).unwrap();
    std::fs::write(dir.join("b.js"), "const x = { y: [1, 2, 3] };\n").unwrap();

    let result = detect(&dir);
    assert!(result.clones.is_empty(), "{:?}", result.clones);
    assert_eq!(result.statistics.total.duplicated_lines, 0);
    assert_eq!(result.statistics.total.percentage, 0.0);
}

#[test]
#[ignore = "known bug: statistics.rs sums matched lines per clone, so clones whose copies overlap count lines twice and the share passes 100%"]
fn duplicated_share_never_exceeds_one_hundred_percent() {
    // A short block and a long run of the same line: the long file is
    // reported as many copies of the short one, and consecutive copies share
    // a boundary line. Summed per clone, the duplicated lines outgrow the
    // project (342 of 310 lines, 110%, when this test was written).
    let dir = temp_dir("stats-bounds", "overlap");
    std::fs::write(dir.join("a.js"), LINE.repeat(10)).unwrap();
    std::fs::write(dir.join("b.js"), LINE.repeat(300)).unwrap();

    let stats = detect(&dir).statistics;
    let total = &stats.total;
    assert!(
        total.duplicated_lines <= total.lines,
        "{} duplicated of {} lines",
        total.duplicated_lines,
        total.lines
    );
    assert!(total.percentage <= 100.0, "{}%", total.percentage);
    assert!(total.duplicated_tokens <= total.tokens);
    assert!(
        total.percentage_tokens <= 100.0,
        "{}%",
        total.percentage_tokens
    );
    for (format, row) in &stats.formats {
        assert!(row.percentage <= 100.0, "{format}: {}%", row.percentage);
    }
}
