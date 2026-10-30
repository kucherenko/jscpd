// prism.rs — Prism (https://prismjs.com, MIT) for the HTML report: the page
// highlights the code of a clone in the browser, and carries only the Prism
// languages its clones are in.

use cpd_tokenizer::formats::resolve_format;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

/// The Prism core and every Prism language, bundled by
/// rust/scripts/vendor-prism.py.
const BUNDLE: &str = include_str!("../assets/prism/prism.min.js");
const LICENSE: &str = include_str!("../assets/prism/LICENSE");
const MARKER: &str = "\n/*prism-component ";

struct Component<'a> {
    /// The languages this one needs.
    require: Vec<&'a str>,
    /// The languages that load before this one when the page has them too.
    after: Vec<&'a str>,
    code: &'a str,
}

struct Bundle<'a> {
    core: &'a str,
    components: HashMap<&'a str, Component<'a>>,
}

static PRISM: LazyLock<Bundle<'static>> = LazyLock::new(|| Bundle::parse(BUNDLE));

impl<'a> Bundle<'a> {
    /// Split `bundle` at its marker lines. Git on Windows checks the bundle
    /// out with CRLF line endings, and it reads the same then.
    fn parse(bundle: &'a str) -> Self {
        let mut parts = bundle.split(MARKER);
        let core = parts.next().unwrap_or_default().trim_end();
        let names = |field: &'a str| -> Vec<&'a str> {
            if field == "-" {
                Vec::new()
            } else {
                field.split(',').collect()
            }
        };
        let components = parts
            .filter_map(|part| {
                let (head, code) = part.split_once("*/")?;
                let mut fields = head.split(' ');
                let id = fields.next()?;
                let require = names(fields.next()?);
                let after = names(fields.next()?);
                Some((
                    id,
                    Component {
                        require,
                        after,
                        code: code.trim(),
                    },
                ))
            })
            .collect();
        Self { core, components }
    }
}

/// The Prism language that highlights `format`, None when Prism has none.
/// jscpd's format names come from Prism's, so most map to themselves. A
/// block can go by another name, which resolves to its format first: `html`
/// is the markup of a component, and a `<script lang="coffee">` keeps its
/// `lang`.
pub fn language(format: &str) -> Option<&'static str> {
    let id = match resolve_format(format).unwrap_or(format) {
        "c-header" => "c",
        "cpp-header" => "cpp",
        "razor" => "cshtml",
        "vue" | "svelte" | "astro" | "cfml" => "markup",
        other => other,
    };
    PRISM.components.get_key_value(id).map(|(id, _)| *id)
}

/// The script for a page with code in `languages`: Prism's license, its core,
/// and each language after the ones it needs. Empty without a language.
pub fn script<'a>(languages: impl IntoIterator<Item = &'a str>) -> String {
    let prism = &*PRISM;
    let mut wanted = BTreeSet::new();
    let mut stack: Vec<&str> = languages.into_iter().collect();
    while let Some(id) = stack.pop() {
        if let Some((id, component)) = prism.components.get_key_value(id)
            && wanted.insert(*id)
        {
            stack.extend(&component.require);
        }
    }
    if wanted.is_empty() {
        return String::new();
    }

    let mut order = Vec::new();
    let mut seen = HashSet::new();
    for id in &wanted {
        load_order(id, &wanted, &mut seen, &mut order);
    }

    let mut script = format!("/*!\n{LICENSE}*/\n{}", prism.core);
    for id in order {
        script.push('\n');
        script.push_str(prism.components[id].code);
    }
    script
}

