use super::*;
use crate::testing::project;
use cpd_finder::orchestrate::RunConfig;
use std::path::Path;

/// A function, to copy into two files.
const ADD: &str =
    "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}\n";
const ADD_SNIPPET: &str =
    "function add(a, b) {\n  const sum = a + b;\n  console.log('sum', sum);\n  return sum;\n}";

/// The same function twice, with every identifier renamed: a Type-2 clone.
/// (Names the TypeScript grammar also uses as keywords, such as `out`,
/// `type` or `from`, are not normalized.)
const SCALE: &str = "function scale(values, factor) {\n  const acc = [];\n  for (const value of values) {\n    acc.push(value * factor + 1);\n  }\n  console.log('scaled', acc.length);\n  return acc;\n}\n";
const GROW: &str = "function grow(items, ratio) {\n  const list = [];\n  for (const item of items) {\n    list.push(item * ratio + 1);\n  }\n  console.log('scaled', list.length);\n  return list;\n}\n";

/// A function and a copy with a line inserted in the middle: a Type-3
/// clone of two exact halves.
const REPORT: &str = "function report(rows) {\n  const header = rows.map((row) => row.name).join(', ');\n  const total = rows.reduce((sum, row) => sum + row.amount, 0);\n  console.info('report header', header, total);\n  const average = total / Math.max(rows.length, 1);\n  const widest = rows.reduce((max, row) => Math.max(max, row.name.length), 0);\n  return { header, total, average, widest };\n}\n";
const REPORT_EDITED: &str = "function report(rows) {\n  const header = rows.map((row) => row.name).join(', ');\n  const total = rows.reduce((sum, row) => sum + row.amount, 0);\n  console.info('report header', header, total);\n  audit.push({ at: Date.now(), rows: rows.length });\n  const average = total / Math.max(rows.length, 1);\n  const widest = rows.reduce((max, row) => Math.max(max, row.name.length), 0);\n  return { header, total, average, widest };\n}\n";

fn run_config(dir: &Path, min_tokens: usize) -> RunConfig {
    RunConfig {
        paths: vec![dir.to_path_buf()],
        min_tokens,
        min_lines: 1,
        ..Default::default()
    }
}

fn server(dir: &Path) -> McpServer {
    McpServer::new(Settings::of_run(run_config(dir, 15)))
}

/// A server over two copies of [`ADD`].
fn copies() -> McpServer {
    server(&project(&[("one.js", ADD), ("two.js", ADD)]))
}

fn request(server: &mut McpServer, method: &str, params: Value) -> Value {
    server
        .handle_message(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .expect("a request gets a response")
}

/// The result of a tool call.
fn call(server: &mut McpServer, name: &str, args: Value) -> Value {
    let response = request(
        server,
        "tools/call",
        json!({ "name": name, "arguments": args }),
    );
    response["result"].clone()
}

/// The JSON a tool answered with; the text is the structured content.
fn payload(result: &Value) -> Value {
    assert_eq!(result["isError"], false, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(text, result["structuredContent"].to_string());
    result["structuredContent"].clone()
}

/// The message of a tool error.
fn tool_error(result: &Value) -> String {
    assert_eq!(result["isError"], true, "{result}");
    result["content"][0]["text"].as_str().unwrap().to_string()
}

/// `_meta` of a request of the stateless protocol.
fn stateless(version: &str) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" },
    })
}

#[test]
fn initialize_negotiates_a_handshake_version() {
    let mut s = copies();
    for (asked, given) in [
        ("2025-03-26", "2025-03-26"),
        ("2025-11-25", "2025-11-25"),
        ("2026-07-28", "2025-11-25"),
        ("1999-01-01", "2025-11-25"),
    ] {
        let resp = request(&mut s, "initialize", json!({ "protocolVersion": asked }));
        assert_eq!(resp["result"]["protocolVersion"], given, "asked {asked}");
        assert_eq!(resp["result"]["serverInfo"]["name"], "jscpd");
        assert!(resp["result"].get("resultType").is_none());
    }
    let resp = request(&mut s, "initialize", json!({}));
    let instructions = resp["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains(
            "without it they report what jscpd reports with this server's options: exact."
        ),
        "{instructions}"
    );
}

