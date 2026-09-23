// Regression tests for https://github.com/kucherenko/jscpd/issues/1090:
// a code block embedded in markdown keeps the host file's line numbers, and
// the statistics used to read those numbers as if the block were contiguous.
// Prose sitting between two blocks was counted as duplicated code, in the
// numerator and in the denominator both.
//
// The fixture is two identical guides. Each holds two ten-line `ts` blocks,
// at lines 17-26 and 43-52, with prose in between and around them: 20 lines
// of TypeScript per file, 40 across the pair.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;

mod common;
use common::fixtures;

const CODE_LINES_PER_FILE: u64 = 20;

fn config(formats: Vec<String>) -> RunConfig {
    RunConfig {
        paths: vec![fixtures("embedded_statistics")],
        min_tokens: 20,
        min_lines: 2,
        mode: Mode::Mild,
        formats,
        no_gitignore: true,
        ..Default::default()
    }
}

#[test]
fn a_formats_line_total_counts_only_its_own_blocks() {
    let result = run(&config(vec![])).unwrap();
    let ts = &result.statistics.formats["typescript"];

    assert_eq!(
        ts.lines,
        CODE_LINES_PER_FILE * 2,
        "the typescript total is the two blocks of each guide, not the guides"
    );
    assert_eq!(ts.sources, 2, "one synthetic source per guide");
}

#[test]
fn prose_between_two_blocks_is_not_duplicated_code() {
    let result = run(&config(vec![])).unwrap();
    let clone = result
        .clones
        .iter()
        .find(|c| c.format == "typescript")
        .expect("the two guides share their typescript blocks");

    let span = clone.fragment_a.end.line - clone.fragment_a.start.line + 1;
    assert_eq!(
        span, 36,
        "the fragment reaches from the first block to the last, prose included"
    );
    assert_eq!(
        clone.matched_lines(),
        CODE_LINES_PER_FILE,
        "but only the block lines are duplicated code"
    );
}

#[test]
fn the_host_format_keeps_counting_whole_files() {
    let result = run(&config(vec![])).unwrap();
    let md = &result.statistics.formats["markdown"];

    // Both guides are 66 lines; the fix touches embedded sources only, so the
    // format the file is actually written in is unaffected.
    assert_eq!(md.lines, 132);
}

#[test]
fn filtering_to_the_embedded_format_gives_the_same_line_total() {
    // The gap this issue came from: `-f typescript` drops the guides entirely,
    // so before the fix the filtered run and the full run disagreed wildly
    // about how many TypeScript lines the project has. They no longer do --
    // filtering leaves nothing of that format behind to count.
    let full = run(&config(vec![])).unwrap();
    let filtered = run(&config(vec!["typescript".to_string()])).unwrap();

    assert_eq!(full.statistics.formats["typescript"].lines, 40);
    assert!(
        filtered.statistics.formats.is_empty(),
        "no plain .ts file in the fixture, so the filtered run finds nothing"
    );
}
