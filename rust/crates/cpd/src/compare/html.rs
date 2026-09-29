//! The `html` reporter of `--compare`: one page, `jscpd-compare.html`, that
//! draws the two sides as dependency graphs facing each other, with the pairs
//! bridging them, next to the progress per folder, the similarity of the
//! pairs, the functions ready to port and a table of every file.
//!
//! The page works offline: its styles and script are in the file, and the
//! data it draws is a JSON document in a `<script type="application/json">`
//! element. The data is compact on purpose, a list of files, functions,
//! pairs and calls referring to each other by index, and the page derives
//! every view (folders, files or functions; code or tests) from it.

use super::describe;
use cpd_semantic::compare::{Comparison, Level};
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
            let by_name = u8::from(pair.matched_by.as_str() == "name");
            (pair.a, pair.b, similarity, level, by_name)
        })
        .collect();
    let page = Page {
        version: env!("CARGO_PKG_VERSION"),
        model,
        sides: paths,
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
