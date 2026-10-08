//! The tools of the MCP server: their definitions, the arguments they take,
//! and the JSON they answer with.
//!
//! - `check_duplication` (code, format, kinds?, limit?, similarity?): the
//!   clones between a snippet and the project
//! - `get_file_clones` (path, kinds?, limit?): the clones of one file
//! - `get_statistics` (kinds?): the project's duplication statistics
//! - `check_current_directory` (kinds?, limit?): scan again, list the clones
//! - `compare_folders` (left, right, limit?): two folders compared function
//!   by function, as `--compare` does
//!
//! A tool call with arguments a tool cannot use is answered with a tool
//! error (`isError`), which the model reads and can correct; only an
//! unknown tool or a malformed request is a protocol error.

use super::project::{Checked, Kinds, Match, Project, SNIPPET_ID};
use cpd_core::models::{CpdClone, SimilarityMethod};
use cpd_reporter::json_reporter::add_near_miss;
use serde_json::{Map, Value, json};

/// Default cap on the entries of a list, so a heavily duplicated project
/// does not flood the model's context.
const DEFAULT_LIMIT: usize = 100;

/// Why a request failed at the protocol level.
pub(super) struct Failure {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl Failure {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// How ast matches compare functions, as a sentence for a client.
const AST_METHOD: &str = "In ast matches each function's syntax tree is normalized: the names of the functions and methods it calls and its operators count, local names, field names and literals do not, and two functions score the share of subtrees their trees have in common. Test files are left out.";

/// What the server tells a client about itself: how to use the tools, with
/// the kinds this server looks for by default.
pub(super) fn instructions(project: &Project) -> String {
    format!(
        "jscpd finds duplicated code (clones) in the project it scanned at startup. Clones come in four kinds: exact (Type-1: the same tokens), renamed (Type-2: the same code with identifiers or literals changed), similar (Type-3, near-miss: a copy with lines added or removed, found by merging across the gap, 'gap', or a function with a similar structure, 'ast'), and semantic (Type-4: functions that do the same job written differently or in another language, found by an embedding model). Every clone tool takes 'kinds' to choose; without it they report what jscpd reports with this server's options: {}. Ask for more with 'kinds': [\"exact\", \"renamed\", \"similar\"] also finds copies with renamed identifiers or edited lines, and \"semantic\" functions that do the same job. Workflow: call check_duplication with code you are about to write, to find existing code to reuse instead; call get_file_clones before refactoring a file; call get_statistics for the duplication percentage; call check_current_directory after editing files, to scan again; call compare_folders to pair the functions of two folders, such as a port and its original or two implementations of one app. Results carry each clone's kind and line ranges; lists come biggest first, capped by 'limit' (default 100) with the full count alongside. A kind that cannot be searched is named under 'unavailable' with the reason: semantic and compare_folders need the embedding model, which is downloaded only when the user agrees (`jscpd --semantic-download`). The tools never change the project's files. {}",
        project.defaults().names().join(", "),
        AST_METHOD
    )
}

/// Read-only, repeatable, project-local: the hints of every tool.
fn annotations() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    })
}

fn limit_schema(what: &str, total_key: &str) -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "default": DEFAULT_LIMIT,
        "description": format!(
            "Maximum number of {what} in the response. '{total_key}' always carries the full count, so a small limit still tells you how much was found."
        )
    })
}

fn kinds_schema(project: &Project) -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "string",
            "enum": ["exact", "renamed", "similar", "gap", "ast", "semantic", "type1", "type2", "type3", "type4"]
        },
        "uniqueItems": true,
        "description": format!(
            "The kinds of clone to look for: exact (type1), renamed (type2), similar (type3; 'gap' or 'ast' for one of its two mechanisms) and semantic (type4). Without it: {}, what jscpd reports with this server's options. Renamed and similar clones may need one scan more, and semantic ones run an embedding model, which takes a while the first time.",
            project.defaults().names().join(", ")
        ),
        "examples": [["exact", "renamed", "similar"], ["semantic"]]
    })
}

