//! The `html` reporter of `--compare`: one page, `jscpd-compare.html`, with
//! two views of the comparison. The map draws the two sides as dependency
//! graphs facing each other, with the pairs bridging them; the table lists
//! the same bridges as rows. Below both: the functions ready to port, the
//! similarity of the pairs and the progress per folder.
//!
//! The page works offline: its styles and script are in the file, and the
//! data it draws is a JSON document in a `<script type="application/json">`
//! element. The data is compact on purpose, a list of files, functions,
//! pairs and calls referring to each other by index, and the page derives
//! every view (map or table; folders, files or functions; code, tests or
//! both) from it.

use super::describe;
use cpd_semantic::compare::{Comparison, Level, MatchedBy};
use cpd_semantic::search::UnitSource;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// The page, with `/*DATA*/null` where the data goes.
const TEMPLATE: &str = include_str!("page.html");

/// A function's flags in [`Page::functions`].
const COUNTED: u8 = 1;
const TEST: u8 = 2;
const READY: u8 = 4;

/// What the page draws. Indexes refer into the lists: a function names its
/// file, a pair and a call name their functions.
#[derive(Debug, Serialize)]
struct Page<'a> {
    version: &'a str,
    model: &'a str,
    /// The two paths as given on the command line.
    sides: [&'a str; 2],
    /// Whether each side is a single file rather than a folder: its files
    /// are then named by their file name alone.
    #[serde(rename = "sideIsFile")]
    side_is_file: [bool; 2],
    /// `(side, path relative to the side's folder)`.
    files: Vec<(usize, String)>,
    /// `(file, name, first line, last line, flags)`; flags add up
    /// [`COUNTED`], [`TEST`] and [`READY`].
    functions: Vec<(usize, &'a str, u32, u32, u8)>,
    /// `(function on side 0, function on side 1, similarity, level, found
    /// by name)`; the level is 0 for low, 1 for medium, 2 for high.
    pairs: Vec<(usize, usize, f64, u8, u8)>,
    /// `(caller, callee)`, within one side.
    calls: &'a [(usize, usize)],
}

/// The page for `comparison` of `sides`, whose folders are `roots` and
/// whose paths were given as `paths`.
pub(super) fn page(
    paths: [&str; 2],
    roots: &[PathBuf; 2],
    sides: &[Vec<UnitSource>; 2],
    comparison: &Comparison,
    model: &str,
) -> String {
    let root_is_file = [&roots[0], &roots[1]].map(|root| root.is_file());
    let ready = comparison.ready();
    let mut file_ids: HashMap<(usize, String), usize> = HashMap::new();
    let mut files = Vec::new();
    let functions = comparison
        .functions
        .iter()
        .enumerate()
        .map(|(index, f)| {
            let source = &sides[f.side][f.source];
            let unit = &source.units[f.unit];
            let described = describe(&roots[f.side], root_is_file[f.side], &source.id, unit);
            let next = files.len();
            let file = *file_ids
                .entry((f.side, described.file.clone()))
                .or_insert_with(|| {
                    files.push((f.side, described.file.clone()));
                    next
                });
            let flags = u8::from(f.counted) * COUNTED
                + u8::from(f.test) * TEST
                + u8::from(ready[index]) * READY;
            (
                file,
                unit.name.as_str(),
                unit.start.line,
                unit.end.line,
                flags,
            )
        })
        .collect();
    let pairs = comparison
        .pairs
        .iter()
        .map(|pair| {
            let level = match pair.level {
                Level::Low => 0,
                Level::Medium => 1,
                Level::High => 2,
            };
            let similarity = (f64::from(pair.similarity) * 1000.0).round() / 1000.0;
            let by_name = u8::from(matches!(pair.matched_by, MatchedBy::Name));
            (pair.a, pair.b, similarity, level, by_name)
        })
        .collect();
    let page = Page {
        version: env!("CARGO_PKG_VERSION"),
        model,
        sides: paths,
        side_is_file: root_is_file,
        files,
        functions,
        pairs,
        calls: &comparison.calls,
    };
    render(&page)
}

/// The template with `page` in place of its data. Every `<` of the JSON
/// becomes `<`, which JSON reads back the same, so a name holding
/// `</script>` cannot end the element early.
fn render(page: &Page<'_>) -> String {
    let json = serde_json::to_string(page)
        .unwrap_or_else(|_| "null".to_string())
        .replace('<', "\\u003c");
    TEMPLATE.replacen("/*DATA*/null", &json, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Page<'static> {
        Page {
            version: "0.0.0",
            model: "stand-in",
            sides: ["java/", "python/"],
            side_is_file: [false, false],
            files: vec![(0, "QrCode.java".into()), (1, "qrcodegen.py".into())],
            functions: vec![
                (0, "drawVersion", 10, 20, COUNTED),
                (0, "</script><b>", 30, 40, COUNTED | READY),
                (1, "_draw_version", 5, 12, COUNTED),
            ],
            pairs: vec![(0, 2, 0.87, 2, 0)],
            calls: &[(1, 0)],
        }
    }

    /// The JSON the page embeds.
    fn embedded(html: &str) -> serde_json::Value {
        let start = html
            .find("<script type=\"application/json\" id=\"data\">")
            .unwrap();
        let rest = &html[start..];
        let body = &rest[rest.find('>').unwrap() + 1..rest.find("</script>").unwrap()];
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn the_page_embeds_its_data_once() {
        let html = render(&sample());
        assert!(!html.contains("/*DATA*/"), "the placeholder is replaced");
        let data = embedded(&html);
        assert_eq!(data["sides"][1], "python/");
        assert_eq!(data["functions"][0][1], "drawVersion");
        assert_eq!(data["pairs"][0][2], 0.87);
        assert_eq!(data["calls"][0], serde_json::json!([1, 0]));
    }

    #[test]
    fn a_name_cannot_close_the_data_element() {
        let html = render(&sample());
        let data = embedded(&html);
        assert_eq!(data["functions"][1][1], "</script><b>");
        assert_eq!(data["functions"][1][4], 5, "counted and ready");
    }

    #[test]
    fn the_page_rounds_shares_like_the_console() {
        // `share_percent` in compare/mod.rs computes the same expression,
        // so the map's header and the console print the same share: 60 of
        // 147 is 41% in both.
        assert!(TEMPLATE.contains("Math.round((part * 100) / whole)"));
    }

    /// The page's `function name(…) { … }`, to its closing brace.
    fn page_function(name: &str) -> &'static str {
        let start = TEMPLATE.find(&format!("function {name}(")).expect(name);
        let mut depth = 0;
        for (at, c) in TEMPLATE[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 1 => return &TEMPLATE[start..=start + at],
                '}' => depth -= 1,
                _ => {}
            }
        }
        panic!("{name} has no end")
    }

    #[test]
    fn folder_labels_keep_the_folders_that_tell_them_apart() {
        // Cut at their end, the labels of one module's folders all read
        // `lucene/analysis/common/src/…`. Cut in their middle they keep the
        // folders that tell them apart, `en/ext` and `de/ext` when both end
        // in `ext`, the start of a package's name when it is that, and the
        // separators and leading `/` of the path.
        let script = format!(
            "{}\n{}\n{}\nconst label = (paths) => {{ const d = tailDepths(paths); \
             return paths.map((p) => shortPath(p, d.get(p), 28)); }};\n\
             console.log(JSON.stringify(JSON.parse(process.argv[1]).map(label)));",
            page_function("folders"),
            page_function("tailDepths"),
            page_function("shortPath"),
        );
        let lucene = "lucene/analysis/common/src/java/org/apache/lucene/analysis";
        let paths = serde_json::json!([
            [
                format!("{lucene}/en/ext"),
                format!("{lucene}/de/ext"),
                format!("{lucene}/cjk")
            ],
            [
                "packages/very-long-package-name/src/utils",
                "packages/another-long-package-name/src/utils"
            ],
            [
                "src\\main\\java\\org\\example\\project\\module\\x",
                "/abs/path/to/some/deep/folder/name/here",
                "src/app",
                "a-folder-name-far-too-long-for-the-label"
            ],
        ]);
        let output = match std::process::Command::new("node")
            .args(["-e", &script, "--"])
            .arg(paths.to_string())
            .output()
        {
            Ok(output) => output,
            // The page's script runs in node, which CI has; a machine
            // without it skips the check.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipped: node is not installed");
                return;
            }
            Err(e) => panic!("node: {e}"),
        };
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let labels: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            labels,
            serde_json::json!([
                [
                    "lucene/analysis/…/en/ext",
                    "lucene/analysis/…/de/ext",
                    "lucene/analysis/common/…/cjk"
                ],
                [
                    "packages/very-long-pa…/utils",
                    "packages/another-long…/utils"
                ],
                [
                    "src\\main\\java\\org\\…\\x",
                    "/abs/path/to/some/…/here",
                    "src/app",
                    "…-far-too-long-for-the-label"
                ],
            ])
        );
        assert!(
            TEMPLATE.contains("shortPath(n.label, depths[n.side].get(n.label), 28)"),
            "on the map"
        );
    }

    #[test]
    fn the_template_is_one_self_contained_page() {
        assert!(TEMPLATE.starts_with("<!doctype html>"));
        assert_eq!(TEMPLATE.matches("/*DATA*/null").count(), 1);
        // Nothing is fetched: no script, style sheet, font or image from
        // elsewhere (the SVG namespace URI is a name, not a request).
        for external in [
            "<script src",
            "<link",
            "@import",
            "src=\"http",
            "href=\"http",
        ] {
            assert!(!TEMPLATE.contains(external), "no external {external}");
        }
        // A url() points into the page only, at a clip path or a gradient.
        for (at, _) in TEMPLATE.match_indices("url(") {
            assert!(
                TEMPLATE[at + 4..].starts_with('#'),
                "url( at {at} leaves the page"
            );
        }
    }
}
