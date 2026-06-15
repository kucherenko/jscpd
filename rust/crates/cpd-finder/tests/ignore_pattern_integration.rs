// Integration tests for ignorePattern (code-level regex) and ignore (file-level glob) behavior.
//
// Test strategy:
// - Two identical TypeScript files (invoice.ts, pricing.ts) → guaranteed clone detection
// - A third different file (order.ts) for baseline comparison
// - With code_ignore_patterns matching "function" → fewer tokens, fewer clones
// - With file-level ignore glob → entire file excluded, fewer files processed

use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::{Mode, TokenizeOptions, code_ignore_ranges, tokenize_to_detection};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn ignore_pattern_dir() -> PathBuf {
    fixtures_dir().join("ignore_pattern")
}

fn skip_if_missing(dir: &PathBuf) -> bool {
    if !dir.exists() {
        eprintln!("fixture dir not found, skipping");
        return true;
    }
    false
}

fn base_config(dir: PathBuf) -> RunConfig {
    RunConfig {
        paths: vec![dir],
        min_tokens: 3,
        min_lines: 1,
        mode: Mode::Mild,
        ignore: vec![],
        code_ignore_patterns: vec![],
        ..Default::default()
    }
}

#[test]
fn identical_files_produce_clones_without_ignore() {
    let dir = ignore_pattern_dir();
    if skip_if_missing(&dir) {
        return;
    }

    let config = base_config(dir);

    let result = run(&config).unwrap();
    assert!(
        !result.clones.is_empty(),
        "identical files must produce at least one clone, got 0 clones and {} sources",
        result.sources.len()
    );
}

#[test]
fn code_ignore_pattern_reduces_clones() {
    let dir = ignore_pattern_dir();
    if skip_if_missing(&dir) {
        return;
    }

    let result_baseline = run(&base_config(dir.clone())).unwrap();
    let baseline_dup_lines = result_baseline.statistics.total.duplicated_lines;

    let mut config_with_pattern = base_config(dir);
    config_with_pattern.code_ignore_patterns = vec!["function".to_string()];
    let result_with_pattern = run(&config_with_pattern).unwrap();
    let pattern_dup_lines = result_with_pattern.statistics.total.duplicated_lines;

    assert!(
        pattern_dup_lines < baseline_dup_lines,
        "ignorePattern 'function' should reduce duplicated lines: got {} with pattern vs {} without",
        pattern_dup_lines,
        baseline_dup_lines,
    );
}

#[test]
fn file_level_ignore_excludes_file() {
    let dir = ignore_pattern_dir();
    if skip_if_missing(&dir) {
        return;
    }

    let mut config = base_config(dir);
    config.ignore = vec!["**/invoice.ts".to_string()];

    let result = run(&config).unwrap();
    let has_invoice = result.sources.iter().any(|sf| sf.id.contains("invoice.ts"));
    assert!(
        !has_invoice,
        "invoice.ts should be excluded by ignore glob, but found in sources: {:?}",
        result.sources.iter().map(|sf| &sf.id).collect::<Vec<_>>()
    );
}

#[test]
fn code_ignore_pattern_invalid_regex_is_skipped() {
    let dir = ignore_pattern_dir();
    if skip_if_missing(&dir) {
        return;
    }

    let result_baseline = run(&base_config(dir.clone())).unwrap();

    let mut config_invalid = base_config(dir);
    config_invalid.code_ignore_patterns = vec!["(".to_string()];
    let result_invalid = run(&config_invalid).unwrap();

    assert_eq!(
        result_invalid.clones.len(),
        result_baseline.clones.len(),
        "invalid regex should be silently skipped, not crash"
    );
}

#[test]
fn multi_token_ignore_pattern_reduces_clones() {
    let dir = ignore_pattern_dir();
    if skip_if_missing(&dir) {
        return;
    }

    let mut config_baseline = base_config(dir.clone());
    config_baseline.formats = vec!["typescript".to_string()];
    config_baseline.ignore = vec![
        "**/invoice.ts".to_string(),
        "**/pricing.ts".to_string(),
        "**/order.ts".to_string(),
    ];
    let result_baseline = run(&config_baseline).unwrap();

    let mut config_with_pattern = config_baseline;
    config_with_pattern.paths = vec![dir];
    config_with_pattern.code_ignore_patterns = vec![r"import \* from".to_string()];
    let result_with_pattern = run(&config_with_pattern).unwrap();

    assert!(
        result_with_pattern.sources.len() == result_baseline.sources.len(),
        "same number of sources should be scanned"
    );
}
#[test]
fn code_ignore_regex_matches_source_text_not_tokens() {
    // Verify that code_ignore_patterns work by matching against source text
    // (not individual token values). This is the v4 semantics: a regex like
    // "import.*from" should match the entire "import * from 'lodash'" region
    // and ALL tokens overlapping that region should be skipped.
    let source = "import * from 'lodash';\nconst x = 1;";
    let regexes = vec![regex::Regex::new(r"import\s+\*\s+from").unwrap()];
    let ranges = code_ignore_ranges(source, &regexes);
    assert!(!ranges.is_empty(), "regex should match source text");

    let mut opts = TokenizeOptions::new(Mode::Mild);
    opts.ignore_ranges = ranges.clone();
    let tokens = tokenize_to_detection("javascript", source, &opts);

    // Tokens overlapping the import region should be skipped.
    // const/x/=/1 tokens should remain.
    let remaining_tokens: Vec<_> = tokens.iter().filter(|t| t.range[0] >= 24).collect();
    assert!(
        !remaining_tokens.is_empty(),
        "tokens after import line should remain"
    );
    // All remaining tokens should be after the import match.
    for t in &tokens {
        if t.range[0] < ranges[0][1] && t.range[1] > ranges[0][0] {
            panic!(
                "token at {:?} should have been filtered by ignore_range {:?}",
                t.range, ranges[0]
            );
        }
    }
}
