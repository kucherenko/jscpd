//! Which files hold tests, by the naming conventions of the languages
//! jscpd compares: `--similarity` leaves them out, and `--compare` and
//! `--semantic` measure tests and code apart.

use std::path::{Component, Path};

/// Folders that hold tests, compared without case.
const TEST_DIRS: &[&str] = &["test", "tests", "__tests__", "spec", "specs"];

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

/// `tests`, `__tests__`, `src/test`, Android's `androidTest`, and the test
/// targets of Xcode and .NET: `MyAppTests`, `MyApp.UITests`, `MyApp.Tests`.
/// A .NET or Eclipse test project is named after its assembly or bundle, and
/// the marker can sit among its dotted parts: `MyApp.Tests.Integration`,
/// `Lucene.Net.Tests.Analysis.Common`, `org.eclipse.jdt.core.tests.model`.
/// There only the plural counts: `MSTest.TestAdapter` and
/// `Microsoft.NET.Test.Sdk` are code.
fn is_test_dir(dir: &str) -> bool {
    let lower = dir.to_ascii_lowercase();
    TEST_DIRS.contains(&lower.as_str())
        || ["Tests", "Test"]
            .iter()
            .any(|suffix| dir.len() > suffix.len() && dir.ends_with(suffix))
        || dir.split('.').any(|part| {
            part.eq_ignore_ascii_case("tests") || (part.len() > 5 && part.ends_with("Tests"))
        })
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
        // CartTests.swift, CartTests.cs: a capitalized plural after another
        // word. The singular `CartTest.java` and `CartSpec.scala` are left
        // to their folders (`src/test/`): as a name alone they would take
        // `ABTest.java` and `OpenApiSpec.ts` for tests.
        || (stem.len() > 5 && stem.ends_with("Tests"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_by_the_conventions_of_each_language() {
        for path in [
            "billing/app_test.go",
            "billing/test_app.py",
            "billing/app_test.py",
            "src/app.test.ts",
            "src/Cart.spec.tsx",
            "src/test/java/CartTest.java",
            "src/test/scala/CartSpec.scala",
            "app/src/androidTest/kotlin/CartTests.kt",
            "MyAppTests/CartTests.swift",
            "MyApp.Tests/CartTests.cs",
            "MyApp.Tests.Integration/CartFlow.cs",
            "src/Lucene.Net.Tests.Analysis.Common/Analysis/Ar/TestArabicAnalyzer.cs",
            "src/MyApp.UnitTests.Core/CartFacts.cs",
            "org.eclipse.jdt.core.tests.model/src/ModelFixture.java",
            "spec/cart_spec.rb",
            "lib/__tests__/copy.js",
            "tests/copy.rs",
            "src/cart/tests.rs",
        ]
        .iter()
        .map(|p| format!("side/{p}"))
        {
            assert!(is_test_path(Path::new(&path)), "{path}");
        }
        for path in [
            "side/src/contest.rs",
            "side/src/Request.java",
            "side/src/Contest.kt",
            "side/src/testing_utils.py",
            "side/fixtures/app.py",
            "side/src/latest.ts",
            "side/src/OpenApiSpec.ts",
            "side/src/ABTest.java",
            "side/src/LoadTest.kt",
            "side/src/MyApp.Contest/Entry.cs",
            "side/src/Contests.Api/Entry.cs",
            "side/src/Adapter/MSTest.TestAdapter/Execution.cs",
            "side/src/Microsoft.NET.Test.Sdk/Runner.cs",
            "side/src/SpeedTest.Net/Client.cs",
        ] {
            assert!(!is_test_path(Path::new(path)), "{path}");
        }
        // The compared folder's own name counts.
        assert!(is_test_path(Path::new("tests/copy.rs")));
    }
}