#[test]
fn notifications_get_no_response() {
    let mut s = copies();
    for method in ["notifications/initialized", "notifications/cancelled"] {
        assert!(
            s.handle_message(&json!({ "jsonrpc": "2.0", "method": method }))
                .is_none()
        );
    }
}

#[test]
fn unknown_method_returns_method_not_found() {
    let mut s = copies();
    let resp = request(&mut s, "resources/list", json!({}));
    assert_eq!(resp["error"]["code"], METHOD_NOT_FOUND);
    assert_eq!(resp["id"], 1);
}

#[test]
fn tools_list_describes_five_tools() {
    let mut s = copies();
    let resp = request(&mut s, "tools/list", json!({}));
    let tools = resp["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "check_duplication",
            "get_file_clones",
            "get_statistics",
            "check_current_directory",
            "compare_folders"
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    let kinds = &tools[0]["inputSchema"]["properties"]["kinds"];
    assert!(
        kinds["description"]
            .as_str()
            .unwrap()
            .contains("Without it: exact, what jscpd reports")
    );
    assert!(resp["result"].get("ttlMs").is_none(), "handshake era");
}

#[test]
fn stateless_requests_carry_their_version_in_meta() {
    let mut s = copies();
    let discover = request(
        &mut s,
        "server/discover",
        json!({ "_meta": stateless("2026-07-28") }),
    );
    let result = &discover["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"][0], "2026-07-28");
    assert!(
        result["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2024-11-05"))
    );
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["cacheScope"], "public");
    assert!(result["ttlMs"].as_u64().unwrap() > 0);
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "jscpd"
    );

    let list = request(
        &mut s,
        "tools/list",
        json!({ "_meta": stateless("2026-07-28") }),
    );
    assert_eq!(list["result"]["resultType"], "complete");
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 5);
    assert!(list["result"]["ttlMs"].is_u64());

    let call = request(
        &mut s,
        "tools/call",
        json!({
            "name": "get_statistics",
            "arguments": {},
            "_meta": stateless("2026-07-28"),
        }),
    );
    assert_eq!(call["result"]["resultType"], "complete");
    assert_eq!(payload(&call["result"])["files"], 2);

    // A handshake revision in `_meta` is served as after a handshake.
    let old = request(
        &mut s,
        "tools/list",
        json!({ "_meta": stateless("2025-06-18") }),
    );
    assert!(old["result"].get("resultType").is_none());
}

#[test]
fn an_unknown_protocol_version_lists_the_supported_ones() {
    let mut s = copies();
    let resp = request(
        &mut s,
        "tools/list",
        json!({ "_meta": stateless("1900-01-01") }),
    );
    assert_eq!(resp["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION);
    assert_eq!(resp["error"]["data"]["requested"], "1900-01-01");
    assert_eq!(resp["error"]["data"]["supported"][0], "2026-07-28");
}

#[test]
fn a_stateless_request_must_carry_the_client_capabilities() {
    let mut s = copies();
    let resp = request(
        &mut s,
        "tools/list",
        json!({ "_meta": { "io.modelcontextprotocol/protocolVersion": "2026-07-28" } }),
    );
    assert_eq!(resp["error"]["code"], INVALID_PARAMS);
    let resp = request(&mut s, "server/discover", json!({}));
    assert_eq!(
        resp["error"]["code"], INVALID_PARAMS,
        "discovery names its version"
    );
}

#[test]
fn check_duplication_finds_an_exact_copy() {
    let mut s = copies();
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": ADD_SNIPPET, "format": "javascript" }),
    ));
    assert!(found["count"].as_u64().unwrap() >= 1, "{found}");
    assert_eq!(found["kinds"], json!(["exact"]), "a plain jscpd run's");
    let dup = &found["duplications"][0];
    assert_eq!(dup["kind"], "exact");
    assert!(dup["file"].as_str().unwrap().ends_with(".js"));
    assert!(dup["tokens"].as_u64().unwrap() >= 15);
    assert_eq!(dup["snippetStartLine"], 1);
}

#[test]
fn check_duplication_finds_every_copy_in_the_project() {
    // Three copies: detection pairs copies with the first that has them,
    // and a snippet must not lose its match with the second.
    let mut s = server(&project(&[
        ("one.js", ADD),
        ("two.js", ADD),
        ("three.js", ADD),
    ]));
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": ADD_SNIPPET, "format": "javascript", "kinds": ["exact"] }),
    ));
    let mut files: Vec<&str> = found["duplications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["file"].as_str().unwrap())
        .collect();
    files.sort_unstable();
    assert_eq!(files, ["one.js", "three.js", "two.js"], "{found}");
}

