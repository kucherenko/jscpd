//! The rules of the `boundaries` key, parsed and checked: which code each
//! one protects, where its copies must not show up, and where they may.

use cpd_core::models::{CloneKind, KindFilter, SimilarityMethod};
use globset::{Glob, GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One rule as the config file writes it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawRule {
    name: String,
    code: Vec<String>,
    #[serde(default)]
    not_in: Option<Vec<String>>,
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    thresholds: Thresholds,
    #[serde(default)]
    disable: Vec<String>,
}

/// The kinds of copy a rule looks for: every kind but the ones its
/// `disable` names, by the names of `--kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kinds {
    exact: bool,
    renamed: bool,
    gap: bool,
    ast: bool,
    semantic: bool,
}

impl Kinds {
    /// Every kind but `disable`'s; `similar` stands for `gap` and `ast`.
    /// Fails on a name that is not a kind, and when no kind is left.
    fn without(disable: &[String]) -> Result<Self, String> {
        let mut kinds = Self {
            exact: true,
            renamed: true,
            gap: true,
            ast: true,
            semantic: true,
        };
        for name in disable {
            match name
                .parse::<KindFilter>()
                .map_err(|e| format!("disable: {e}"))?
            {
                KindFilter::Exact => kinds.exact = false,
                KindFilter::Renamed => kinds.renamed = false,
                KindFilter::Similar => (kinds.gap, kinds.ast) = (false, false),
                KindFilter::Gap => kinds.gap = false,
                KindFilter::Ast => kinds.ast = false,
                KindFilter::Semantic => kinds.semantic = false,
            }
        }
        if !(kinds.tokens() || kinds.ast || kinds.semantic) {
            return Err(
                "disable turns off every kind of copy, so the rule would check nothing".to_string(),
            );
        }
        Ok(kinds)
    }

    /// Whether the rule looks for token clones of some kind.
    pub fn tokens(&self) -> bool {
        self.exact || self.renamed || self.gap
    }

    /// Whether the rule looks for clones merged across a gap.
    pub fn gap(&self) -> bool {
        self.gap
    }

    /// Whether the rule looks for functions of the same structure.
    pub fn ast(&self) -> bool {
        self.ast
    }

    /// Whether the rule looks for functions that do the same job, with the
    /// model.
    pub fn semantic(&self) -> bool {
        self.semantic
    }

    /// Whether the rule looks for a copy of this kind, found this way.
    pub fn takes(&self, kind: CloneKind, method: Option<SimilarityMethod>) -> bool {
        match (kind, method) {
            (CloneKind::Exact, _) => self.exact,
            (CloneKind::Renamed, _) => self.renamed,
            (CloneKind::Similar, Some(SimilarityMethod::Gap)) => self.gap,
            (CloneKind::Similar, _) => self.ast,
            (CloneKind::Semantic, _) => self.semantic,
        }
    }
}

/// The bars of one rule, each in place of the project's own (see
/// [`super::check`]): a rule that guards money can look for smaller and
/// looser copies than a clone report does.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Thresholds {
    pub min_tokens: Option<usize>,
    pub min_lines: Option<usize>,
    pub min_nodes: Option<u32>,
    pub similarity: Option<f64>,
    pub semantic_threshold: Option<f32>,
    pub semantic_same_threshold: Option<f32>,
}

/// The rules of a config file, in its order.
#[derive(Debug)]
pub struct Rules {
    /// The folder relative paths of the rules start from, canonical.
    base: PathBuf,
    pub rules: Vec<Rule>,
}

/// One rule, ready to match files.
#[derive(Debug)]
pub struct Rule {
    pub name: String,
    code: Vec<CodeEntry>,
    /// Where copies must not show up; `None` is everywhere outside `code`.
    not_in: Option<Paths>,
    /// Where copies may show up after all.
    allow: Paths,
    pub thresholds: Thresholds,
    /// The kinds of copy the rule looks for.
    pub kinds: Kinds,
}

