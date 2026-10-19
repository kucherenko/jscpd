// rust/crates/cpd/tests/boundaries.rs — the boundary rules of a config file
// (issue #1225), run through the CLI. The model is the stand-in embeddings
// server of `common`, so no test downloads one.

mod common;

use common::embeddings::Server;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

/// A fee calculation of a backend, long enough for a clone at the default
/// `--min-tokens`.
const FEES_TS: &str = "export function calculateFee(amount: number, country: string, express: boolean): number {\n  const base = amount * 0.029 + 30;\n  const international = country === 'US' ? 0 : amount * 0.015;\n  const rush = express ? Math.max(500, amount * 0.01) : 0;\n  const total = Math.round(base + international + rush);\n  return total > 10000 ? 10000 : total;\n}\n";

/// A function of its own, with nothing in common with the fees.
const GREETING_TS: &str = "export function greeting(user: { first: string; last: string }, hour: number): string {\n  const part = hour < 12 ? 'morning' : hour < 18 ? 'afternoon' : 'evening';\n  const name = [user.first, user.last].filter(Boolean).join(' ');\n  const text = `Good ${part}, ${name}!`;\n  return text.length > 60 ? text.slice(0, 57) + '...' : text;\n}\n";

/// A folder for test `name`, gone with its cache and report folders.
fn fresh(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cpd-boundaries-{name}-{}", std::process::id()));
    for dir in [root.clone(), beside(&root, "cache"), beside(&root, "out")] {
        let _ = std::fs::remove_dir_all(dir);
    }
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A directory next to the scanned one, so reports and caches written there
/// are never scanned.
fn beside(dir: &Path, what: &str) -> PathBuf {
    dir.with_file_name(format!(
        "{}-{what}",
        dir.file_name().unwrap().to_string_lossy()
    ))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn config(root: &Path, boundaries: Value) {
    write(
        root,
        ".jscpd.json",
        &json!({ "boundaries": boundaries }).to_string(),
    );
}

/// `jscpd <args> .` in `dir`, the model served by `server`.
fn run(dir: &Path, args: &[&str], server: Option<&Server>) -> Output {
    let mut command = Command::new(cpd_bin());
    command
        .args(args)
        .args(["--no-tips", "--no-colors"])
        .arg(".")
        .current_dir(dir)
        .env("JSCPD_CACHE_DIR", beside(dir, "cache"))
        .env_remove("JSCPD_SEMANTIC_API_KEY");
    if let Some(server) = server {
        command.args([
            "--semantic-url",
            &server.url,
            "--semantic-model",
            "stand-in",
        ]);
    }
    command.output().expect("failed to run cpd")
}

/// The JSON report of a run, its exit code and its stderr.
fn scan(dir: &Path, args: &[&str], server: Option<&Server>) -> (Value, Option<i32>, String) {
    let out = beside(dir, "out");
    let mut full: Vec<&str> = args.to_vec();
    full.extend([
        "--reporters",
        "json,silent",
        "--output",
        out.to_str().unwrap(),
    ]);
    let output = run(dir, &full, server);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let report = std::fs::read_to_string(out.join("jscpd-report.json"))
        .unwrap_or_else(|_| panic!("no JSON report: {stderr}"));
    (
        serde_json::from_str(&report).unwrap(),
        output.status.code(),
        stderr,
    )
}

/// `(rule, kind, copy file, protected file)` of every violation, with `/`
/// between folders.
fn violations(report: &Value) -> Vec<(String, String, String, String)> {
    let name = |side: &Value| side["name"].as_str().unwrap().replace('\\', "/");
    report["boundaries"]
        .as_array()
        .expect("a boundaries key")
        .iter()
        .flat_map(|rule| {
            rule["violations"].as_array().unwrap().iter().map(move |v| {
                (
                    rule["name"].as_str().unwrap().to_string(),
                    v["kind"].as_str().unwrap().to_string(),
                    name(&v["copy"]),
                    name(&v["protected"]),
                )
            })
        })
        .collect()
}

fn money_project(name: &str) -> PathBuf {
    let root = fresh(name);
    write(&root, "backend/billing/fees.ts", FEES_TS);
    write(&root, "web/src/greeting.ts", GREETING_TS);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/billing/**"], "notIn": ["web/**"]}]),
    );
    root
}

#[test]
fn a_copy_of_protected_code_outside_its_boundary_fails_the_run() {
    let server = Server::start();
    let root = money_project("copy");
    write(
        &root,
        "web/src/cart.ts",
        &format!("{GREETING_TS}\n{FEES_TS}"),
    );

    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        violations(&report),
        vec![(
            "money".into(),
            "exact".into(),
            "web/src/cart.ts".into(),
            "backend/billing/fees.ts".into()
        )],
        "{stderr}"
    );
    let violation = &report["boundaries"][0]["violations"][0];
    assert_eq!(violation["copy"]["start"], 8);
    assert_eq!(violation["copy"]["function"], "calculateFee");
    assert_eq!(violation["protected"]["function"], "calculateFee");
    assert!(
        stderr.contains(
            "ERROR: jscpd found 1 copy of protected code outside its boundary (money: 1)"
        ),
        "{stderr}"
    );
    // The clone report is the usual one: the fees and the greeting.
    assert_eq!(report["statistics"]["total"]["clones"], 2);
}

#[test]
fn without_a_copy_the_rules_pass_and_the_report_says_so() {
    let server = Server::start();
    let root = money_project("clean");
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(
        report["boundaries"],
        json!([{"name": "money", "violations": []}])
    );
    assert!(!stderr.contains("ERROR"), "{stderr}");
}

#[test]
fn copies_inside_the_module_in_allowed_places_and_outside_not_in_are_fine() {
    let server = Server::start();
    let root = fresh("allowed");
    write(&root, "backend/billing/fees.ts", FEES_TS);
    write(&root, "backend/billing/legacy/fees.ts", FEES_TS);
    write(&root, "backend/orders/fees.ts", FEES_TS);
    write(&root, "web/e2e/fees.spec.ts", FEES_TS);
    config(
        &root,
        json!([{
            "name": "money",
            "code": ["backend/billing"],
            "notIn": ["web/**"],
            "allow": ["web/e2e/**"]
        }]),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(violations(&report), vec![]);
}

#[test]
fn without_not_in_every_place_outside_the_code_is_kept_out() {
    let server = Server::start();
    let root = fresh("outside");
    write(&root, "packages/billing/fees.ts", FEES_TS);
    write(&root, "packages/orders/fees.ts", FEES_TS);
    config(
        &root,
        json!([{"name": "billing", "code": ["packages/billing"]}]),
    );
    let (report, code, _) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1));
    assert_eq!(
        violations(&report),
        vec![(
            "billing".into(),
            "exact".into(),
            "packages/orders/fees.ts".into(),
            "packages/billing/fees.ts".into()
        )]
    );
}

#[test]
fn a_method_rule_protects_the_methods_it_names_and_not_the_rest_of_the_file() {
    let server = Server::start();
    let root = fresh("methods");
    write(
        &root,
        "backend/billing.ts",
        &format!("{FEES_TS}\n{GREETING_TS}"),
    );
    // A copy of the greeting, which no rule protects.
    write(&root, "web/hello.ts", GREETING_TS);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/billing.ts#calculateFee"], "notIn": ["web/**"]}]),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(violations(&report), vec![]);

    // And a copy of the fee calculation.
    write(&root, "web/fees.ts", FEES_TS);
    let (report, code, _) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1));
    assert_eq!(
        violations(&report),
        vec![(
            "money".into(),
            "exact".into(),
            "web/fees.ts".into(),
            "backend/billing.ts".into()
        )]
    );

    // A pattern of names picks it too.
    config(
        &root,
        json!([{"name": "money", "code": ["backend/*.ts#(calculate*)"], "notIn": ["web/**"]}]),
    );
    let (report, _, _) = scan(&root, &[], Some(&server));
    assert_eq!(violations(&report).len(), 1);
}

#[test]
fn a_second_copy_outside_pairs_with_the_protected_code_not_with_the_first_copy() {
    let server = Server::start();
    let root = fresh("two-copies");
    write(&root, "lib/money/fees.ts", FEES_TS);
    // Both copies sort before the protected file: detection pairs every
    // copy with the first one it sees, unless the protected file goes first.
    write(&root, "app/a/fees.ts", FEES_TS);
    write(&root, "app/b/fees.ts", FEES_TS);
    config(&root, json!([{"name": "money", "code": ["lib/money/**"]}]));
    let (report, _, _) = scan(&root, &[], Some(&server));
    let copies: Vec<String> = violations(&report).into_iter().map(|v| v.2).collect();
    assert_eq!(copies, vec!["app/a/fees.ts", "app/b/fees.ts"]);
}

#[test]
fn the_logic_written_again_in_another_language_is_a_violation() {
    let server = Server::start();
    let root = fresh("rewrite");
    common::embeddings::cart_project(&root);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/**"], "notIn": ["frontend/**"]}]),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        violations(&report),
        vec![(
            "money".into(),
            "semantic".into(),
            "frontend/src/Cart.svelte".into(),
            "backend/src/cart.rs".into()
        )],
        "{stderr}"
    );
    let violation = &report["boundaries"][0]["violations"][0];
    assert_eq!(violation["copy"]["function"], "cartTotal");
    assert_eq!(violation["protected"]["function"], "cart_total");
    assert!(violation["similarity"].as_f64().unwrap() > 0.6);
    assert!(
        stderr.contains("Boundaries: comparing 13 protected functions with "),
        "{stderr}"
    );
    // Without --semantic: the model ran for the rules only.
    assert_eq!(report["statistics"]["total"]["clones"], 0);
}

#[test]
fn an_edited_copy_in_a_test_file_counts() {
    let server = Server::start();
    let root = fresh("edited");
    write(&root, "backend/billing/invoice.js", common::INVOICE_JS);
    write(
        &root,
        "web/src/__tests__/credit.test.js",
        common::CREDIT_NOTE_JS,
    );
    config(
        &root,
        json!([{
            "name": "money",
            "code": ["backend/billing/**"],
            "notIn": ["web/**"],
            "thresholds": {"similarity": 0.6, "minTokens": 30}
        }]),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        violations(&report),
        vec![(
            "money".into(),
            "similar".into(),
            "web/src/__tests__/credit.test.js".into(),
            "backend/billing/invoice.js".into()
        )],
        "{stderr}"
    );
    assert_eq!(report["boundaries"][0]["violations"][0]["method"], "ast");
}

#[test]
fn a_rule_can_look_for_smaller_copies_than_the_clone_report() {
    let server = Server::start();
    let root = fresh("thresholds");
    let small = "export function serviceFee(amount: number): number {\n  const fee = Math.round(amount * 0.029 + 30);\n  const capped = fee > 10000 ? 10000 : fee;\n  return capped < 50 ? 50 : capped;\n}\n";
    write(&root, "backend/billing/fees.ts", small);
    write(&root, "web/fees.ts", small);
    let rule = |thresholds: Value| json!([{"name": "money", "code": ["backend/billing/**"], "notIn": ["web/**"], "thresholds": thresholds}]);
    // The function is too small for the structure search at this minNodes,
    // and its 46 tokens too few for a token clone at the default 50.
    config(&root, rule(json!({"minNodes": 500})));
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(violations(&report), vec![]);

    config(
        &root,
        rule(json!({"minNodes": 500, "minTokens": 20, "minLines": 3})),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(violations(&report).len(), 1);
    assert_eq!(
        report["statistics"]["total"]["clones"], 0,
        "the clone report keeps its bars"
    );
}

#[test]
fn the_filters_of_the_clone_report_do_not_hide_a_violation() {
    let server = Server::start();
    let root = money_project("filters");
    write(&root, "web/src/cart.ts", FEES_TS);
    let (report, code, stderr) = scan(&root, &["--skip-isolated", "backend|web"], Some(&server));
    assert_eq!(report["statistics"]["total"]["clones"], 0, "{stderr}");
    assert_eq!(code, Some(1));
    assert_eq!(violations(&report).len(), 1);
}

#[test]
fn the_console_shows_each_copy_and_what_it_copies() {
    let server = Server::start();
    let root = money_project("console");
    write(
        &root,
        "web/src/cart.ts",
        &format!("{GREETING_TS}\n{FEES_TS}"),
    );
    config(
        &root,
        json!([
            {"name": "money", "code": ["backend/billing/**"], "notIn": ["web/**"]},
            {"name": "greetings", "code": ["web/src/greeting.ts"]}
        ]),
    );
    let output = run(&root, &["--reporters", "console"], Some(&server));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let block = stdout
        .split("Boundaries:\n")
        .nth(1)
        .unwrap_or_else(|| panic!("no boundaries block: {stdout}"));
    let block: Vec<&str> = block.lines().take(4).collect();
    assert_eq!(
        block,
        vec![
            "money: 1 copy outside the boundary",
            "  web/src/cart.ts:8-14 calculateFee",
            "    copy of backend/billing/fees.ts:1-7 calculateFee (exact, 7 lines)",
            "greetings: 1 copy outside the boundary",
        ],
        "{stdout}"
    );

    let output = run(&root, &["--reporters", "ai"], Some(&server));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("money: web/src/cart.ts:8-14 ~ backend/billing/fees.ts:1-7\n"),
        "{stdout}"
    );
    assert!(stdout.contains("2 copies outside boundaries"), "{stdout}");
}

#[test]
fn without_the_model_the_rules_go_on_without_the_semantic_search() {
    // A copy is still found, by its tokens, and the warning shows even
    // under --silent.
    let root = money_project("no-model");
    write(&root, "web/src/cart.ts", FEES_TS);
    let (report, code, stderr) = scan(&root, &["--silent"], None);
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(violations(&report).len(), 1, "{stderr}");
    assert!(
        stderr.contains("Warning: boundaries: the model nomic-ai/CodeRankEmbed is not downloaded, so the rules look for copies without it"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Run `jscpd --semantic-download` once"),
        "{stderr}"
    );

    // The logic written again in another language goes unseen.
    let root = fresh("no-model-rewrite");
    common::embeddings::cart_project(&root);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/**"], "notIn": ["frontend/**"]}]),
    );
    let (report, code, stderr) = scan(&root, &[], None);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(violations(&report), vec![]);
    assert!(
        !stderr.contains("Boundaries: comparing"),
        "nothing to embed with: {stderr}"
    );
}

#[test]
fn a_rule_that_disables_semantic_needs_no_model() {
    // No embeddings server and no model in the cache, and no warning.
    let root = money_project("disable-semantic");
    write(&root, "web/src/cart.ts", FEES_TS);
    config(
        &root,
        json!([{
            "name": "money",
            "code": ["backend/billing/**"],
            "notIn": ["web/**"],
            "disable": ["semantic"]
        }]),
    );
    let (report, code, stderr) = scan(&root, &[], None);
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(violations(&report).len(), 1, "{stderr}");
    assert!(!stderr.contains("is not downloaded"), "{stderr}");
}

#[test]
fn a_disabled_kind_leaves_the_copy_to_the_kinds_still_on() {
    let server = Server::start();
    let root = money_project("disable-kinds");
    write(&root, "web/src/cart.ts", FEES_TS);
    let kinds_of = |disable: Value| {
        config(
            &root,
            json!([{
                "name": "money",
                "code": ["backend/billing/**"],
                "notIn": ["web/**"],
                "disable": disable
            }]),
        );
        let (report, code, stderr) = scan(&root, &[], Some(&server));
        let kinds: Vec<String> = violations(&report).into_iter().map(|v| v.1).collect();
        (kinds, code, stderr)
    };
    // The copy is exact first; without exact clones it is a function of the
    // same structure, then one that does the same job.
    assert_eq!(kinds_of(json!([])).0, vec!["exact"]);
    assert_eq!(kinds_of(json!(["exact"])).0, vec!["similar"]);
    assert_eq!(kinds_of(json!(["exact", "ast"])).0, vec!["semantic"]);
    let (kinds, code, stderr) = kinds_of(json!(["exact", "ast", "semantic"]));
    assert_eq!(kinds, Vec::<String>::new(), "{stderr}");
    assert_eq!(code, Some(0), "{stderr}");
}

#[test]
fn a_rule_that_disables_gap_keeps_the_exact_parts_of_a_merged_clone() {
    let server = Server::start();
    let root = money_project("disable-gap");
    // The fee calculation with a line put in the middle.
    let edited = FEES_TS.replace("  const rush", "  console.log(amount);\n  const rush");
    write(&root, "web/src/cart.ts", &edited);
    let found = |disable: Value| {
        config(
            &root,
            json!([{
                "name": "money",
                "code": ["backend/billing/**"],
                "notIn": ["web/**"],
                "thresholds": {"minTokens": 20, "minLines": 2},
                "disable": disable
            }]),
        );
        let (report, _, stderr) = scan(&root, &["--max-gap-lines", "2"], Some(&server));
        let found: Vec<(String, Option<String>)> = report["boundaries"][0]["violations"]
            .as_array()
            .unwrap_or_else(|| panic!("{stderr}"))
            .iter()
            .map(|v| {
                (
                    v["kind"].as_str().unwrap().to_string(),
                    v["method"].as_str().map(String::from),
                )
            })
            .collect();
        found
    };
    assert_eq!(
        found(json!([])),
        vec![("similar".to_string(), Some("gap".to_string()))]
    );
    assert_eq!(
        found(json!(["gap"])),
        vec![("exact".to_string(), None), ("exact".to_string(), None)]
    );
}

#[test]
fn a_mistake_in_a_rule_stops_the_run() {
    let root = fresh("mistake");
    write(&root, "backend/fees.ts", FEES_TS);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/**"], "notin": ["web/**"]}]),
    );
    let output = run(&root, &[], None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("Error: boundaries: rule 'money': unknown field `notin`"),
        "{stderr}"
    );
}

#[test]
fn a_config_named_with_config_starts_the_paths_of_its_rules_in_its_folder() {
    let server = Server::start();
    let root = fresh("config-folder");
    write(&root, "project/backend/billing/fees.ts", FEES_TS);
    write(&root, "project/web/cart.ts", FEES_TS);
    write(
        &root,
        "project/jscpd.json",
        &json!({"boundaries": [{"name": "money", "code": ["backend/billing/**"], "notIn": ["web/**"]}]})
            .to_string(),
    );
    let (report, code, stderr) = scan(&root, &["--config", "project/jscpd.json"], Some(&server));
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(violations(&report).len(), 1, "{stderr}");
}

#[test]
fn a_rule_that_protects_no_file_says_so() {
    let server = Server::start();
    let root = money_project("typo");
    config(
        &root,
        json!([{"name": "money", "code": ["backend/biling/**"]}]),
    );
    let (_, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0));
    assert!(
        stderr.contains("Warning: boundaries: rule 'money' protects no code of this scan"),
        "{stderr}"
    );

    // A method name with a typo protects nothing either.
    write(&root, "web/src/cart.ts", FEES_TS);
    config(
        &root,
        json!([{"name": "money", "code": ["backend/billing/fees.ts#calculateFees"], "notIn": ["web/**"]}]),
    );
    let (report, code, stderr) = scan(&root, &[], Some(&server));
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(violations(&report), vec![]);
    assert!(
        stderr.contains("Warning: boundaries: rule 'money' protects no code of this scan"),
        "{stderr}"
    );
}
