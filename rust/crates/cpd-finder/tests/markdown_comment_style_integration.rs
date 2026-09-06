// Regression test: Markdown prose had no arm in `comment_style`, so it fell to
// the C style. A `/*` that prose can hold — inside a glob such as `docs/**`, or
// in a code span — then opened a block comment that never closed, and every
// later clone in that file went unreported.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;
use std::path::PathBuf;

fn fixtures(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{dir}"))
}

#[test]
fn prose_after_a_glob_is_still_matched() {
    let result = run(&RunConfig {
        paths: vec![fixtures("markdown_comment_style")],
        min_tokens: 20,
        min_lines: 2,
        mode: Mode::Mild,
        no_gitignore: true,
        ..Default::default()
    })
    .unwrap();

    assert!(
        result.clones.iter().any(|c| c.format == "markdown"),
        "the shared paragraph must be reported, even though `docs/**` sits above it"
    );
}