/// One entry of `code`: files, or the methods of files whose names match.
#[derive(Debug)]
struct CodeEntry {
    files: Paths,
    methods: Option<Methods>,
}

/// The methods of a `code` entry: `path#name` names one, `path#(pattern)`
/// takes every method whose name matches the pattern.
#[derive(Debug)]
pub enum Methods {
    Name(String),
    Pattern(GlobMatcher),
}

impl Methods {
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Methods::Name(own) => own == name,
            Methods::Pattern(pattern) => pattern.is_match(name),
        }
    }
}

/// How much of a file a rule protects.
#[derive(Debug)]
pub enum Protection<'r> {
    /// The whole file.
    Whole,
    /// The methods these entries pick.
    Methods(Vec<&'r Methods>),
}

/// Where a file is, for the patterns of the rules: its path below the
/// rules' folder with `/` between the parts, if it lies there, and its
/// absolute path, for patterns that start at the root.
pub struct Location {
    relative: Option<String>,
    absolute: String,
}

/// Path patterns: relative ones match the path below the rules' folder,
/// absolute ones the absolute path.
#[derive(Debug)]
struct Paths {
    relative: GlobSet,
    absolute: GlobSet,
}

impl Paths {
    /// The patterns of `field`, an error naming the one that does not
    /// compile.
    fn new(patterns: &[String], field: &str) -> Result<Self, String> {
        let mut relative = GlobSetBuilder::new();
        let mut absolute = GlobSetBuilder::new();
        for raw in patterns {
            let pattern = raw.trim().replace('\\', "/");
            let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
            if pattern.is_empty() {
                return Err(format!("{field}: an empty path"));
            }
            let set = match is_absolute(pattern) {
                true => &mut absolute,
                false => &mut relative,
            };
            set.add(path_glob(pattern).map_err(|e| format!("{field}: {e}"))?);
            // A folder written without a pattern stands for everything in
            // it: `packages/auth` is the module.
            if !has_glob_syntax(pattern) {
                set.add(path_glob(&format!("{pattern}/**")).map_err(|e| format!("{field}: {e}"))?);
            }
        }
        let build = |set: GlobSetBuilder| set.build().map_err(|e| format!("{field}: {e}"));
        Ok(Self {
            relative: build(relative)?,
            absolute: build(absolute)?,
        })
    }

    fn matches(&self, location: &Location) -> bool {
        location
            .relative
            .as_deref()
            .is_some_and(|path| self.relative.is_match(path))
            || self.absolute.is_match(&location.absolute)
    }
}

/// A glob over paths: `*` stays in one folder, `**` crosses folders.
fn path_glob(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}

fn has_glob_syntax(pattern: &str) -> bool {
    pattern.contains(['*', '?', '[', '{'])
}

/// `/src`, `C:/src`: a pattern for the absolute path of a file.
fn is_absolute(pattern: &str) -> bool {
    let mut chars = pattern.chars();
    pattern.starts_with('/')
        || matches!(
            (chars.next(), chars.next(), chars.next()),
            (Some(drive), Some(':'), Some('/')) if drive.is_ascii_alphabetic()
        )
}

impl CodeEntry {
    fn parse(entry: &str) -> Result<Self, String> {
        let (path, methods) = split_methods(entry)?;
        Ok(Self {
            files: Paths::new(&[path.to_string()], "code")?,
            methods,
        })
    }
}