#[test]
fn check_duplication_accepts_an_extension_for_the_format() {
    let mut s = copies();
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": ADD_SNIPPET, "format": "js" }),
    ));
    assert_eq!(
        found["format"], "javascript",
        "the resolved format is echoed"
    );
    assert!(found["count"].as_u64().unwrap() >= 1);
}

#[test]
fn check_duplication_limit_truncates_with_a_note() {
    let mut s = copies();
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": ADD_SNIPPET, "format": "javascript", "limit": 0 }),
    ));
    assert!(found["count"].as_u64().unwrap() >= 1, "the total stays");
    assert_eq!(found["returned"], 0);
    assert!(found["note"].as_str().unwrap().contains("truncated"));
}

#[test]
fn check_duplication_of_a_short_or_unknown_snippet() {
    let mut s = copies();
    let short = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": "let x = 1;", "format": "javascript" }),
    ));
    assert_eq!(short["count"], 0);
    assert!(short["note"].as_str().unwrap().contains("min-tokens"));
    let clean = payload(&call(
        &mut s,
        "check_duplication",
        json!({
            "code": "const totallyUnique = [9, 8, 7].map((n) => n * 31 + 5).filter((n) => n % 2 === 0);",
            "format": "javascript"
        }),
    ));
    assert_eq!(clean["count"], 0);
}

#[test]
fn bad_arguments_are_tool_errors_the_model_can_fix() {
    let mut s = copies();
    let cases = [
        (
            "check_duplication",
            json!({ "code": "x" }),
            "'format' is required",
        ),
        (
            "check_duplication",
            json!({ "code": 1, "format": "js" }),
            "'code' must be a string",
        ),
        (
            "check_duplication",
            json!({ "code": "x", "format": "not-a-language" }),
            "unknown format",
        ),
        (
            "check_duplication",
            json!({ "code": "x", "format": "js", "similarity": 2 }),
            "'similarity' must be a number",
        ),
        (
            "check_current_directory",
            json!({ "limit": "ten" }),
            "'limit' must be a non-negative integer",
        ),
        ("get_file_clones", json!({}), "'path' is required"),
        (
            "get_statistics",
            json!({ "kinds": ["copies"] }),
            "unknown clone kind 'copies'",
        ),
        ("get_statistics", json!({ "kinds": [] }), "names no kind"),
        (
            "get_statistics",
            json!({ "kinds": "exact" }),
            "'kinds' must be an array",
        ),
        (
            "compare_folders",
            json!({ "left": "nowhere", "right": "." }),
            "'nowhere' does not exist",
        ),
    ];
    for (tool, args, expected) in cases {
        let message = tool_error(&call(&mut s, tool, args.clone()));
        assert!(message.contains(expected), "{tool} {args}: {message}");
    }
}

#[test]
fn unknown_tools_and_malformed_calls_are_protocol_errors() {
    let mut s = copies();
    for params in [
        json!({ "name": "bogus", "arguments": {} }),
        json!({ "name": "get_statistics", "arguments": "all" }),
        json!({ "arguments": {} }),
    ] {
        let resp = request(&mut s, "tools/call", params.clone());
        assert_eq!(resp["error"]["code"], INVALID_PARAMS, "{params}");
    }
}

#[test]
fn check_duplication_finds_a_renamed_copy_when_asked() {
    let dir = project(&[("scale.js", SCALE)]);
    let mut s = server(&dir);
    let check = |s: &mut McpServer, extra: Value| {
        let mut args = json!({ "code": GROW, "format": "javascript" });
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        payload(&call(s, "check_duplication", args))
    };
    assert_eq!(check(&mut s, json!({}))["count"], 0, "a plain run's kinds");
    for kinds in [json!(["exact", "renamed"]), json!(["type2"])] {
        let renamed = check(&mut s, json!({ "kinds": kinds }));
        assert_eq!(renamed["count"], 1, "{renamed}");
        assert_eq!(renamed["duplications"][0]["kind"], "renamed");
        assert_eq!(renamed["duplications"][0]["file"], "scale.js");
    }
    // A server that normalizes identifiers finds them without being asked.
    let run = RunConfig {
        ignore_identifiers: true,
        ..run_config(&dir, 15)
    };
    let mut normalizing = McpServer::new(Settings::of_run(run));
    let found = check(&mut normalizing, json!({}));
    assert_eq!(found["kinds"], json!(["exact", "renamed"]));
    assert_eq!(found["count"], 1, "{found}");
}