pub(super) fn definitions(project: &Project) -> Value {
    let annotations = annotations();
    let kinds = kinds_schema(project);
    json!([
        {
            "name": "check_duplication",
            "title": "Check a snippet for duplication",
            "description": "Check whether a code snippet duplicates code that already exists in the scanned project. Use it before writing or committing a function, class or block, to find the existing code you should reuse instead; pass kinds [\"exact\", \"renamed\", \"similar\"] to find copies with other names or edited lines as well. Returns {format, kinds, count, returned, duplications[]}, each duplication with 'kind', 'file', 'fileStartLine', 'fileEndLine', 'snippetStartLine', 'snippetEndLine' and 'tokens'; similar and semantic ones add 'similarity' (and 'method': gap or ast), and ast and semantic ones add 'name' (the name of the project's function) and 'snippetName'. Exact matches come first, then renamed, similar and semantic ones. A request without 'kinds' gets its ast matches under 'similar' with 'similarCount', as earlier versions answered. A snippet shorter than the server's --min-tokens (50 by default) cannot match and gets a 'note' saying so. Kinds that could not be searched are listed under 'unavailable' with the reason. The snippet is compared with the last scan: call check_current_directory first if files changed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "The source code to check, verbatim. Whole functions or blocks work best."
                    },
                    "format": {
                        "type": "string",
                        "description": "Language of the snippet: a jscpd format name such as 'javascript', 'typescript', 'python', 'java', or a file extension such as 'js', 'py' (`jscpd --list` lists all 224).",
                        "examples": ["javascript", "python", "ts"]
                    },
                    "kinds": kinds,
                    "limit": limit_schema("duplications", "count"),
                    "similarity": {
                        "type": "number",
                        "exclusiveMinimum": 0,
                        "maximum": 1,
                        "description": format!(
                            "The share of subtrees two functions' normalized syntax trees must have in common to be an ast match: 1 finds copies with other names and literals only, 0.8 also one-line edits, 0.7 a couple of added or removed statements. Giving it asks for ast matches. Defaults to the server's --similarity, or 0.8. Functions of JavaScript, TypeScript, Python, Java, Kotlin, Scala, C#, Go, Rust, C, C++, PHP, Ruby, Swift and Clojure. {}",
                            AST_METHOD
                        ),
                        "examples": [0.8]
                    }
                },
                "required": ["code", "format"],
                "additionalProperties": false
            },
            "annotations": annotations
        },
        {
            "name": "get_file_clones",
            "title": "List the clones of one file",
            "description": "List every clone of the last scan that involves one file, to see which other files share code with it. Use it before refactoring, splitting or deleting a file. Returns {file, kinds, clones, returned, duplications[]}, each duplication with 'kind', 'format', 'fileA', 'startA', 'endA', 'fileB', 'startB', 'endB', 'lines' and 'tokens' ('similarity' and 'method' for similar and semantic ones), biggest first; the file may be fileA or fileB. A path that was not part of the scan gets clones 0 and a 'note'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file, relative to the scan root as other results show paths (e.g. 'src/cart.js'), or absolute.",
                        "examples": ["src/cart.js"]
                    },
                    "kinds": kinds,
                    "limit": limit_schema("clones", "clones")
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "annotations": annotations
        },
        {
            "name": "get_statistics",
            "title": "Project duplication statistics",
            "description": "Report the duplication statistics of the last scan, for the clones of the kinds asked: for the whole project and per format, the files, lines and tokens analyzed, the clones, and the duplicated lines and tokens with their percentages, plus the number of clones of each kind ('byKind'). Use it to judge overall duplication, or to compare before and after a refactoring (call check_current_directory in between). It reflects the last scan, not the files on disk now.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kinds": kinds
                },
                "additionalProperties": false
            },
            "annotations": annotations
        },
        {
            "name": "check_current_directory",
            "title": "Scan the project again",
            "description": "Scan the paths the server was started with again and return the fresh clone list and counts. Use it after creating, editing or deleting files, so that the other tools answer from current content. Returns {files, kinds, clones, returned, duplicatedLines, percentage, byKind, duplications[]} with the duplications of get_file_clones, biggest first. Reads the files only, and replaces the previous scan; it can take a few seconds on a large project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kinds": kinds,
                    "limit": limit_schema("clones", "clones")
                },
                "additionalProperties": false
            },
            "annotations": annotations
        },
        {
            "name": "compare_folders",
            "title": "Compare two folders function by function",
            "description": "Compare two folders function by function, in one language or across languages: a port and its original (the source first, the target second), or two implementations of one app (iOS and Android). Every function of one folder is paired with the function of the other that does the same job, by the embedding model, and what has no counterpart is listed. Returns the JSON report of `jscpd --compare`: 'code' and 'tests' sections, each with 'sides' (per folder: 'functions', 'matched', 'percentage', 'files', 'unmatched', 'readyToPort': unported functions whose callees are all ported) and 'pairs' (each with both functions, 'similarity', 'level' high/medium/low, 'renamed' and 'matchedBy' code or name). Read a few low pairs and the pairs matched by name before relying on them. Needs the embedding model; the first comparison of two folders embeds every function and can take minutes, later ones reuse the cached vectors.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "left": {
                        "type": "string",
                        "description": "The first folder (or file): the source of a port, or one of two implementations. It must lie inside the scanned folders: give it relative to one of them, or absolute.",
                        "examples": ["python-lib", "ios/Sources"]
                    },
                    "right": {
                        "type": "string",
                        "description": "The second folder (or file): the target of a port, or the other implementation. It must lie inside the scanned folders too, and must not contain the first or lie inside it.",
                        "examples": ["rust-lib", "android/app/src/main"]
                    },
                    "limit": limit_schema("entries of each list ('pairs', 'unmatched', 'readyToPort', 'files')", "functions")
                },
                "required": ["left", "right"],
                "additionalProperties": false
            },
            "annotations": annotations
        }
    ])
}