/// The path of a `code` entry and its methods: `src/fees.kt#total` names
/// one method, `src/fees.kt#(*Fee)` takes the methods whose names match.
/// A `#` followed by more folders belongs to the path.
fn split_methods(entry: &str) -> Result<(&str, Option<Methods>), String> {
    let entry = entry.trim();
    let Some((path, selector)) = entry.rsplit_once('#') else {
        return Ok((entry, None));
    };
    if selector.contains(['/', '\\']) {
        return Ok((entry, None));
    }
    let example = "e.g. src/fees.kt#total for one method or src/fees.kt#(*Fee) for a pattern";
    if path.trim().is_empty() {
        return Err(format!("code: '{entry}' has no path before '#', {example}"));
    }
    if selector.is_empty() {
        return Err(format!(
            "code: '{entry}' names no method after '#', {example}"
        ));
    }
    let methods = match selector.strip_prefix('(') {
        Some(inner) => {
            let Some(pattern) = inner.strip_suffix(')') else {
                return Err(format!("code: '{entry}' has no ')' after the pattern"));
            };
            if pattern.trim().is_empty() {
                return Err(format!("code: '{entry}' has an empty pattern, {example}"));
            }
            let glob = Glob::new(pattern.trim()).map_err(|e| format!("code: '{entry}': {e}"))?;
            Methods::Pattern(glob.compile_matcher())
        }
        None if selector.contains(['(', ')', '*', '?']) => {
            return Err(format!(
                "code: '{entry}': write a pattern of names in parentheses, {example}"
            ));
        }
        None => Methods::Name(selector.to_string()),
    };
    Ok((path, Some(methods)))
}

impl Rules {
    /// The rules of `raw`, the value of the `boundaries` key, with their
    /// relative paths starting at `base`. Fails on the first rule that is
    /// wrong, naming it: a rule that is not checked must not look as if it
    /// passed.
    pub fn parse(raw: &serde_json::Value, base: &Path) -> Result<Self, String> {
        let serde_json::Value::Array(items) = raw else {
            return Err(
                r#"expected a list of rules, e.g. [{"name": "money", "code": ["backend/billing/**"], "notIn": ["web/**"]}]"#
                    .to_string(),
            );
        };
        let mut rules: Vec<Rule> = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let label = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map_or_else(
                    || format!("rule {}", index + 1),
                    |name| format!("rule '{name}'"),
                );
            let rule = Rule::parse(item).map_err(|e| format!("{label}: {e}"))?;
            if rules.iter().any(|other| other.name == rule.name) {
                return Err(format!("{label}: another rule has the same name"));
            }
            rules.push(rule);
        }
        // `--config jscpd.json` has an empty folder: the working directory.
        let base = match base.as_os_str().is_empty() {
            true => Path::new("."),
            false => base,
        };
        Ok(Self {
            base: std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf()),
            rules,
        })
    }

    /// Where the file at `path`, as the walk found it, is for the patterns.
    pub fn locate(&self, path: &Path) -> Location {
        let slashes = |p: &Path| p.to_string_lossy().replace('\\', "/");
        Location {
            relative: path.strip_prefix(&self.base).ok().map(slashes),
            absolute: slashes(path),
        }
    }
}

impl Rule {
    fn parse(item: &serde_json::Value) -> Result<Self, String> {
        if !item.is_object() {
            return Err(
                r#"expected an object such as {"name": "money", "code": ["backend/billing/**"]}"#
                    .to_string(),
            );
        }
        let raw: RawRule = serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
        if raw.name.trim().is_empty() {
            return Err("the name is empty".to_string());
        }
        if raw.code.is_empty() {
            return Err(
                "code is empty: list the modules, folders, files or methods to protect".to_string(),
            );
        }
        let not_in = match &raw.not_in {
            Some(paths) if paths.is_empty() => {
                return Err(
                    "notIn is empty: list the paths copies must stay out of, or leave notIn out to mean every path outside code"
                        .to_string(),
                );
            }
            Some(paths) => Some(Paths::new(paths, "notIn")?),
            None => None,
        };
        check_thresholds(&raw.thresholds)?;
        Ok(Self {
            name: raw.name.trim().to_string(),
            code: raw
                .code
                .iter()
                .map(|entry| CodeEntry::parse(entry))
                .collect::<Result<_, _>>()?,
            not_in,
            allow: Paths::new(&raw.allow, "allow")?,
            thresholds: raw.thresholds,
            kinds: Kinds::without(&raw.disable)?,
        })
    }

