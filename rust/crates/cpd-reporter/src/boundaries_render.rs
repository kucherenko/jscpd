//! What the boundary rules of a config file found, in the reports: a block
//! after the clones in the console reporters, a few lines in the `ai`
//! report, and the `boundaries` key of the JSON report.

use crate::shared::Style;
use cpd_core::boundaries::{BoundaryReport, Place, Violation};
use cpd_core::models::{CloneKind, Location, SimilarityMethod};
use cpd_finder::orchestrate::host_file;
use serde_json::json;

/// The console block:
///
/// ```text
/// Boundaries:
/// money: 2 copies outside the boundary
///   web/src/cart/total.ts:12-40 cartTotal
///     looks like backend/billing/Fees.kt:20-51 calculateFee (semantic ~0.71)
///   web/src/checkout/discount.ts:5-19 applyDiscount
///     copy of backend/billing/Discount.kt:30-44 applyDiscount (exact, 15 lines)
/// auth: no copies outside the boundary
/// ```
pub fn print_boundaries(report: &BoundaryReport, style: &Style) {
    if report.rules.is_empty() {
        return;
    }
    println!();
    println!("{}", style.bold("Boundaries:"));
    for rule in &report.rules {
        let found = match rule.violations.len() {
            0 => style.paint("no copies outside the boundary", 32),
            1 => style.red("1 copy outside the boundary"),
            n => style.red(&format!("{n} copies outside the boundary")),
        };
        println!("{}: {found}", style.bold(&rule.name));
        for violation in &rule.violations {
            println!("  {}", place(&violation.copy));
            println!(
                "    {} {} ({})",
                verb(violation),
                place(&violation.protected),
                match_label(violation)
            );
        }
    }
}

/// The `ai` lines: one per copy, the copy first, then a count.
///
/// ```text
/// Boundaries:
/// money: web/src/cart/total.ts:12-40 ~ backend/billing/Fees.kt:20-51 [~0.71 semantic]
/// 1 copy outside boundaries
/// ```
pub fn print_boundaries_compact(report: &BoundaryReport, style: &Style) {
    if report.rules.is_empty() {
        return;
    }
    println!("{}", style.bold("Boundaries:"));
    for rule in &report.rules {
        for violation in &rule.violations {
            println!(
                "{}: {} ~ {}{}",
                rule.name,
                compact(&violation.copy),
                compact(&violation.protected),
                compact_label(violation)
            );
        }
    }
    let total = report.violations();
    println!(
        "{} {} outside boundaries",
        style.bold(&total.to_string()),
        match total {
            1 => "copy",
            _ => "copies",
        }
    );
}

/// The `boundaries` key of the JSON report: every rule, with its copies.
/// A side names the file and the lines, as `firstFile` and `secondFile` of
/// a clone do, and the function it lies in when it has a name.
pub fn boundaries_json(report: &BoundaryReport) -> serde_json::Value {
    let rules: Vec<serde_json::Value> = report
        .rules
        .iter()
        .map(|rule| {
            let violations: Vec<serde_json::Value> = rule
                .violations
                .iter()
                .map(|violation| {
                    let mut value = json!({
                        "kind": violation.kind.as_str(),
                        "lines": violation.copy_lines(),
                        "tokens": violation.tokens,
                        "copy": side_json(&violation.copy),
                        "protected": side_json(&violation.protected),
                    });
                    if let Some(similarity) = violation.similarity_rounded() {
                        value["similarity"] = json!(similarity);
                    }
                    if let Some(method) = violation.method {
                        value["method"] = json!(method.as_str());
                    }
                    value
                })
                .collect();
            json!({"name": rule.name, "violations": violations})
        })
        .collect();
    serde_json::Value::Array(rules)
}

fn side_json(side: &Place) -> serde_json::Value {
    let fragment = &side.fragment;
    let location = |loc: &Location| {
        json!({
            "line": loc.line,
            "column": loc.column,
            "position": loc.offset,
        })
    };
    let mut value = json!({
        "name": host_file(&fragment.source_id),
        "start": fragment.start.line,
        "end": fragment.end.line,
        "startLoc": location(&fragment.start),
        "endLoc": location(&fragment.end),
    });
    if let Some(function) = &side.function {
        value["function"] = json!(function);
    }
    value
}

/// `path:start-end name`.
fn place(side: &Place) -> String {
    let fragment = &side.fragment;
    let name = side
        .function
        .as_ref()
        .map(|f| format!(" {f}"))
        .unwrap_or_default();
    format!(
        "{}:{}-{}{name}",
        host_file(&fragment.source_id),
        fragment.start.line,
        fragment.end.line
    )
}

/// A copy-paste is a copy; an edited or rewritten one looks like the code.
fn verb(violation: &Violation) -> &'static str {
    match (violation.kind, violation.method) {
        (CloneKind::Exact | CloneKind::Renamed, _) => "copy of",
        (CloneKind::Similar, Some(SimilarityMethod::Gap)) => "copy of",
        _ => "looks like",
    }
}

