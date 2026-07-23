use cpd_finder::orchestrate::{RunConfig, run};
use cpd_tokenizer::tokenizer::Mode;
use std::{fs, path::PathBuf};

fn setup_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-cross-formats-{}", suffix));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Typed TypeScript source and its untyped JavaScript twin. The bodies are
/// identical apart from TypeScript-only syntax.
fn typed_ts() -> &'static str {
    r#"interface Totals {
    sum: number;
    max: number;
}

function computeTotals(values: number[], threshold: number): Totals {
    let sum: number = 0;
    let max: number = 0;
    for (const value of values) {
        sum = sum + value;
        if (value > max) {
            max = value;
        }
    }
    if (sum > threshold) {
        sum = sum - threshold;
    }
    return { sum: sum, max: max } as Totals;
}
"#
}

fn untyped_js() -> &'static str {
    r#"function computeTotals(values, threshold) {
    let sum = 0;
    let max = 0;
    for (const value of values) {
        sum = sum + value;
        if (value > max) {
            max = value;
        }
    }
    if (sum > threshold) {
        sum = sum - threshold;
    }
    return { sum: sum, max: max };
}
"#
}

fn write_pair(dir: &PathBuf) {
    fs::write(dir.join("a.ts"), typed_ts()).unwrap();
    fs::write(dir.join("b.js"), untyped_js()).unwrap();
}

fn config(paths: Vec<PathBuf>, cross_formats: Vec<Vec<String>>) -> RunConfig {
    RunConfig {
        paths,
        min_tokens: 20,
        min_lines: 1,
        mode: Mode::Mild,
        cross_formats,
        ..Default::default()
    }
}

#[test]
fn ts_js_pair_not_detected_by_default() {
    let dir = setup_temp_dir("default");
    write_pair(&dir);
    let result = run(&config(vec![dir], vec![])).unwrap();
    assert_eq!(
        result.clones.len(),
        0,
        "without --cross-formats, TS and JS stay in isolated pools"
    );
}

#[test]
fn ts_js_pair_detected_with_cross_formats() {
    let dir = setup_temp_dir("enabled");
    write_pair(&dir);
    let groups = vec![vec!["javascript".to_string(), "typescript".to_string()]];
    let result = run(&config(vec![dir], groups)).unwrap();
    assert_eq!(
        result.clones.len(),
        1,
        "cross-formats group must match the TS file against its JS twin"
    );

    let clone = &result.clones[0];
    let ids = [
        clone.fragment_a.source_id.as_str(),
        clone.fragment_b.source_id.as_str(),
    ];
    assert!(
        ids.iter().any(|id| id.ends_with("a.ts")),
        "one side is the TS file"
    );
    assert!(
        ids.iter().any(|id| id.ends_with("b.js")),
        "one side is the JS file"
    );

    // Positions reference the ORIGINAL sources: the TS function body starts
    // after the interface block (line 6 of a.ts), the JS twin at line 1.
    let (ts_frag, js_frag) = if clone.fragment_a.source_id.ends_with("a.ts") {
        (&clone.fragment_a, &clone.fragment_b)
    } else {
        (&clone.fragment_b, &clone.fragment_a)
    };
    assert!(
        ts_frag.start.line >= 6,
        "TS fragment must start after the stripped interface block, got line {}",
        ts_frag.start.line
    );
    assert!(
        js_frag.start.line <= 2,
        "JS fragment must start at the top of the file, got line {}",
        js_frag.start.line
    );
}

#[test]
fn unrelated_format_stays_isolated() {
    let dir = setup_temp_dir("unrelated");
    write_pair(&dir);
    // Same content in a Python file: same tokens would match, but python is
    // not part of the cross-formats group.
    fs::write(dir.join("c.py"), untyped_js()).unwrap();
    let groups = vec![vec!["javascript".to_string(), "typescript".to_string()]];
    let result = run(&config(vec![dir], groups)).unwrap();
    for clone in &result.clones {
        for id in [&clone.fragment_a.source_id, &clone.fragment_b.source_id] {
            assert!(
                !id.ends_with("c.py"),
                "python file must not join the javascript/typescript pool"
            );
        }
    }
}