    /// How much of the file at `location` the rule protects; `None` when
    /// none of it.
    pub fn protects(&self, location: &Location) -> Option<Protection<'_>> {
        let mut methods = Vec::new();
        for entry in self.code.iter().filter(|e| e.files.matches(location)) {
            match &entry.methods {
                None => return Some(Protection::Whole),
                Some(m) => methods.push(m),
            }
        }
        (!methods.is_empty()).then_some(Protection::Methods(methods))
    }

    /// Whether a copy of the protected code in the file at `location` breaks
    /// the rule. The rule's own files are never such a place: two copies
    /// inside one module are duplication, not a leak.
    pub fn forbids(&self, location: &Location) -> bool {
        self.protects(location).is_none()
            && self
                .not_in
                .as_ref()
                .is_none_or(|paths| paths.matches(location))
            && !self.allow.matches(location)
    }
}

/// Every bar in its range: a size of at least one, a ratio in (0, 1].
fn check_thresholds(thresholds: &Thresholds) -> Result<(), String> {
    let sizes = [
        ("minTokens", thresholds.min_tokens.map(|v| v as u64)),
        ("minNodes", thresholds.min_nodes.map(u64::from)),
    ];
    for (key, value) in sizes {
        if value == Some(0) {
            return Err(format!("thresholds.{key} must be at least 1"));
        }
    }
    let ratios = [
        ("similarity", thresholds.similarity),
        (
            "semanticThreshold",
            thresholds.semantic_threshold.map(f64::from),
        ),
        (
            "semanticSameThreshold",
            thresholds.semantic_same_threshold.map(f64::from),
        ),
    ];
    for (key, value) in ratios {
        if let Some(value) = value
            && !(value > 0.0 && value <= 1.0)
        {
            return Err(format!(
                "thresholds.{key} is {value}, it must be a number in (0, 1]"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(raw: serde_json::Value) -> Result<Rules, String> {
        Rules::parse(&raw, Path::new("/repo"))
    }

    fn rule(raw: serde_json::Value) -> Rule {
        rules(json!([raw])).unwrap().rules.remove(0)
    }

    fn at(path: &str) -> Location {
        Location {
            relative: Some(path.to_string()),
            absolute: format!("/repo/{path}"),
        }
    }

    #[test]
    fn a_rule_protects_folders_files_and_methods() {
        let money = rule(json!({
            "name": "money",
            "code": [
                "backend/billing/**",
                "backend/core/Money.kt",
                "backend/loans/**/*Payment*.kt",
                "backend/loans/Interest.kt#monthlyPayment",
                "backend/loans/Interest.kt#(*Payments)"
            ],
            "notIn": ["web/**"]
        }));
        assert!(matches!(
            money.protects(&at("backend/billing/Fees.kt")),
            Some(Protection::Whole)
        ));
        assert!(matches!(
            money.protects(&at("backend/core/Money.kt")),
            Some(Protection::Whole)
        ));
        assert!(matches!(
            money.protects(&at("backend/loans/v2/LatePayments.kt")),
            Some(Protection::Whole)
        ));
        let Some(Protection::Methods(methods)) = money.protects(&at("backend/loans/Interest.kt"))
        else {
            panic!("the methods of Interest.kt");
        };
        let picked = |name: &str| methods.iter().any(|m| m.matches(name));
        assert!(picked("monthlyPayment") && picked("latePayments"));
        assert!(!picked("monthlyPayments2") && !picked("rate"));
        assert!(money.protects(&at("backend/core/Rates.kt")).is_none());
    }

    #[test]
    fn a_star_stays_in_its_folder_and_a_bare_folder_takes_everything_in_it() {
        let rule = rule(json!({"name": "auth", "code": ["packages/auth", "lib/*.ts"]}));
        assert!(rule.protects(&at("packages/auth/src/token.ts")).is_some());
        assert!(rule.protects(&at("packages/authz/src/token.ts")).is_none());
        assert!(rule.protects(&at("lib/session.ts")).is_some());
        assert!(rule.protects(&at("lib/deep/session.ts")).is_none());
    }

    #[test]
    fn copies_are_forbidden_in_not_in_or_everywhere_outside_the_code() {
        let money = rule(json!({
            "name": "money",
            "code": ["backend/billing/**"],
            "notIn": ["web/**", "mobile/**"],
            "allow": ["web/e2e/**"]
        }));
        assert!(money.forbids(&at("web/src/cart.ts")));
        assert!(money.forbids(&at("mobile/cart.swift")));
        assert!(!money.forbids(&at("web/e2e/cart.spec.ts")), "allowed");
        assert!(
            !money.forbids(&at("backend/orders/Order.kt")),
            "not in notIn"
        );
        assert!(
            !money.forbids(&at("backend/billing/Fees.kt")),
            "its own code"
        );

        let auth = rule(json!({"name": "auth", "code": ["packages/auth/**"]}));
        assert!(auth.forbids(&at("packages/billing/src/a.ts")));
        assert!(auth.forbids(&at("web/a.ts")));
        assert!(!auth.forbids(&at("packages/auth/src/a.ts")));
    }

    #[test]
    fn a_not_in_that_covers_the_code_still_leaves_the_code_alone() {
        let billing = rule(json!({
            "name": "billing",
            "code": ["backend/billing/**"],
            "notIn": ["backend/**"]
        }));
        assert!(billing.forbids(&at("backend/orders/Order.kt")));
        assert!(!billing.forbids(&at("backend/billing/Fees.kt")));
    }

    #[test]
    fn absolute_patterns_match_absolute_paths() {
        let rule = rule(json!({"name": "abs", "code": ["/repo/backend/**"]}));
        assert!(rule.protects(&at("backend/a.kt")).is_some());
        let outside = Location {
            relative: None,
            absolute: "/elsewhere/backend/a.kt".to_string(),
        };
        assert!(rule.protects(&outside).is_none());
        assert!(rule.forbids(&outside));
    }

    #[test]
    fn a_path_below_the_base_is_relative_with_slashes() {
        let rules = rules(json!([{"name": "x", "code": ["a/**"]}])).unwrap();
        let inside = rules.locate(&rules.base.join("web").join("cart.ts"));
        assert_eq!(inside.relative.as_deref(), Some("web/cart.ts"));
        let outside = rules.locate(Path::new("/elsewhere/cart.ts"));
        assert_eq!(outside.relative, None);
        assert_eq!(outside.absolute, "/elsewhere/cart.ts");
    }

    #[test]
    fn mistakes_stop_the_run_and_name_the_rule() {
        let error = |raw: serde_json::Value| rules(raw).unwrap_err();
        assert!(error(json!({"name": "x"})).starts_with("expected a list of rules"));
        assert_eq!(
            error(json!([{"name": "money", "code": ["a/**"], "notin": ["web/**"]}])),
            "rule 'money': unknown field `notin`, expected one of `name`, `code`, `notIn`, `allow`, `thresholds`, `disable`"
        );
        assert_eq!(
            error(json!([{"code": ["a/**"]}])),
            "rule 1: missing field `name`"
        );
        assert_eq!(
            error(json!([{"name": "x", "code": []}])),
            "rule 'x': code is empty: list the modules, folders, files or methods to protect"
        );
        assert!(
            error(json!([{"name": "x", "code": ["a/**"], "notIn": []}]))
                .starts_with("rule 'x': notIn is empty")
        );
        assert!(error(json!([{"name": "x", "code": ["a/[b"]}])).starts_with("rule 'x': code: "));
        assert!(
            error(json!([{"name": "x", "code": ["a.kt#"]}])).contains("names no method after '#'")
        );
        assert!(error(json!([{"name": "x", "code": ["a.kt#(*Fee"]}])).contains("no ')'"));
        assert!(
            error(json!([{"name": "x", "code": ["a.kt#*Fee"]}]))
                .contains("write a pattern of names in parentheses")
        );
        assert!(error(json!([{"name": "x", "code": ["#total"]}])).contains("no path before '#'"));
        assert_eq!(
            error(json!([{"name": "x", "code": ["a/**"]}, {"name": "x", "code": ["b/**"]}])),
            "rule 'x': another rule has the same name"
        );
        assert_eq!(
            error(json!([{"name": "x", "code": ["a/**"], "thresholds": {"similarity": 1.5}}])),
            "rule 'x': thresholds.similarity is 1.5, it must be a number in (0, 1]"
        );
        assert_eq!(
            error(json!([{"name": "x", "code": ["a/**"], "thresholds": {"minTokens": 0}}])),
            "rule 'x': thresholds.minTokens must be at least 1"
        );
        assert!(
            error(json!([{"name": "x", "code": ["a/**"], "thresholds": {"minToken": 3}}]))
                .contains("unknown field `minToken`")
        );
    }

    #[test]
    fn disable_turns_kinds_off_by_the_names_of_kind() {
        let kinds = |names: &[&str]| {
            Kinds::without(&names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
        };
        let all = kinds(&[]).unwrap();
        assert!(all.tokens() && all.gap() && all.ast() && all.semantic());

        let no_model = kinds(&["semantic"]).unwrap();
        assert!(!no_model.semantic());
        assert!(no_model.takes(CloneKind::Exact, None));
        assert!(!no_model.takes(CloneKind::Semantic, None));

        // `similar` is both mechanisms, in any letter case.
        let no_similar = kinds(&["Similar"]).unwrap();
        assert!(no_similar.tokens() && !no_similar.gap() && !no_similar.ast());
        assert!(!no_similar.takes(CloneKind::Similar, Some(SimilarityMethod::Gap)));
        assert!(!no_similar.takes(CloneKind::Similar, Some(SimilarityMethod::Ast)));

        let no_exact = kinds(&["exact"]).unwrap();
        assert!(no_exact.tokens(), "renamed and gap are token clones too");
        assert!(!no_exact.takes(CloneKind::Exact, None));
        assert!(no_exact.takes(CloneKind::Renamed, None));

        let only_ast = kinds(&["exact", "renamed", "gap", "semantic"]).unwrap();
        assert!(!only_ast.tokens() && only_ast.ast() && !only_ast.semantic());

        assert_eq!(
            kinds(&["exact", "renamed", "similar", "semantic"]).unwrap_err(),
            "disable turns off every kind of copy, so the rule would check nothing"
        );
        assert_eq!(
            kinds(&["semntic"]).unwrap_err(),
            "disable: unknown clone kind 'semntic': must be one of: exact, renamed, similar, gap, ast, semantic"
        );
    }

    #[test]
    fn a_kind_the_rule_cannot_disable_stops_the_run() {
        assert_eq!(
            rules(json!([{"name": "x", "code": ["a/**"], "disable": ["semntic"]}])).unwrap_err(),
            "rule 'x': disable: unknown clone kind 'semntic': must be one of: exact, renamed, similar, gap, ast, semantic"
        );
        let rule = rule(json!({"name": "x", "code": ["a/**"], "disable": ["semantic", "ast"]}));
        assert!(!rule.kinds.semantic() && !rule.kinds.ast() && rule.kinds.tokens());
    }

    #[test]
    fn a_hash_inside_a_folder_name_is_part_of_the_path() {
        let rule = rule(json!({"name": "x", "code": ["c#/src/**"]}));
        assert!(matches!(
            rule.protects(&at("c#/src/a.cs")),
            Some(Protection::Whole)
        ));
    }
}