#[test]
fn ast_matches_take_the_shape_of_the_request() {
    let mut s = server(&project(&[("scale.js", SCALE)]));
    // Without `kinds`, as earlier versions answered: under `similar`.
    let earlier = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": GROW, "format": "javascript", "similarity": 0.85 }),
    ));
    assert_eq!(earlier["kinds"], json!(["exact", "ast"]));
    assert_eq!(earlier["count"], 0, "{earlier}");
    assert_eq!(earlier["similarCount"], 1, "{earlier}");
    let hit = &earlier["similar"][0];
    assert_eq!(
        (&hit["file"], &hit["name"], &hit["snippetName"]),
        (&json!("scale.js"), &json!("scale"), &json!("grow"))
    );
    assert_eq!(hit["similarity"], 1.0);
    // With `kinds`, among the duplications with every other match.
    let asked = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": GROW, "format": "javascript", "kinds": ["ast"] }),
    ));
    assert!(asked.get("similar").is_none(), "{asked}");
    assert_eq!(asked["duplications"][0]["method"], "ast", "{asked}");
    assert_eq!(asked["duplications"][0]["name"], "scale");
    // A format without syntax trees says so where earlier versions did.
    let ruby = payload(&call(
        &mut s,
        "check_duplication",
        json!({
            "code": "def scale(values, factor)\n  out = []\n  values.each do |value|\n    out << value * factor + 1\n  end\n  puts \"scaled #{out.size}\"\n  out\nend\n",
            "format": "ruby",
            "similarity": 0.85,
        }),
    ));
    assert_eq!(ruby["similarCount"], 0, "{ruby}");
    assert!(ruby["similarNote"].is_string(), "{ruby}");
    assert!(ruby.get("unavailable").is_none(), "{ruby}");
}

const CART_VUE: &str = "<template>\n  <p>{{ total }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction total(items: Item[]): number {\n  let sum = 0;\n  for (const item of items) {\n    sum += item.price * item.count;\n  }\n  return sum;\n}\n</script>\n";
const BASKET_VUE: &str = "<template>\n  <p>{{ amount }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction amount(lines: Line[]): number {\n  let acc = 0;\n  for (const line of lines) {\n    acc += line.price * line.count;\n  }\n  return acc;\n}\n</script>\n";

#[test]
fn component_snippets_match_by_the_functions_of_their_scripts() {
    let mut s = server(&project(&[("cart.vue", CART_VUE)]));
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": BASKET_VUE, "format": "vue", "similarity": 0.85 }),
    ));
    assert_eq!(found["similarCount"], 1, "{found}");
    assert!(found.get("unavailable").is_none(), "{found}");
}

const SCALE_PY: &str = "def scale(values, factor):\n    out = []\n    for value in values:\n        out.append(value * factor + 1)\n    out.sort()\n    print('scaled', len(out))\n    return out\n";
const GROW_PY: &str = "def grow(items, ratio):\n    res = []\n    for item in items:\n        res.append(item * ratio + 1)\n    res.sort()\n    print('grown', len(res))\n    return res\n";
const SHRINK_PY: &str = "def shrink(items, ratio):\n    res = []\n    for item in items:\n        res.remove(item * ratio + 1)\n    res.reverse()\n    print('shrunk', len(res))\n    return res\n";

#[test]
fn python_snippets_match_by_shape_and_role_aware_servers_read_called_methods() {
    use cpd_similarity::SimilarityIdentifiers;
    let dir = project(&[("scale.py", SCALE_PY)]);
    let ask = |s: &mut McpServer, code: &str| {
        payload(&call(
            s,
            "check_duplication",
            json!({ "code": code, "format": "python", "similarity": 0.85 }),
        ))
    };
    let mut plain = server(&dir);
    assert_eq!(ask(&mut plain, GROW_PY)["similarCount"], 1);
    assert_eq!(
        ask(&mut plain, SHRINK_PY)["similarCount"],
        1,
        "names do not count by default"
    );
    let run = RunConfig {
        similarity_identifiers: SimilarityIdentifiers::RoleAware,
        ..run_config(&dir, 15)
    };
    let mut role_aware = McpServer::new(Settings::of_run(run));
    let renamed = ask(&mut role_aware, GROW_PY);
    assert_eq!(renamed["similarCount"], 1, "renames still match: {renamed}");
    let other = ask(&mut role_aware, SHRINK_PY);
    assert_eq!(
        other["similarCount"], 0,
        "other called methods do not: {other}"
    );
}

