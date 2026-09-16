//! Turning a resolved graph into the findings a reader acts on.
//!
//! The graph knows what is reachable. This module decides which of the
//! unreachable things are worth saying out loud, in what words, and with how
//! much confidence — and, just as importantly, which to stay quiet about.
//!
//! The suppressions matter as much as the rules. A declaration inside a file
//! that is itself unused is not reported: the file is the finding, and
//! listing its twenty exports underneath would bury it. A member whose name
//! is read anywhere in the project is not reported, because basta infers no
//! types and cannot tell one `render` from another. A declaration the parser
//! never saw is never guessed at.

use crate::confidence::{Evidence, score};
use crate::config::BastaConfig;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::model::{Module, Symbol, SymbolFlags, SymbolKind};
use cpd_core::deadcode::Category;

/// Every finding the configuration asks for, ordered for a reader: whole
/// files first, then by path, then down the file.
pub fn findings(graph: &Graph, config: &BastaConfig) -> Vec<Finding> {
    let unparsed = graph.has_unparsed_modules();
    let mut out = Vec::new();

    for module in &graph.modules {
        if module.parse_failed {
            continue; // Nothing was read; nothing can be claimed.
        }
        // Two file-level verdicts, both of which make everything inside the
        // file moot: nothing runs it at all, or only the test suite does.
        // Reporting one file beats reporting its twenty exports.
        let dead = !graph.is_module_reachable(module.id);
        let test_only = !module.is_test && graph.is_module_used_only_by_tests(module.id);
        if dead || test_only {
            if config.reports(Category::UnusedFile) {
                out.push(unused_file(module, test_only, unparsed, config));
            }
            continue;
        }
        if module.is_test && !config.include_tests {
            continue;
        }
        // Everything in an ambient declaration file is declared for a compiler
        // rather than imported by a module — an `interface Array` there
        // augments a global, and nothing will ever name it. The file itself is
        // still checked above; only its contents are off limits.
        if module.traits.ambient_declarations {
            continue;
        }

        for symbol in graph.symbols_of(module.id) {
            if let Some(finding) = classify_symbol(graph, module, symbol, config, unparsed) {
                out.push(finding);
            }
        }
    }

    out.retain(|f| f.confidence >= config.min_confidence && f.lines >= config.min_lines);
    out.sort_by(|a, b| {
        (a.category != Category::UnusedFile)
            .cmp(&(b.category != Category::UnusedFile))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.start.line.cmp(&b.start.line))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

fn classify_symbol(
    graph: &Graph,
    module: &Module,
    symbol: &Symbol,
    config: &BastaConfig,
    unparsed: bool,
) -> Option<Finding> {
    let category = category_for(graph, module, symbol, config)?;
    if !config.reports(category) {
        return None;
    }
    // A declaration whose name basta could not read cannot be matched against
    // anything, so it is never reported rather than always reported.
    if symbol.name.starts_with('<') {
        return None;
    }

    let evidence = gather_evidence(graph, module, symbol, category, unparsed);

    let (confidence, reasons) = score(category, &evidence);
    let parent = symbol
        .parent
        .map(|p| graph.symbol(p).name.clone())
        .filter(|name| !name.is_empty());
    Some(Finding {
        message: message_for(
            category,
            symbol,
            parent.as_deref(),
            &module.path,
            graph.is_used_only_by_tests(symbol.id),
        ),
        category,
        path: module.path.clone(),
        name: symbol.name.clone(),
        exported_as: symbol
            .export_name()
            .filter(|name| *name != symbol.name)
            .map(str::to_string),
        symbol_kind: Some(symbol.kind),
        parent,
        language: module.language.to_string(),
        start: symbol.start.clone(),
        end: symbol.end.clone(),
        lines: symbol.lines,
        confidence,
        reasons,
    })
}

/// Which rule, if any, a declaration falls under.
///
/// The question every branch answers is the same one: does this run when the
/// shipped program runs? A declaration the test suite reaches and production
/// does not still counts as a finding — the confidence model marks it — but
/// one the program itself reaches never does.
fn category_for(
    graph: &Graph,
    module: &Module,
    symbol: &Symbol,
    config: &BastaConfig,
) -> Option<Category> {
    // An import the module also exports is a re-export: the binding *is* the
    // module's public surface, so it is judged by whether anything imports it
    // onward, not by whether the file that wrote it also uses it. A package
    // `__init__.py` is made almost entirely of these.
    if symbol.kind == SymbolKind::Import && !symbol.is_exported() {
        // Used in the file that wrote it, or imported onward out of it. The
        // second is how a barrel file works: `phases.py` imports a name only
        // so that `orchestrator.py` can import it from `phases`, and nothing
        // in `phases.py` itself ever mentions it again.
        let used = graph.is_import_used(symbol.id)
            || graph.is_export_imported(symbol.module, &symbol.name);
        return (!used).then_some(Category::UnusedImport);
    }
    if symbol.kind.is_member() {
        // Without types, `x.render()` could be a call to any `render`, so a
        // name nobody reads *anywhere* is the only member basta can honestly
        // call dead. Reachability is deliberately not consulted: the graph
        // keeps a live class's members reachable so the chain through them
        // holds, which says nothing about whether any of them is called.
        if graph.is_member_name_read(&symbol.name) {
            return None;
        }
        // A member of a declaration that is itself dead is covered by that
        // declaration's finding; repeating it per method buries the one line
        // a reader needs.
        if symbol
            .parent
            .is_some_and(|parent| !graph.is_symbol_production_reachable(parent))
        {
            return None;
        }
        return Some(Category::UnusedMember);
    }
    if !symbol.flags.contains(SymbolFlags::TOP_LEVEL) {
        // A nested declaration is reported through its enclosing one: if the
        // outer function is dead the file already says so, and if it is alive
        // a closure inside it is part of that body.
        return None;
    }
    if symbol.is_exported() {
        let export_name = symbol.export_name().unwrap_or(&symbol.name);
        // Imported by name from a module that runs: used, entry point or not.
        // A module something namespace-imports can be reached without naming,
        // so its exports are only reported when no member access anywhere in
        // the scan could be that reach.
        if graph.is_export_imported_by_production(symbol.module, export_name)
            || (graph.is_wildcarded(symbol.module) && graph.is_member_name_read(export_name))
        {
            return None;
        }
        // An entry point's exports are the surface the outside world calls.
        // Nothing in the scan is meant to import them, so reachability says
        // nothing about them — the graph roots them on purpose — and they are
        // reported only on request.
        if module.is_entry {
            return config
                .include_entry_exports
                .then_some(Category::UnusedExport);
        }
        // An export its own module uses, in a module that runs, is alive.
        // That the `export` keyword is then unnecessary is a style question,
        // not dead code, and basta does not raise it.
        if graph.is_symbol_production_reachable(symbol.id) {
            return None;
        }
        return Some(Category::UnusedExport);
    }
    (!graph.is_symbol_production_reachable(symbol.id)).then_some(Category::UnusedSymbol)
}

fn unused_file(module: &Module, test_only: bool, unparsed: bool, config: &BastaConfig) -> Finding {
    let evidence = Evidence {
        in_test_file: module.is_test && !config.include_tests,
        unparsed_module: unparsed,
        package_surface: module.traits.package_surface,
        used_only_by_tests: test_only,
        ..Evidence::default()
    };
    let (confidence, reasons) = score(Category::UnusedFile, &evidence);
    let message = if test_only {
        format!("{} is only imported by tests", module.path)
    } else {
        format!(
            "{} is never imported and is not an entry point",
            module.path
        )
    };
    Finding {
        category: Category::UnusedFile,
        message,
        path: module.path.clone(),
        name: String::new(),
        exported_as: None,
        symbol_kind: None,
        parent: None,
        language: module.language.to_string(),
        start: cpd_core::models::Location {
            line: 1,
            column: 0,
            offset: 0,
        },
        end: cpd_core::models::Location {
            line: module.lines.max(1),
            column: 0,
            offset: 0,
        },
        lines: module.lines,
        confidence,
        reasons,
    }
}

/// Everything that bears on whether this finding is really dead.
///
/// Only what can apply to the category is collected. A string literal cannot
/// keep an import *binding* alive, and a member has no module boundary to be
/// public across, so filling those in would subtract points for evidence that
/// is not evidence.
fn gather_evidence(
    graph: &Graph,
    module: &Module,
    symbol: &Symbol,
    category: Category,
    unparsed: bool,
) -> Evidence {
    // True of any finding, whatever rule produced it.
    let mut evidence = Evidence {
        in_test_file: module.is_test,
        unparsed_module: unparsed,
        // Code that evaluates source or resolves names at runtime can reach
        // anything, including an import binding.
        dynamic_module: module.has_dynamic_access,
        ..Evidence::default()
    };
    if category == Category::UnusedImport {
        // An unreferenced binding is a fact about one file, not an inference
        // about the project; nothing else can make it wrong.
        return evidence;
    }

    // True of any finding about a named declaration.
    evidence.name_in_string = graph.appears_in_string(&symbol.name);
    evidence.decorated = symbol.flags.contains(SymbolFlags::DECORATED);

    match category {
        Category::UnusedMember => {
            // A member is resolved by name alone, so who declared it and who
            // implements it matter more than any module boundary.
            evidence.abstract_declaration = symbol.flags.contains(SymbolFlags::ABSTRACT);
            evidence.overrides = symbol.flags.contains(SymbolFlags::OVERRIDE);
            evidence.ambiguous_name = graph.is_name_ambiguous(&symbol.name);
        }
        Category::UnusedSymbol | Category::UnusedExport => {
            // Where top-level names are attributes of the module object, a
            // member-style read of the name anywhere may be this declaration.
            evidence.name_read_as_attribute =
                module.traits.names_are_attributes && graph.is_member_name_read(&symbol.name);
            evidence.package_surface = module.traits.package_surface;
            evidence.used_only_by_tests = graph.is_used_only_by_tests(symbol.id);
            if category == Category::UnusedExport {
                // Only an export can be called from outside the scan, and
                // only an export is matched against another module's imports,
                // which a name two modules declare makes unreliable.
                evidence.public_api = module.is_entry;
                evidence.wildcarded = graph.is_wildcarded(module.id);
                evidence.ambiguous_name = graph.is_name_ambiguous(&symbol.name);
            }
        }
        Category::UnusedImport | Category::UnusedFile => {}
    }
    evidence
}

fn message_for(
    category: Category,
    symbol: &Symbol,
    parent: Option<&str>,
    path: &str,
    test_only: bool,
) -> String {
    let noun = symbol.kind.noun();
    let name = &symbol.name;
    match category {
        Category::UnusedImport => format!("`{name}` is imported but never used"),
        Category::UnusedSymbol if test_only => {
            format!("{noun} `{name}` is only used by tests")
        }
        Category::UnusedSymbol => {
            format!("{noun} `{name}` is never used in {path}")
        }
        Category::UnusedExport if test_only => {
            format!("exported {noun} `{name}` is only imported by tests")
        }
        Category::UnusedExport => match symbol.export_name() {
            Some(exported) if exported != name => {
                format!("{noun} `{name}` is exported as `{exported}` but nothing imports it")
            }
            _ => format!("exported {noun} `{name}` is never imported"),
        },
        Category::UnusedMember => match parent {
            Some(owner) => format!("{noun} `{owner}.{name}` is never accessed"),
            None => format!("{noun} `{name}` is never accessed"),
        },
        Category::UnusedFile => format!("{path} is never imported"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::deadcode::Reason;

    struct Scan {
        graph: Graph,
    }

    fn scan(files: &[(&str, &str)], entries: &[&str]) -> Scan {
        scan_with(files, entries, &[])
    }

    fn scan_with(files: &[(&str, &str)], entries: &[&str], tests: &[&str]) -> Scan {
        Scan {
            graph: crate::test_scan::build(files, entries, tests),
        }
    }

    impl Scan {
        fn findings(&self, config: &BastaConfig) -> Vec<Finding> {
            findings(&self.graph, config)
        }

        fn all(&self) -> Vec<Finding> {
            self.findings(&BastaConfig {
                categories: Category::ALL.to_vec(),
                min_confidence: 0,
                ..BastaConfig::default()
            })
        }
    }

    fn names(findings: &[Finding], category: Category) -> Vec<String> {
        let mut out: Vec<String> = findings
            .iter()
            .filter(|f| f.category == category)
            .map(|f| {
                if f.name.is_empty() {
                    f.path.clone()
                } else {
                    f.name.clone()
                }
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn each_rule_reports_the_thing_it_is_named_after() {
        let scan = scan(
            &[
                (
                    "index.ts",
                    "import { used, spare } from './api';\nused();\n",
                ),
                (
                    "api.ts",
                    "export function used() { helper(); }\n\
                     function helper() {}\n\
                     function deadHelper() {}\n\
                     export function spare() {}\n\
                     export function neverImported() {}\n",
                ),
                ("orphan.ts", "export const x = 1;\n"),
            ],
            &["index.ts"],
        );
        let found = scan.all();
        assert_eq!(names(&found, Category::UnusedFile), vec!["orphan.ts"]);
        assert_eq!(names(&found, Category::UnusedImport), vec!["spare"]);
        assert_eq!(names(&found, Category::UnusedSymbol), vec!["deadHelper"]);
        assert_eq!(
            names(&found, Category::UnusedExport),
            vec!["neverImported"],
            "`spare` is imported, so the import is the finding, not the export"
        );
    }

    #[test]
    fn a_dead_file_swallows_the_findings_inside_it() {
        let scan = scan(
            &[
                ("index.ts", "export const kept = 1;\n"),
                (
                    "orphan.ts",
                    "import { missing } from './nowhere';\n\
                     export function a() {}\n\
                     export function b() {}\n\
                     function c() {}\n",
                ),
            ],
            &["index.ts"],
        );
        let found = scan.all();
        let in_orphan: Vec<&Finding> = found.iter().filter(|f| f.path == "orphan.ts").collect();
        assert_eq!(
            in_orphan.len(),
            1,
            "one unused file, not four findings inside it: {:?}",
            in_orphan.iter().map(|f| &f.message).collect::<Vec<_>>()
        );
        assert_eq!(in_orphan[0].category, Category::UnusedFile);
    }

    #[test]
    fn a_member_whose_name_is_read_anywhere_is_never_reported() {
        let scan = scan(
            &[(
                "index.ts",
                "export class A {\n  render() {}\n}\n\
                 export class B {\n  render() {}\n  onlyHere() {}\n}\n\
                 declare const a: A;\na.render();\n",
            )],
            &["index.ts"],
        );
        let found = scan.all();
        assert_eq!(
            names(&found, Category::UnusedMember),
            vec!["onlyHere"],
            "without types basta cannot tell A.render from B.render"
        );
    }

    #[test]
    fn findings_below_the_confidence_threshold_are_dropped() {
        let scan = scan(
            &[
                ("index.ts", "import './api';\n"),
                (
                    "api.ts",
                    "export function unusedButNamed() {}\nexport const hint = 'unusedButNamed';\n",
                ),
            ],
            &["index.ts"],
        );
        let strict = scan.findings(&BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 90,
            ..BastaConfig::default()
        });
        assert!(
            !strict.iter().any(|f| f.name == "unusedButNamed"),
            "a name that appears in a string is not a 90-confidence finding"
        );
        let lenient = scan.all();
        let finding = lenient
            .iter()
            .find(|f| f.name == "unusedButNamed")
            .expect("reported at min_confidence 0");
        assert!(finding.reasons.contains(&Reason::NameAppearsInString));
    }

    #[test]
    fn min_lines_filters_out_one_line_declarations() {
        let scan = scan(
            &[(
                "index.ts",
                "const short = 1;\n\
                 function long() {\n  const a = 1;\n  return a;\n}\n\
                 export const kept = 2;\n",
            )],
            &["index.ts"],
        );
        let found = scan.findings(&BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            min_lines: 3,
            ..BastaConfig::default()
        });
        assert_eq!(names(&found, Category::UnusedSymbol), vec!["long"]);
    }

    #[test]
    fn test_files_are_quiet_by_default_and_reported_on_request() {
        let files = [
            ("index.ts", "export const used = 1;\n"),
            (
                "a.test.ts",
                "import { used } from './index';\nfunction deadInTest() {}\nconsole.log(used);\n",
            ),
        ];
        let scan = scan_with(&files, &["index.ts"], &["a.test.ts"]);
        assert!(
            !scan.all().iter().any(|f| f.path == "a.test.ts"),
            "a test file's own dead code is noise by default"
        );

        let including = scan.findings(&BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            include_tests: true,
            ..BastaConfig::default()
        });
        assert!(including.iter().any(|f| f.name == "deadInTest"));
    }

    #[test]
    fn python_private_helpers_and_public_names_land_in_different_categories() {
        let scan = scan(
            &[
                ("app/__main__.py", "from .core import main\n\nmain()\n"),
                (
                    "app/core.py",
                    "def main():\n    pass\n\n\ndef _unused_helper():\n    pass\n\n\ndef public_unused():\n    pass\n",
                ),
            ],
            &["app/__main__.py"],
        );
        let found = scan.all();
        assert_eq!(
            names(&found, Category::UnusedSymbol),
            vec!["_unused_helper"]
        );
        assert_eq!(names(&found, Category::UnusedExport), vec!["public_unused"]);
    }

    #[test]
    fn messages_name_the_thing_and_read_as_sentences() {
        let scan = scan(
            &[
                ("index.ts", "import { unusedImport } from './api';\n"),
                (
                    "api.ts",
                    "function inner() {}\nexport { inner as outer };\nexport function unusedImport() {}\n",
                ),
            ],
            &["index.ts"],
        );
        let found = scan.all();
        let messages: Vec<&str> = found.iter().map(|f| f.message.as_str()).collect();
        assert!(
            messages.contains(&"`unusedImport` is imported but never used"),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("`inner` is exported as `outer` but nothing imports it")),
            "{messages:?}"
        );
    }

    #[test]
    fn findings_are_ordered_files_first_then_by_path_and_line() {
        let scan = scan(
            &[
                (
                    "index.ts",
                    "function b() {}\nfunction a() {}\nexport const kept = 1;\n",
                ),
                ("z-orphan.ts", "export const x = 1;\n"),
            ],
            &["index.ts"],
        );
        let found = scan.all();
        assert_eq!(found[0].category, Category::UnusedFile);
        let in_index: Vec<&str> = found
            .iter()
            .filter(|f| f.path == "index.ts")
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(
            in_index,
            vec!["b", "a"],
            "declaration order, not alphabetical"
        );
    }

    #[test]
    fn an_unparsable_file_lowers_confidence_everywhere_rather_than_being_reported() {
        let scan = scan(
            &[
                ("index.ts", "export function orphaned() {}\n"),
                ("broken.ts", "function ( { { {\n"),
            ],
            &["index.ts"],
        );
        let found = scan.all();
        assert!(
            !found.iter().any(|f| f.path == "broken.ts"),
            "a file basta could not read is never a finding"
        );
        assert!(
            found
                .iter()
                .all(|f| f.reasons.contains(&Reason::UnparsedModule)),
            "every finding says a file in the scan did not parse"
        );
    }

    #[test]
    fn an_entry_points_exports_are_its_public_surface_not_dead_code() {
        let files = [(
            "src/index.ts",
            "export function published() {}\nfunction unusedHelper() {}\n",
        )];
        let scan = scan(&files, &["src/index.ts"]);
        assert_eq!(
            names(&scan.all(), Category::UnusedExport),
            Vec::<String>::new(),
            "nothing inside the scan is supposed to import a published API"
        );
        assert_eq!(
            names(&scan.all(), Category::UnusedSymbol),
            vec!["unusedHelper"]
        );

        let including = scan.findings(&BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            include_entry_exports: true,
            ..BastaConfig::default()
        });
        assert_eq!(names(&including, Category::UnusedExport), vec!["published"]);
    }

    #[test]
    fn include_entry_exports_reports_only_the_entry_exports_nothing_imports() {
        let scan = scan(
            &[
                (
                    "src/index.ts",
                    "export function imported() {}\nexport function orphaned() {}\n",
                ),
                (
                    "src/other.ts",
                    "import { imported } from './index';\nexport const x = imported();\n",
                ),
            ],
            &["src/index.ts", "src/other.ts"],
        );
        let including = scan.findings(&BastaConfig {
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            include_entry_exports: true,
            ..BastaConfig::default()
        });
        let reported = names(&including, Category::UnusedExport);
        assert!(reported.contains(&"orphaned".to_string()), "{reported:?}");
        assert!(
            !reported.contains(&"imported".to_string()),
            "an entry export another module imports is used, flag or no flag: {reported:?}"
        );
    }

    #[test]
    fn a_test_file_is_an_entry_point_and_is_never_an_unused_file() {
        let scan = scan_with(
            &[
                ("index.ts", "export const used = 1;\n"),
                (
                    "a.test.ts",
                    "import { used } from './index';\nconsole.log(used);\n",
                ),
            ],
            &["index.ts"],
            &["a.test.ts"],
        );
        assert!(
            !scan
                .all()
                .iter()
                .any(|f| f.path == "a.test.ts" && f.category == Category::UnusedFile),
            "a test runner starts a test file, so nothing needs to import it"
        );
    }

    #[test]
    fn an_export_only_the_tests_import_says_so() {
        let scan = scan_with(
            &[
                ("index.ts", "import { shipped } from './api';\nshipped();\n"),
                (
                    "api.ts",
                    "export function shipped() {}\nexport function forTestsOnly() {}\n",
                ),
                (
                    "api.test.ts",
                    "import { forTestsOnly } from './api';\nforTestsOnly();\n",
                ),
            ],
            &["index.ts"],
            &["api.test.ts"],
        );
        let found = scan.all();
        let finding = found
            .iter()
            .find(|f| f.name == "forTestsOnly")
            .expect("an export only tests reach is still worth reporting");
        assert_eq!(finding.category, Category::UnusedExport);
        assert!(finding.reasons.contains(&Reason::UsedOnlyByTests));
        assert!(
            finding.message.contains("only imported by tests"),
            "{}",
            finding.message
        );
        assert!(
            !found.iter().any(|f| f.name == "shipped"),
            "an export the program imports is not a finding"
        );
    }

    #[test]
    fn a_file_only_the_tests_import_is_one_finding_not_many() {
        let scan = scan_with(
            &[
                ("index.ts", "export const shipped = 1;\n"),
                (
                    "helpers.ts",
                    "export function makeUser() {}\nexport function makeOrder() {}\n",
                ),
                (
                    "a.test.ts",
                    "import { makeUser } from './helpers';\nmakeUser();\n",
                ),
            ],
            &["index.ts"],
            &["a.test.ts"],
        );
        let in_helpers: Vec<&Finding> = scan
            .all()
            .into_iter()
            .filter(|f| f.path == "helpers.ts")
            .collect::<Vec<_>>()
            .leak()
            .iter()
            .collect();
        assert_eq!(in_helpers.len(), 1, "{in_helpers:?}");
        assert_eq!(in_helpers[0].category, Category::UnusedFile);
        assert!(in_helpers[0].message.contains("only imported by tests"));
    }

    #[test]
    fn an_export_clause_does_not_keep_its_own_symbol_alive() {
        let scan = scan(
            &[
                ("index.ts", "import './api';\n"),
                (
                    "api.ts",
                    "function inner() {}\nexport { inner as outer };\n",
                ),
            ],
            &["index.ts"],
        );
        assert_eq!(
            names(&scan.all(), Category::UnusedExport),
            vec!["inner"],
            "`export {{ inner }}` is not a use of inner"
        );
    }

    #[test]
    fn declarations_in_an_ambient_file_are_left_alone() {
        let scan = scan(
            &[
                ("index.ts", "export const a = 1;\n"),
                (
                    "globals.d.ts",
                    "interface Array<T> {\n  toSorted(): T[];\n}\ndeclare const VERSION: string;\n",
                ),
            ],
            // A `.d.ts` is an entry point in a real run (`entry::detect`
            // matches it), so the file-level rule never fires on one.
            &["index.ts", "globals.d.ts"],
        );
        assert!(
            !scan.all().iter().any(|f| f.path == "globals.d.ts"),
            "an ambient declaration augments a global; no import will ever name it"
        );
    }

    #[test]
    fn a_barrel_file_import_is_used_by_whoever_imports_it_onward() {
        let scan = scan(
            &[
                (
                    "app/__main__.py",
                    "from app.phases import run_phase\n\nrun_phase()\n",
                ),
                ("app/phases.py", "from app.verify import run_phase\n"),
                ("app/verify.py", "def run_phase():\n    pass\n"),
            ],
            &["app/__main__.py"],
        );
        assert!(
            !scan
                .all()
                .iter()
                .any(|f| f.path == "app/phases.py" && f.name == "run_phase"),
            "phases.py imports the name so that __main__ can import it from phases"
        );
    }

    #[test]
    fn disabled_categories_produce_nothing() {
        let scan = scan(
            &[
                ("index.ts", "import { spare } from './api';\n"),
                ("api.ts", "export function spare() {}\n"),
                ("orphan.ts", "export const x = 1;\n"),
            ],
            &["index.ts"],
        );
        let only_files = scan.findings(&BastaConfig {
            categories: vec![Category::UnusedFile],
            min_confidence: 0,
            ..BastaConfig::default()
        });
        assert!(
            only_files
                .iter()
                .all(|f| f.category == Category::UnusedFile)
        );
        assert_eq!(only_files.len(), 1);
    }
}