/// Put the wanted languages `id` loads after into `order`, then `id` when it
/// is wanted. As in Prism's own loader, that is every language `id` can reach,
/// through the ones the page leaves out too: haml reads markup as it loads,
/// but names it only through languages that need it, such as markdown.
fn load_order(
    id: &'static str,
    wanted: &BTreeSet<&'static str>,
    seen: &mut HashSet<&'static str>,
    order: &mut Vec<&'static str>,
) {
    if !seen.insert(id) {
        return;
    }
    let Some(component) = PRISM.components.get(id) else {
        return;
    };
    for dep in component.require.iter().chain(&component.after) {
        load_order(dep, wanted, seen, order);
    }
    if wanted.contains(id) {
        order.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where each component's code starts in `script`, to check the order.
    fn position(script: &str, id: &str) -> usize {
        let code = PRISM.components[id].code;
        script
            .find(code)
            .unwrap_or_else(|| panic!("{id} is not in the script"))
    }

    #[test]
    fn the_bundle_holds_the_core_and_the_languages() {
        assert!(PRISM.core.contains("Prism"), "the core comes first");
        assert!(PRISM.components.len() > 250, "{}", PRISM.components.len());
        for id in ["javascript", "typescript", "python", "rust", "markup"] {
            assert!(PRISM.components.contains_key(id), "{id}");
        }
    }

    #[test]
    fn formats_map_to_prism_languages() {
        for (format, lang) in [
            ("javascript", Some("javascript")),
            ("tsx", Some("tsx")),
            ("python", Some("python")),
            ("plant-uml", Some("plant-uml")),
            ("c-header", Some("c")),
            ("cpp-header", Some("cpp")),
            ("razor", Some("cshtml")),
            ("vue", Some("markup")),
            ("html", Some("markup")),
            ("coffee", Some("coffeescript")),
            ("txt", None),
            ("comments", None),
        ] {
            assert_eq!(language(format), lang, "{format}");
        }
    }

    #[test]
    fn every_format_prism_can_highlight_has_its_language() {
        // The formats of jscpd, and `html`, the format of a component's
        // markup.
        let mut none: Vec<&str> = cpd_tokenizer::formats::list_formats()
            .into_iter()
            .chain(["html"])
            .filter(|format| language(format).is_none())
            .collect();
        none.sort_unstable();
        assert_eq!(none, ["comments", "keymap", "txt"]);
    }

    #[test]
    fn a_crlf_checkout_reads_the_same_bundle() {
        // Git on Windows checks the bundle out with CRLF line endings.
        let crlf = BUNDLE.replace('\n', "\r\n");
        let bundle = Bundle::parse(&crlf);
        assert_eq!(bundle.components.len(), PRISM.components.len());
        for (id, component) in &PRISM.components {
            let read = &bundle.components[id];
            assert_eq!(read.require, component.require, "{id}");
            assert_eq!(read.after, component.after, "{id}");
            assert_eq!(read.code, component.code, "{id}");
        }
    }

    #[test]
    fn a_language_loads_after_the_ones_it_needs() {
        let script = script(["tsx"]);
        for (first, then) in [
            ("clike", "javascript"),
            ("markup", "jsx"),
            ("javascript", "jsx"),
            ("javascript", "typescript"),
            ("jsx", "tsx"),
            ("typescript", "tsx"),
        ] {
            assert!(
                position(&script, first) < position(&script, then),
                "{first} before {then}"
            );
        }
        assert!(script.starts_with("/*!\nMIT LICENSE"), "license first");
        assert!(
            !script.contains(PRISM.components["python"].code),
            "only the languages of the page"
        );
    }

    #[test]
    fn a_language_loads_after_the_ones_it_modifies_when_the_page_has_them() {
        // css adds inline styles to markup, so markup must be there first.
        let script = script(["css", "markup"]);
        assert!(position(&script, "markup") < position(&script, "css"));
    }

    #[test]
    fn a_language_loads_after_what_it_reads_through_languages_it_names() {
        // haml reads markup as it loads, and names it only through markdown
        // and the like, which need it. Prism loads markup first, so does the
        // page, though it carries no markdown.
        let script = script(["haml", "markup"]);
        assert!(position(&script, "markup") < position(&script, "haml"));
    }

    #[test]
    fn no_language_no_script() {
        assert_eq!(script([]), "");
        assert_eq!(script(["no-such-language"]), "");
    }

    #[test]
    fn the_script_never_ends_its_script_element() {
        let all: Vec<&str> = PRISM.components.keys().copied().collect();
        assert!(!script(all).to_lowercase().contains("</script"));
    }
}