/// Run the tool `params` names. `Ok` holds the tool's result, a tool error
/// among them; `Err` is a protocol error.
pub(super) fn call(project: &mut Project, params: &Value) -> Result<Value, Failure> {
    const INVALID_PARAMS: i64 = -32602;
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err(Failure::new(INVALID_PARAMS, "missing tool name"));
    };
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(args)) => args,
        Some(_) => {
            return Err(Failure::new(
                INVALID_PARAMS,
                "'arguments' must be an object",
            ));
        }
    };
    let outcome = match name {
        "check_duplication" => check_duplication(project, args),
        "get_file_clones" => get_file_clones(project, args),
        "get_statistics" => get_statistics(project, args),
        "check_current_directory" => check_current_directory(project, args),
        "compare_folders" => compare_folders(project, args),
        other => {
            return Err(Failure::new(
                INVALID_PARAMS,
                format!("unknown tool '{other}'"),
            ));
        }
    };
    Ok(match outcome {
        Ok(payload) => json!({
            "content": [{ "type": "text", "text": payload.to_string() }],
            "structuredContent": payload,
            "isError": false,
        }),
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    })
}

type Args = Map<String, Value>;

fn string<'a>(args: &'a Args, key: &str, tool: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(format!("{tool}: '{key}' must be a string")),
        None => Err(format!("{tool}: the argument '{key}' is required")),
    }
}

fn limit(args: &Args) -> Result<usize, String> {
    match args.get("limit") {
        None | Some(Value::Null) => Ok(DEFAULT_LIMIT),
        Some(value) => value
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| "'limit' must be a non-negative integer".to_string()),
    }
}

fn kinds(args: &Args, project: &Project) -> Result<Kinds, String> {
    let names = match args.get("kinds") {
        None | Some(Value::Null) => return Ok(project.defaults()),
        Some(Value::Array(names)) => names,
        Some(_) => return Err("'kinds' must be an array of kind names".to_string()),
    };
    let names: Vec<&str> = names
        .iter()
        .map(|name| {
            name.as_str()
                .ok_or("'kinds' must be an array of kind names")
        })
        .collect::<Result<_, _>>()?;
    let kinds = Kinds::parse(names)?;
    match kinds == Kinds::default() {
        true => Err("'kinds' names no kind: leave it out for the server's default".to_string()),
        false => Ok(kinds),
    }
}

