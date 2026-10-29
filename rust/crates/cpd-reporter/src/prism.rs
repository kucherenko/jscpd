// prism.rs — Prism (https://prismjs.com, MIT) for the HTML report: the page
// highlights the code of a clone in the browser, and carries only the Prism
// languages its clones are in.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

/// The Prism core and every Prism language, bundled by
/// rust/scripts/vendor-prism.py.
const BUNDLE: &str = include_str!("../assets/prism/prism.min.js");
const LICENSE: &str = include_str!("../assets/prism/LICENSE");
const MARKER: &str = "\n/*prism-component ";

struct Component {
    /// The languages this one needs.
    require: Vec<&'static str>,
    /// The languages that load before this one when the page has them too.
    after: Vec<&'static str>,
    code: &'static str,
}

struct Bundle {
    core: &'static str,
    components: HashMap<&'static str, Component>,
}

static PRISM: LazyLock<Bundle> = LazyLock::new(|| {
    let mut parts = BUNDLE.split(MARKER);
    let core = parts.next().unwrap_or_default();
    let names = |field: &'static str| -> Vec<&'static str> {
        if field == "-" {
            Vec::new()
        } else {
            field.split(',').collect()
        }
    };
    let components = parts
        .filter_map(|part| {
            let (head, code) = part.split_once("*/\n")?;
            let mut fields = head.split(' ');
            let id = fields.next()?;
            let require = names(fields.next()?);
            let after = names(fields.next()?);
            Some((
                id,
                Component {
                    require,
                    after,
                    code: code.trim_end(),
                },
            ))
        })
        .collect();
    Bundle { core, components }
});

/// The Prism language that highlights `format`, None when Prism has none.
/// jscpd's format names come from Prism's, so most map to themselves.
pub fn language(format: &str) -> Option<&'static str> {
    let id = match format {
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

/// Put `id` into `order` after the wanted languages it loads after.
fn load_order(
    id: &'static str,
    wanted: &BTreeSet<&'static str>,
    seen: &mut HashSet<&'static str>,
    order: &mut Vec<&'static str>,
) {
    if !seen.insert(id) {
        return;
    }
    let component = &PRISM.components[id];
    for dep in component.require.iter().chain(&component.after) {
        if wanted.contains(dep) {
            load_order(dep, wanted, seen, order);
        }
    }
    order.push(id);
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
            ("txt", None),
            ("comments", None),
        ] {
            assert_eq!(language(format), lang, "{format}");
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
