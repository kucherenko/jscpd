// Regression tests for https://github.com/kucherenko/jscpd/issues/883:
// prose-only Markdown files (no code fences) were silently dropped — they
// never counted as analyzed and `-f markdown` matched 0 files.

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;
use std::path::PathBuf;

fn fixtures(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{dir}"))
}

fn config(paths: Vec<PathBuf>, formats: Vec<String>) -> RunConfig {
    RunConfig {
        paths,
        min_tokens: 20,
        min_lines: 2,
        mode: Mode::Mild,
        formats,
        no_gitignore: true,
        ..Default::default()
    }
}

#[test]
fn autodetect_analyzes_prose_markdown_files() {
    let result = run(&config(vec![fixtures("markdown_prose")], vec![])).unwrap();

    let md_sources: Vec<_> = result
        .sources
        .iter()
        .filter(|s| s.format == "markdown")
        .collect();
    assert_eq!(
        md_sources.len(),
        2,
        "both prose-only .md files must be analyzed during autodetect"
    );
    assert!(
        md_sources.iter().all(|s| !s.tokens.is_empty()),
        "prose-only markdown must produce display tokens"
    );

    assert!(
        result.clones.iter().any(|c| c.format == "markdown"),
        "identical prose-only .md pair must be reported as a markdown clone"
    );
    assert!(
        result.clones.iter().any(|c| c.format == "txt"),
        "the identical .txt pair must still be detected alongside markdown"
    );
}

#[test]
fn markdown_format_filter_matches_prose_markdown_files() {
    let result = run(&config(
        vec![fixtures("markdown_prose")],
        vec!["markdown".to_string()],
    ))
    .unwrap();

    assert_eq!(
        result
            .sources
            .iter()
            .filter(|s| s.format == "markdown")
            .count(),
        2,
        "-f markdown must analyze prose-only .md files"
    );
    assert!(
        result.sources.iter().all(|s| s.format == "markdown"),
        "-f markdown must not pick up the .txt files"
    );
    assert!(
        result.clones.iter().any(|c| c.format == "markdown"),
        "identical prose-only .md pair must be reported as a clone with -f markdown"
    );
}

#[test]
fn embedded_code_fences_detected_as_sub_format_clones() {
    let result = run(&config(vec![fixtures("markdown_embedded")], vec![])).unwrap();

    assert_eq!(
        result
            .sources
            .iter()
            .filter(|s| s.format == "markdown")
            .count(),
        2,
        "both fence-bearing .md files must be analyzed"
    );
    assert!(
        result.clones.iter().any(|c| c.format == "javascript"),
        "identical javascript fences in two .md files must be reported as a javascript clone"
    );
    assert!(
        !result.clones.iter().any(|c| c.format == "markdown"),
        "differing prose must not be reported as a markdown clone"
    );
}
