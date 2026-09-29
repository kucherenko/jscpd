//! Which functions are tests, so that `--compare` measures tests and code
//! apart and pairs a test only with a test.
//!
//! Most languages keep tests in files of their own, named by convention
//! ([`is_test_path`]): `*_test.go`, `test_*.py`, `*.test.ts`, `*Test.java`,
//! `*Tests.swift`, `*_spec.rb`, or folders such as `tests/`, `__tests__/`,
//! `src/test/` and `MyAppTests/`. Two kinds of test live among the code
//! ([`inline_test`]): Rust tests, in a `#[cfg(test)]` module or under a
//! `#[test]` attribute, and JavaScript and TypeScript test cases,
//! `it('title', () => …)`, which Vitest runs from source files too.

use cpd_core::models::Token;
use cpd_tokenizer::functions::TEST_CASE_CALLS;
use std::path::{Component, Path};

/// Folders that hold tests, compared without case.
const TEST_DIRS: &[&str] = &[
    "test",
    "tests",
    "__tests__",
    "spec",
    "specs",
    "androidtest",
    "uitests",
];

/// Whether `path` names a test file by the conventions of the languages
/// jscpd compares. `path` starts at the compared folder itself
/// (`tests/copy.rs` when the folder is `tests/`), so a folder given on the
/// command line counts as well as the folders below it.
pub fn is_test_path(path: &Path) -> bool {
    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let Some((file, dirs)) = parts.split_last() else {
        return false;
    };
    dirs.iter().any(|dir| is_test_dir(dir)) || is_test_file(file)
}

/// `tests`, `__tests__`, `src/test`, and the test targets of Xcode and
/// .NET: `MyAppTests`, `MyApp.Tests`, `MyApp.UITests`.
fn is_test_dir(dir: &str) -> bool {
    let lower = dir.to_ascii_lowercase();
    TEST_DIRS.contains(&lower.as_str())
        || ["Tests", ".Tests", "Test", ".Test"]
            .iter()
            .any(|suffix| dir.len() > suffix.len() && dir.ends_with(suffix))
}

fn is_test_file(file: &str) -> bool {
    let lower = file.to_ascii_lowercase();
    let stem = file.split('.').next().unwrap_or(file);
    let lower_stem = stem.to_ascii_lowercase();
    // app.test.ts, app.spec.jsx, app_test.go, test_app.py, app_spec.rb,
    // and the `tests.rs` a `#[cfg(test)] mod tests;` declares.
    lower.contains(".test.")
        || lower.contains(".spec.")
        || lower_stem.starts_with("test_")
        || lower_stem.ends_with("_test")
        || lower_stem.ends_with("_tests")
        || lower_stem.ends_with("_spec")
        || lower_stem == "tests"
        // CartTest.java, CartTests.swift, CartSpec.scala: a capitalized
        // suffix after another word, so `Contest` and `Request` stay code.
        || ["Test", "Tests", "Spec"]
            .iter()
            .any(|suffix| stem.len() > suffix.len() && stem.ends_with(suffix))
}

/// Whether the function of `grammar` found at `head..` in `code` is a test
/// that lives among the code: a Rust function in a `#[cfg(test)]` module
/// (see [`rust_test_modules`]) or under a test attribute, or a JavaScript
/// test case. `start` is where the Rust item's first keyword is, after its
/// attributes.
pub(crate) fn inline_test(
    grammar: &str,
    code: &str,
    head: usize,
    start: usize,
    rust_modules: &[(usize, usize)],
) -> bool {
    match grammar {
        "rust" => {
            rust_modules
                .iter()
                .any(|&(from, to)| from <= start && start < to)
                || has_test_attribute(code, start)
        }
        "oxc" => {
            let rest = code.get(head..).unwrap_or_default();
            let callee: &str = rest
                .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                .next()
                .unwrap_or_default();
            let after = rest[callee.len()..].trim_start();
            TEST_CASE_CALLS.contains(&callee) && (after.starts_with('(') || after.starts_with('.'))
        }
        _ => false,
    }
}

/// Whether the attributes above the Rust item at `start` include a test
/// attribute: `#[test]`, or one whose path ends in `test` (`#[tokio::test]`,
/// `#[sqlx::test]`), or `#[rstest]` and `#[test_case(…)]`.
fn has_test_attribute(code: &str, start: usize) -> bool {
    let before = code.get(..start).unwrap_or_default();
    // The item's own line up to its first keyword (`pub async fn`), then
    // the lines above it while they hold attributes or doc comments.
    let mut lines = before.rsplit('\n');
    let _ = lines.next();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let Some(attribute) = line.strip_prefix("#[") else {
            return false;
        };
        let path = attribute
            .split(['(', ']'])
            .next()
            .unwrap_or_default()
            .trim();
        let last = path.rsplit("::").next().unwrap_or(path);
        if last == "test" || last.starts_with("test_") || last == "rstest" {
            return true;
        }
    }
    false
}

/// The byte ranges of the bodies of `#[cfg(test)]` modules in Rust `code`
/// (`#[cfg(all(test, feature = "x"))]` included), found by matching the
/// brace tokens after `mod name`.
pub(crate) fn rust_test_modules(code: &str, tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find("#[cfg(") {
        let attr_start = from + at;
        from = attr_start + 6;
        let Some(close) = code[attr_start..].find(")]") else {
            break;
        };
        let condition = &code[attr_start + 6..attr_start + close];
        let mentions_test = condition
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| word == "test");
        if !mentions_test {
            continue;
        }
        // Other attributes and `pub`/`pub(crate)` may stand between the
        // attribute and `mod name {`.
        let after = attr_start + close + 2;
        let Some(brace) = module_brace(&code[after..]).map(|b| after + b) else {
            continue;
        };
        if let Some(end) = matching_brace(tokens, brace) {
            ranges.push((brace, end));
            from = end;
        }
    }
    ranges
}