/// How the copy matches, as clone headers say it: `exact, 15 lines`,
/// `similar (ast) ~0.86`, `semantic ~0.71`.
fn match_label(violation: &Violation) -> String {
    let lines = violation.copy_lines();
    match (violation.kind, violation.similarity, violation.method) {
        (CloneKind::Exact, _, _) => format!("exact, {lines} lines"),
        (CloneKind::Renamed, _, _) => format!("renamed, {lines} lines"),
        (CloneKind::Similar, Some(s), Some(m)) => format!("similar ({}) ~{s:.2}", m.as_str()),
        (CloneKind::Similar, Some(s), None) => format!("similar ~{s:.2}"),
        (CloneKind::Similar, None, _) => "similar".to_string(),
        (CloneKind::Semantic, Some(s), _) => format!("semantic ~{s:.2}"),
        (CloneKind::Semantic, None, _) => "semantic".to_string(),
    }
}

/// `path:start-end`, with `/` between folders on every system.
fn compact(side: &Place) -> String {
    let fragment = &side.fragment;
    format!(
        "{}:{}-{}",
        host_file(&fragment.source_id).replace('\\', "/"),
        fragment.start.line,
        fragment.end.line
    )
}

/// The `ai` reporter's suffix for a clone of the same kind.
fn compact_label(violation: &Violation) -> String {
    match (violation.kind, violation.similarity, violation.method) {
        (CloneKind::Exact, _, _) => String::new(),
        (CloneKind::Renamed, _, _) => " (renamed)".to_string(),
        (CloneKind::Similar, Some(s), Some(m)) => format!(" [~{s:.2} {}]", m.as_str()),
        (CloneKind::Similar, Some(s), None) => format!(" [~{s:.2}]"),
        (CloneKind::Similar, None, _) => " [~]".to_string(),
        (CloneKind::Semantic, Some(s), _) => format!(" [~{s:.2} semantic]"),
        (CloneKind::Semantic, None, _) => " [semantic]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::boundaries::RuleReport;
    use cpd_core::models::Fragment;

    fn side(file: &str, from: u32, to: u32, function: Option<&str>) -> Place {
        Place {
            fragment: Fragment::new(
                file,
                Location::new(from, 0, 0),
                Location::new(to, 1, 0),
                [0, 0],
            ),
            function: function.map(String::from),
        }
    }

    fn report() -> BoundaryReport {
        BoundaryReport {
            rules: vec![
                RuleReport {
                    name: "money".into(),
                    violations: vec![
                        Violation {
                            kind: CloneKind::Semantic,
                            method: None,
                            similarity: Some(0.7123),
                            tokens: 80,
                            copy: side("web/total.ts", 12, 40, Some("cartTotal")),
                            protected: side("backend/Fees.kt", 20, 51, Some("calculateFee")),
                        },
                        Violation {
                            kind: CloneKind::Exact,
                            method: None,
                            similarity: None,
                            tokens: 60,
                            copy: side("web/App.vue:typescript", 5, 19, None),
                            protected: side("backend/Discount.kt", 30, 44, Some("applyDiscount")),
                        },
                    ],
                },
                RuleReport {
                    name: "auth".into(),
                    violations: vec![],
                },
            ],
        }
    }

    #[test]
    fn each_copy_says_where_it_is_what_it_copies_and_how() {
        let money = &report().rules[0];
        assert_eq!(
            place(&money.violations[0].copy),
            "web/total.ts:12-40 cartTotal"
        );
        assert_eq!(verb(&money.violations[0]), "looks like");
        assert_eq!(match_label(&money.violations[0]), "semantic ~0.71");
        // A copy in a component's script names the component.
        assert_eq!(place(&money.violations[1].copy), "web/App.vue:5-19");
        assert_eq!(verb(&money.violations[1]), "copy of");
        assert_eq!(match_label(&money.violations[1]), "exact, 15 lines");
        assert_eq!(compact_label(&money.violations[0]), " [~0.71 semantic]");
    }

    #[test]
    fn the_json_lists_every_rule_with_its_copies() {
        let json = boundaries_json(&report());
        assert_eq!(json[0]["name"], "money");
        let first = &json[0]["violations"][0];
        assert_eq!(first["kind"], "semantic");
        assert_eq!(first["similarity"], 0.712);
        assert_eq!(first["lines"], 29);
        assert_eq!(first["copy"]["name"], "web/total.ts");
        assert_eq!(first["copy"]["function"], "cartTotal");
        assert_eq!(first["protected"]["start"], 20);
        assert!(first.get("method").is_none());
        let second = &json[0]["violations"][1];
        assert_eq!(second["copy"]["name"], "web/App.vue");
        assert!(second["copy"].get("function").is_none());
        assert!(second.get("similarity").is_none());
        assert_eq!(json[1]["name"], "auth");
        assert_eq!(json[1]["violations"], json!([]));
    }
}