/// The kinds that could not be looked for, as a result shows them.
fn unavailable(found: &[(&'static str, String)]) -> Option<Value> {
    (!found.is_empty()).then(|| {
        Value::Object(
            found
                .iter()
                .map(|(kind, why)| (kind.to_string(), Value::from(why.as_str())))
                .collect(),
        )
    })
}

/// How many clones of each kind `clones` holds, similar ones by mechanism.
fn by_kind<'a>(clones: impl IntoIterator<Item = &'a CpdClone>) -> Value {
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    for clone in clones {
        let name = match clone.similarity_method {
            Some(method) => method.as_str(),
            None => clone.kind.as_str(),
        };
        *counts.entry(name).or_default() += 1;
    }
    json!(counts)
}

/// One clone of the project, its paths relative to the scan roots.
fn clone_json(project: &Project, clone: &CpdClone) -> Value {
    let (a, b) = (&clone.fragment_a, &clone.fragment_b);
    let mut value = json!({
        "kind": clone.kind.as_str(),
        "format": clone.format,
        "fileA": project.display_path(&a.source_id),
        "startA": a.start.line,
        "endA": a.end.line,
        "fileB": project.display_path(&b.source_id),
        "startB": b.start.line,
        "endB": b.end.line,
        "lines": clone.fragment_lines(0),
        "tokens": clone.token_count,
    });
    add_near_miss(&mut value, clone);
    value
}

/// The note of a list cut to `limit` entries, if it was.
fn truncated(total: usize, limit: usize, what: &str) -> Option<String> {
    (total > limit).then(|| format!("{what} truncated to {limit}; pass a higher 'limit' for more"))
}

fn check_duplication(project: &mut Project, args: &Args) -> Result<Value, String> {
    let code = string(args, "code", "check_duplication")?;
    let format = string(args, "format", "check_duplication")?;
    let limit = limit(args)?;
    let kinds = kinds(args, project)?;
    let similarity = match args.get("similarity") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_f64() {
            Some(ratio) if ratio > 0.0 && ratio <= 1.0 => Some(ratio),
            _ => return Err("'similarity' must be a number in (0, 1]".to_string()),
        },
    };
    let Checked {
        format,
        kinds,
        matches,
        note,
        unavailable: mut missing,
    } = project.check(code, format, kinds, similarity)?;
    // A request without `kinds` has the shape of one written for an
    // earlier version, and gets its ast matches where that version put
    // them: under `similar`, apart from the token matches.
    let earlier = !args.contains_key("kinds");
    let (similar, matches): (Vec<&Match>, Vec<&Match>) = matches
        .iter()
        .partition(|m| earlier && m.clone.similarity_method == Some(SimilarityMethod::Ast));
    let duplications: Vec<Value> = matches
        .iter()
        .take(limit)
        .map(|m| {
            let (snippet, file) = sides(project, m);
            let mut value = json!({
                "kind": m.clone.kind.as_str(),
                "file": file.0,
                "fileStartLine": file.1,
                "fileEndLine": file.2,
                "snippetStartLine": snippet.0,
                "snippetEndLine": snippet.1,
                "tokens": m.clone.token_count,
            });
            add_near_miss(&mut value, &m.clone);
            if let Some((name, snippet_name)) = &m.names {
                value["name"] = json!(name);
                value["snippetName"] = json!(snippet_name);
            }
            value
        })
        .collect();
    let mut payload = json!({
        "format": format,
        "kinds": kinds.names(),
        "count": matches.len(),
        "returned": duplications.len(),
        "duplications": duplications,
    });
    if let Some(note) = note.or_else(|| truncated(matches.len(), limit, "match list")) {
        payload["note"] = json!(note);
    }
    if earlier && kinds.ast {
        payload["similarCount"] = json!(similar.len());
        payload["similar"] = Value::Array(
            similar
                .iter()
                .take(limit)
                .map(|m| {
                    let (snippet, file) = sides(project, m);
                    let (name, snippet_name) = m.names.clone().unwrap_or_default();
                    json!({
                        "file": file.0,
                        "name": name,
                        "fileStartLine": file.1,
                        "fileEndLine": file.2,
                        "snippetName": snippet_name,
                        "snippetStartLine": snippet.0,
                        "snippetEndLine": snippet.1,
                        "similarity": m.clone.similarity_rounded(),
                    })
                })
                .collect(),
        );
        if let Some(at) = missing.iter().position(|(kind, _)| *kind == "ast") {
            payload["similarNote"] = json!(missing.remove(at).1);
        }
    }
    if let Some(missing) = unavailable(&missing) {
        payload["unavailable"] = missing;
    }
    Ok(payload)
}

/// The snippet's lines in a match, then the project's file and lines, the
/// path as results show it.
fn sides(project: &Project, m: &Match) -> ((u32, u32), (String, u32, u32)) {
    let (snippet, file) = match m.clone.fragment_a.source_id == SNIPPET_ID {
        true => (&m.clone.fragment_a, &m.clone.fragment_b),
        false => (&m.clone.fragment_b, &m.clone.fragment_a),
    };
    (
        (snippet.start.line, snippet.end.line),
        (
            project.display_path(&file.source_id),
            file.start.line,
            file.end.line,
        ),
    )
}

