// Regression test: plain text had no arm in `comment_style`, so it fell to the
// C style, as Markdown did before it. A `/*` inside a glob such as `docs/**`
// then opened a block comment that never closed, and every later clone in that
// file went unreported.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;

mod common;
use common::fixtures;

#[test]
fn text_after_a_glob_is_still_matched() {
    let result = run(&RunConfig {
        paths: vec![fixtures("txt_comment_style")],
        min_tokens: 20,
        min_lines: 2,
        mode: Mode::Mild,
        no_gitignore: true,
        ..Default::default()
    })
    .unwrap();

    assert!(
        result.clones.iter().any(|c| c.format == "txt"),
        "the shared paragraph must be reported, even though `docs/**` sits above it"
    );
}
