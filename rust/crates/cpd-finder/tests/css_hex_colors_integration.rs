// Regression test: the generic tokenizer read a hex color such as `#fee3c909`
// as `#` and a name, which `--ignore-identifiers` renames. A list of colors
// then matched itself one line down.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;

mod common;
use common::fixtures;

#[test]
fn a_list_of_colors_is_not_a_renamed_clone_of_itself() {
    let result = run(&RunConfig {
        paths: vec![fixtures("css_hex_colors")],
        min_tokens: 50,
        min_lines: 5,
        mode: Mode::Mild,
        no_gitignore: true,
        ignore_identifiers: true,
        ..Default::default()
    })
    .unwrap();

    assert!(result.clones.is_empty(), "{:?}", result.clones);
}