/// The offset of the `{` of `mod name {` at the start of `rest`, after
/// attributes and visibility; `None` when `rest` starts with something else
/// (`mod tests;`, a function).
fn module_brace(rest: &str) -> Option<usize> {
    let mut at = 0;
    loop {
        let trimmed = rest[at..].trim_start();
        at = rest.len() - trimmed.len();
        if trimmed.starts_with("#[") {
            at += trimmed.find(']')? + 1;
        } else if let Some(after) = trimmed.strip_prefix("pub(") {
            at += 4 + after.find(')')? + 1;
        } else if trimmed.starts_with("pub ") {
            at += 4;
        } else {
            break;
        }
    }
    let trimmed = rest[at..].strip_prefix("mod ")?;
    let brace = trimmed.find(['{', ';'])?;
    let name = trimmed[..brace].trim();
    (trimmed.as_bytes()[brace] == b'{' && !name.is_empty() && !name.contains(char::is_whitespace))
        .then_some(at + 4 + brace)
}

/// The end offset of the brace token that closes the `{` at `open`.
fn matching_brace(tokens: &[Token], open: usize) -> Option<usize> {
    let first = tokens
        .iter()
        .position(|t| t.start.offset as usize == open)?;
    let mut depth = 0usize;
    for token in &tokens[first..] {
        match token.value.as_str() {
            "{" => depth += 1,
            "}" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(token.end.offset as usize);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_tokenizer::tokenizer::{Mode, tokenize};

    #[test]
    fn test_files_by_the_conventions_of_each_language() {
        for path in [
            "billing/app_test.go",
            "billing/test_app.py",
            "billing/app_test.py",
            "src/app.test.ts",
            "src/Cart.spec.tsx",
            "src/test/java/CartTest.java",
            "app/src/androidTest/kotlin/CartTests.kt",
            "MyAppTests/CartTests.swift",
            "MyApp.Tests/CartTests.cs",
            "spec/cart_spec.rb",
            "lib/__tests__/copy.js",
            "tests/copy.rs",
            "src/cart/tests.rs",
            "src/CartSpec.scala",
            "tests",
        ]
        .iter()
        .map(|p| format!("side/{p}"))
        {
            if path == "side/tests" {
                continue;
            }
            assert!(is_test_path(Path::new(&path)), "{path}");
        }
        for path in [
            "side/src/contest.rs",
            "side/src/Request.java",
            "side/src/Contest.kt",
            "side/src/testing_utils.py",
            "side/fixtures/app.py",
            "side/src/latest.ts",
        ] {
            assert!(!is_test_path(Path::new(path)), "{path}");
        }
        // The compared folder's own name counts.
        assert!(is_test_path(Path::new("tests/copy.rs")));
    }

    fn rust(code: &str) -> (Vec<Token>, Vec<(usize, usize)>) {
        let tokens = tokenize("rust", code, Mode::Weak);
        let modules = rust_test_modules(code, &tokens);
        (tokens, modules)
    }

    #[test]
    fn rust_tests_in_cfg_test_modules_and_under_test_attributes() {
        let code = "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n\n#[tokio::test]\nasync fn adds_async() {\n    assert_eq!(add(1, 2), 3);\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    fn helper() -> u32 { 1 }\n\n    #[test]\n    fn adds() {\n        let s = \"{ not a brace }\";\n        assert_eq!(add(helper(), 2), 3);\n    }\n}\n\npub fn after() {}\n";
        let (_, modules) = rust(code);
        assert_eq!(modules.len(), 1);
        let at = |needle: &str| code.find(needle).unwrap();
        let test = |needle: &str| {
            let start = at(needle);
            inline_test("rust", code, start, start, &modules)
        };
        assert!(!test("pub fn add"));
        assert!(test("async fn adds_async"));
        assert!(test("fn helper"), "a helper inside the test module");
        assert!(test("fn adds()"));
        assert!(
            !test("pub fn after"),
            "the module ends at its closing brace"
        );
    }

    #[test]
    fn a_cfg_test_module_declared_elsewhere_or_cfg_without_test_is_no_range() {
        let (_, none) = rust(
            "#[cfg(test)]\nmod tests;\n\n#[cfg(feature = \"x\")]\nmod extra {\n    fn f() {}\n}\n",
        );
        assert!(none.is_empty());
        let (_, some) =
            rust("#[cfg(all(test, unix))]\npub(crate) mod unix_tests {\n    fn f() {}\n}\n");
        assert_eq!(some.len(), 1);
    }

    #[test]
    fn javascript_test_cases_are_tests_wherever_they_live() {
        let code = "export function add(a, b) { return a + b; }\nit('adds', () => { expect(add(1, 2)).toBe(3); });\ntest.each([[1]])('t %i', (a) => {});\nconst item = { it: 1 };\n";
        let test = |needle: &str| {
            let at = code.find(needle).unwrap();
            inline_test("oxc", code, at, at, &[])
        };
        assert!(!test("export function add"));
        assert!(test("it('adds'"));
        assert!(test("test.each"));
        assert!(!test("item = "));
    }
}
