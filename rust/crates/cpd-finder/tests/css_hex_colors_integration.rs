// Regression test: the generic tokenizer read a hex color such as `#fee3c909`
// as `#` and a name, which `--ignore-identifiers` renames. A list of colors
// then matched itself one line down.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;

mod common;
use common::fixtures;

fn scan(ignore_literals: bool) -> Vec<cpd_core::models::CpdClone> {
    run(&RunConfig {
        paths: vec![fixtures("css_hex_colors")],
        min_tokens: 50,
        min_lines: 5,
        mode: Mode::Mild,
        no_gitignore: true,
        ignore_identifiers: true,
        ignore_literals,
        ..Default::default()
    })
    .unwrap()
    .clones
}

#[test]
fn a_list_of_colors_is_not_a_renamed_clone_of_itself() {
    let clones = scan(false);
    assert!(clones.is_empty(), "{clones:?}");
}

/// With `--ignore-literals` the colors fold like numbers, and the list is a
/// clone of itself again: the scan reads the file, and colors are literals.
#[test]
fn colors_fold_under_ignore_literals() {
    assert!(!scan(true).is_empty());
}