#[test]
fn servers_read_literals_as_their_literal_mode_says() {
    use cpd_similarity::SimilarityLiterals;
    let dir = project(&[("scale.py", SCALE_PY)]);
    // The same function with other values, and with a number in place of
    // the string.
    let other_values = SCALE_PY.replace("+ 1", "+ 7").replace("'scaled'", "'done'");
    let a_number = SCALE_PY.replace("'scaled'", "0");
    let ask = |s: &mut McpServer, code: &str| {
        let answer = payload(&call(
            s,
            "check_duplication",
            json!({ "code": code, "format": "python", "similarity": 0.99 }),
        ));
        answer["similarCount"].as_u64().unwrap()
    };
    let with = |literals| {
        McpServer::new(Settings::of_run(RunConfig {
            similarity_literals: literals,
            ..run_config(&dir, 15)
        }))
    };
    // The tools say how this server compares literals.
    let similarity_text = |s: &mut McpServer| {
        let tools = request(s, "tools/list", json!({}));
        tools["result"]["tools"][0]["inputSchema"]["properties"]["similarity"]["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let mut values_server = with(SimilarityLiterals::Values);
    let described = similarity_text(&mut values_server);
    assert!(
        described.contains("--similarity-literals values"),
        "{described}"
    );
    assert!(!described.contains("literal changes"), "{described}");
    let initialized = request(&mut values_server, "initialize", json!({}));
    let instructions = initialized["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains("a literal counts by its value"),
        "{instructions}"
    );
    let mut categories = with(SimilarityLiterals::Categories);
    assert!(similarity_text(&mut categories).contains("literal changes"));
    assert_eq!(
        ask(&mut categories, &other_values),
        1,
        "other values of one kind match"
    );
    assert_eq!(ask(&mut categories, &a_number), 0, "a number is no string");
    assert_eq!(ask(&mut with(SimilarityLiterals::Values), &other_values), 0);
    assert_eq!(ask(&mut with(SimilarityLiterals::Generic), &a_number), 1);
    assert_eq!(ask(&mut with(SimilarityLiterals::Omit), &a_number), 1);
}

#[test]
fn project_clones_carry_their_kind() {
    let dir = project(&[
        ("renamed/scale.js", SCALE),
        ("renamed/grow.js", GROW),
        ("gap/report.js", REPORT),
        ("gap/edited.js", REPORT_EDITED),
    ]);
    // 30 tokens: the halves of the edited copy count, and the look-alike
    // statements inside one function do not.
    let mut s = McpServer::new(Settings::of_run(run_config(&dir, 30)));
    let all = payload(&call(
        &mut s,
        "check_current_directory",
        json!({ "kinds": ["exact", "renamed", "similar"] }),
    ));
    let kinds: Vec<(&str, Option<&str>)> = all["duplications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["kind"].as_str().unwrap(), d["method"].as_str()))
        .collect();
    assert!(kinds.contains(&("renamed", None)), "{all}");
    assert!(kinds.contains(&("similar", Some("gap"))), "{all}");
    // Each function pair once: the ast pairs of the two copies are the
    // renamed clone and the merged one already.
    assert_eq!(all["byKind"], json!({ "gap": 1, "renamed": 1 }), "{all}");
    let gap = all["duplications"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["method"] == "gap")
        .unwrap();
    assert!(gap["similarity"].as_f64().unwrap() > 0.5, "{gap}");

    // Exact clones alone come from the scan a `jscpd` run makes: no
    // renamed pair, and the two halves of the edited copy unmerged.
    let exact = payload(&call(
        &mut s,
        "get_statistics",
        json!({ "kinds": ["exact"] }),
    ));
    assert_eq!(exact["kinds"], json!(["exact"]));
    assert_eq!(exact["byKind"], json!({ "exact": 2 }), "{exact}");
    let renamed = payload(&call(
        &mut s,
        "get_statistics",
        json!({ "kinds": ["renamed"] }),
    ));
    assert_eq!(renamed["byKind"], json!({ "renamed": 1 }), "{renamed}");
}

#[test]
fn get_file_clones_lists_the_clones_of_one_file() {
    let mut s = copies();
    let found = payload(&call(
        &mut s,
        "get_file_clones",
        json!({ "path": "one.js" }),
    ));
    assert!(found["clones"].as_u64().unwrap() >= 1, "{found}");
    let dup = &found["duplications"][0];
    assert!(
        dup["fileA"] == "one.js" || dup["fileB"] == "one.js",
        "{dup}"
    );
    assert_eq!(dup["kind"], "exact");
    assert!(found.get("note").is_none());
    let missing = payload(&call(
        &mut s,
        "get_file_clones",
        json!({ "path": "does/not/exist.js" }),
    ));
    assert_eq!(missing["clones"], 0);
    assert!(
        missing["note"]
            .as_str()
            .unwrap()
            .contains("not part of the scan"),
        "{missing}"
    );
}

#[test]
fn clone_lists_are_sorted_biggest_first() {
    // The big pair must come first although the small one's files sort
    // first by name.
    let small =
        "function s(a, b) {\n  const r = a * b + a;\n  console.log('r', r);\n  return r;\n}\n";
    let big = "function big(a, b, c) {\n  const x = a + b * c;\n  const y = x * x - a;\n  const z = y + b - c;\n  console.log('x', x);\n  console.log('y', y);\n  console.log('z', z);\n  return x + y + z;\n}\n";
    let dir = project(&[
        ("a1.js", small),
        ("a2.js", small),
        ("z1.js", big),
        ("z2.js", big),
    ]);
    let mut s = server(&dir);
    let found = payload(&call(
        &mut s,
        "check_current_directory",
        json!({ "kinds": ["exact"] }),
    ));
    let dups = found["duplications"].as_array().unwrap();
    assert!(dups.len() >= 2, "{found}");
    assert!(dups[0]["tokens"].as_u64() >= dups[1]["tokens"].as_u64());
    assert!(
        dups[0]["fileA"].as_str().unwrap().starts_with('z'),
        "{found}"
    );
}

#[test]
fn get_statistics_reports_the_scan() {
    let mut s = copies();
    let stats = payload(&call(&mut s, "get_statistics", json!({})));
    assert_eq!(stats["files"], 2);
    assert!(stats["clones"].as_u64().unwrap() >= 1);
    assert_eq!(stats["byKind"]["exact"], stats["clones"]);
    assert!(stats["statistics"]["total"]["tokens"].as_u64().unwrap() > 0);
    assert!(stats.get("unavailable").is_none());
}

#[test]
fn check_current_directory_scans_again() {
    let dir = project(&[("one.js", ADD), ("two.js", ADD)]);
    let mut s = server(&dir);
    let before = payload(&call(&mut s, "get_statistics", json!({})));
    std::fs::write(dir.join("three.js"), ADD).unwrap();
    let stale = payload(&call(&mut s, "get_statistics", json!({})));
    assert_eq!(stale["files"], before["files"], "until the next scan");
    let after = payload(&call(&mut s, "check_current_directory", json!({})));
    assert_eq!(after["files"], 3);
    assert!(
        after["clones"].as_u64() > before["clones"].as_u64(),
        "{after}"
    );
    assert_eq!(after["returned"], after["clones"]);
    let limited = payload(&call(
        &mut s,
        "check_current_directory",
        json!({ "limit": 0 }),
    ));
    assert_eq!(limited["returned"], 0);
    assert!(limited["note"].as_str().unwrap().contains("truncated"));
}

#[test]
fn compare_folders_refuses_overlapping_folders() {
    let dir = project(&[("lib/a.js", ADD), ("lib/inner/b.js", ADD)]);
    let mut s = server(&dir);
    let message = tool_error(&call(
        &mut s,
        "compare_folders",
        json!({ "left": "lib", "right": "lib/inner" }),
    ));
    // The arguments are wrong, whatever the model: nothing to download.
    assert!(
        message.ends_with("overlap; give two separate folders"),
        "{message}"
    );
}

#[test]
fn compare_folders_stays_inside_the_scanned_folders() {
    let outside = project(&[("other/a.js", ADD)]);
    let dir = project(&[("lib/a.js", ADD)]);
    // The folder as a user types it: Windows gives a canonical path the
    // `\\?\` form, in which `..` stays a name.
    let typed = dir.join("lib").to_string_lossy().replace(r"\\?\", "");
    let mut s = server(Path::new(&typed));
    for left in [
        outside.join("other").to_string_lossy().into_owned(),
        format!(
            "../../{}/other",
            outside.file_name().unwrap().to_string_lossy()
        ),
    ] {
        let message = tool_error(&call(
            &mut s,
            "compare_folders",
            json!({ "left": left, "right": "." }),
        ));
        assert!(
            message.contains("is outside the scanned folders"),
            "{left}: {message}"
        );
    }
}

#[test]
fn a_repeat_inside_the_snippet_is_no_match() {
    let mut s = server(&project(&[("one.js", ADD)]));
    // Two copies of a function the project does not have.
    let twice = "function mul(x, y) {\n  const product = x * y;\n  console.warn('product', product, x, y);\n  return product;\n}\n".repeat(2);
    let found = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": twice, "format": "javascript", "kinds": ["exact"] }),
    ));
    assert_eq!(found["count"], 0, "{found}");
}

#[test]
fn a_kind_the_scan_did_not_read_scans_again() {
    let dir = project(&[("one.js", ADD), ("two.js", ADD)]);
    let run = RunConfig {
        kinds: vec![cpd_core::models::KindFilter::Exact],
        ..run_config(&dir, 15)
    };
    let mut s = McpServer::new(Settings::of_run(run));
    let exact = payload(&call(&mut s, "get_statistics", json!({})));
    assert_eq!(exact["kinds"], json!(["exact"]), "--kind sets the default");
    assert_eq!(exact["files"], 2);
    std::fs::write(dir.join("three.js"), ADD).unwrap();
    // The first scan read no syntax trees: ast clones need a scan that
    // does, and its tokens and trees come from the same files.
    let ast = payload(&call(
        &mut s,
        "get_statistics",
        json!({ "kinds": ["exact", "ast"] }),
    ));
    assert_eq!(ast["files"], 3, "{ast}");
    assert_eq!(ast["byKind"]["exact"], 2, "{ast}");
}

#[test]
fn a_kind_that_cannot_be_searched_is_unavailable_with_the_reason() {
    let dir = project(&[("one.js", ADD), ("two.js", ADD)]);
    // An embeddings API nobody listens to.
    let semantic = cpd_semantic::SemanticOptions {
        provider: cpd_semantic::Provider::Http,
        url: "http://127.0.0.1:1/v1".to_string(),
        model: "stand-in".to_string(),
        cache: false,
        ..Default::default()
    };
    let settings = Settings::of_run(run_config(&dir, 15)).with_semantic(semantic, true);
    let mut s = McpServer::new(settings);
    let stats = payload(&call(&mut s, "get_statistics", json!({})));
    assert_eq!(
        stats["kinds"],
        json!(["exact", "semantic"]),
        "--semantic adds the kind to the defaults"
    );
    assert_eq!(
        stats["byKind"]["exact"], 1,
        "the other kinds are found: {stats}"
    );
    assert!(stats["unavailable"]["semantic"].is_string(), "{stats}");
    let checked = payload(&call(
        &mut s,
        "check_duplication",
        json!({ "code": ADD_SNIPPET, "format": "javascript", "kinds": ["exact", "type4"] }),
    ));
    assert_eq!(checked["count"], 2, "both copies: {checked}");
    assert!(checked["unavailable"]["semantic"].is_string(), "{checked}");
}

#[test]
fn a_batch_gets_an_array_of_responses() {
    let mut s = copies();
    let ping = |id: u32| json!({ "jsonrpc": "2.0", "id": id, "method": "ping" });
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let responses = s.handle_batch(&[ping(1), note.clone(), ping(2)]).unwrap();
    let ids: Vec<&Value> = responses
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["id"])
        .collect();
    assert_eq!(ids, [&json!(1), &json!(2)]);
    assert!(s.handle_batch(&[note]).is_none(), "notifications only");
    let empty = s.handle_batch(&[]).unwrap();
    assert_eq!(empty["error"]["code"], INVALID_REQUEST);
    let odd = s.handle_message(&json!(42)).unwrap();
    assert_eq!(odd["error"]["code"], INVALID_REQUEST);
}