/// The fields of a clone list: the kinds, the counts and the clones, with
/// the kinds that could not be looked for.
fn clone_list(
    project: &Project,
    clones: &[&CpdClone],
    unavailable_kinds: &[(&'static str, String)],
    kinds: Kinds,
    limit: usize,
) -> Map<String, Value> {
    let duplications: Vec<Value> = clones
        .iter()
        .take(limit)
        .map(|c| clone_json(project, c))
        .collect();
    let mut fields = Map::new();
    fields.insert("kinds".into(), json!(kinds.names()));
    fields.insert("clones".into(), json!(clones.len()));
    fields.insert("returned".into(), json!(duplications.len()));
    fields.insert("byKind".into(), by_kind(clones.iter().copied()));
    fields.insert("duplications".into(), Value::Array(duplications));
    if let Some(note) = truncated(clones.len(), limit, "clone list") {
        fields.insert("note".into(), json!(note));
    }
    if let Some(missing) = unavailable(unavailable_kinds) {
        fields.insert("unavailable".into(), missing);
    }
    fields
}

fn get_file_clones(project: &mut Project, args: &Args) -> Result<Value, String> {
    let path = string(args, "path", "get_file_clones")?;
    let limit = limit(args)?;
    let kinds = kinds(args, project)?;
    let wanted = path.replace('\\', "/").trim_start_matches("./").to_string();
    let absolute = std::fs::canonicalize(path)
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let found = project.clones(kinds);
    let involves = |id: &str| {
        project.display_path(id) == wanted
            || absolute.as_deref() == Some(crate::index::host_file(id))
    };
    let clones: Vec<&CpdClone> = found
        .clones
        .iter()
        .filter(|c| involves(&c.fragment_a.source_id) || involves(&c.fragment_b.source_id))
        .collect();
    let mut payload = Map::new();
    payload.insert("file".into(), json!(wanted));
    payload.extend(clone_list(
        project,
        &clones,
        &found.unavailable,
        kinds,
        limit,
    ));
    if clones.is_empty() && !project.scanned(kinds, &wanted) {
        payload.insert(
            "note".into(),
            json!("the file was not part of the scan: pass a path relative to the scan root, as other results show paths, or an absolute path"),
        );
    }
    Ok(Value::Object(payload))
}

fn get_statistics(project: &mut Project, args: &Args) -> Result<Value, String> {
    let kinds = kinds(args, project)?;
    let found = project.clones(kinds);
    let mut payload = json!({
        "files": found.files,
        "kinds": kinds.names(),
        "clones": found.clones.len(),
        "byKind": by_kind(&found.clones),
        "statistics": &found.statistics,
    });
    if let Some(missing) = unavailable(&found.unavailable) {
        payload["unavailable"] = missing;
    }
    Ok(payload)
}

fn check_current_directory(project: &mut Project, args: &Args) -> Result<Value, String> {
    let limit = limit(args)?;
    let kinds = kinds(args, project)?;
    project.rescan(kinds);
    let found = project.clones(kinds);
    let mut payload = Map::new();
    payload.insert("files".into(), json!(found.files));
    payload.insert(
        "duplicatedLines".into(),
        json!(found.statistics.total.duplicated_lines),
    );
    payload.insert(
        "percentage".into(),
        json!(found.statistics.total.percentage),
    );
    let clones: Vec<&CpdClone> = found.clones.iter().collect();
    payload.extend(clone_list(
        project,
        &clones,
        &found.unavailable,
        kinds,
        limit,
    ));
    Ok(Value::Object(payload))
}

fn compare_folders(project: &mut Project, args: &Args) -> Result<Value, String> {
    let left = string(args, "left", "compare_folders")?;
    let right = string(args, "right", "compare_folders")?;
    let limit = limit(args)?;
    let mut report = project.compare(left, right)?;
    let mut cut = Vec::new();
    for section in ["code", "tests"] {
        let Some(section_value) = report.get_mut(section) else {
            continue;
        };
        cut_list(
            section_value,
            "pairs",
            limit,
            &format!("{section}.pairs"),
            &mut cut,
        );
        if let Some(Value::Array(sides)) = section_value.get_mut("sides") {
            for (index, side) in sides.iter_mut().enumerate() {
                for key in ["files", "unmatched", "readyToPort"] {
                    let name = format!("{section}.sides[{index}].{key}");
                    cut_list(side, key, limit, &name, &mut cut);
                }
            }
        }
    }
    report["model"] = json!(project.model());
    if !cut.is_empty() {
        report["note"] = json!(format!(
            "lists cut to {limit} entries: {}; pass a higher 'limit' for more",
            cut.join(", ")
        ));
    }
    Ok(report)
}

/// Cut the list `value[key]` to `limit` entries, noting its full length.
fn cut_list(value: &mut Value, key: &str, limit: usize, name: &str, cut: &mut Vec<String>) {
    if let Some(Value::Array(list)) = value.get_mut(key)
        && list.len() > limit
    {
        cut.push(format!("{name} ({})", list.len()));
        list.truncate(limit);
    }
}
