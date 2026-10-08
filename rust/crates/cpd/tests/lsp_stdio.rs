// lsp_stdio.rs — end-to-end tests of `cpd --lsp`: spawn the real binary and
// speak the Language Server Protocol over its stdin and stdout (#1120).

mod common;

use lsp_server::{Message, Notification, Request, RequestId, Response};
use serde_json::{Value, json};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn cpd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cpd"))
}

/// A client of one server process.
struct Lsp {
    child: Child,
    stdin: ChildStdin,
    messages: mpsc::Receiver<Message>,
    next_id: i32,
}

impl Lsp {
    fn start(dir: &Path, args: &[&str]) -> Self {
        Self::start_with_env(dir, args, &[])
    }

    fn start_with_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Self {
        let mut child = Command::new(cpd_bin())
            .arg("--lsp")
            .args(args)
            .current_dir(dir)
            // Vectors of the semantic analysis go beside the workspace.
            .env("JSCPD_CACHE_DIR", dir.with_extension("cache"))
            .env_remove("JSCPD_SEMANTIC_API_KEY")
            // A model download, if any, goes nowhere.
            .env("HF_ENDPOINT", "http://127.0.0.1:9")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cpd --lsp");
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(Some(message)) = Message::read(&mut stdout) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            messages,
            next_id: 0,
        }
    }

    fn send(&mut self, message: Message) {
        message.write(&mut self.stdin).expect("write to the server");
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(Message::Notification(Notification::new(
            method.to_string(),
            params,
        )));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.send(Message::Request(Request::new(
            id.clone(),
            method.to_string(),
            params,
        )));
        match self.wait(|m| matches!(m, Message::Response(r) if r.id == id)) {
            Message::Response(response) => response.response_result.expect("an ok response"),
            _ => unreachable!(),
        }
    }

    /// The next message `accept` takes, answering the server's own requests
    /// on the way (progress, registrations, showDocument) with `null`. A
    /// request `accept` takes is left for the caller to answer.
    fn wait(&mut self, accept: impl Fn(&Message) -> bool) -> Message {
        self.wait_seeing(accept, |_| {})
    }

    /// [`Self::wait`], with every message before the one taken shown to
    /// `seen`.
    fn wait_seeing(
        &mut self,
        accept: impl Fn(&Message) -> bool,
        mut seen: impl FnMut(&Message),
    ) -> Message {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self
                .messages
                .recv_timeout(left)
                .expect("the server answered in time");
            if accept(&message) {
                return message;
            }
            if let Message::Request(request) = &message {
                let answer = Response::new_ok(request.id.clone(), Value::Null);
                self.send(Message::Response(answer));
            }
            seen(&message);
        }
    }

    /// The messages the server sends before its answer to a request, and
    /// the answer: what handling the notifications sent before it caused.
    fn messages_before(&mut self, method: &str, params: Value) -> (Vec<Message>, Response) {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.send(Message::Request(Request::new(
            id.clone(),
            method.to_string(),
            params,
        )));
        let mut before = Vec::new();
        let response = self.wait_seeing(
            |m| matches!(m, Message::Response(r) if r.id == id),
            |m| before.push(m.clone()),
        );
        match response {
            Message::Response(response) => (before, response),
            _ => unreachable!(),
        }
    }

    /// The diagnostics published for `uri` once `accept` takes them.
    fn diagnostics(&mut self, uri: &str, accept: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let message = self.wait(|m| match m {
            Message::Notification(n) if n.method == "textDocument/publishDiagnostics" => {
                n.params["uri"] == uri && accept(n.params["diagnostics"].as_array().unwrap())
            }
            _ => false,
        });
        match message {
            Message::Notification(n) => n.params["diagnostics"].as_array().unwrap().clone(),
            _ => unreachable!(),
        }
    }

    fn initialize(&mut self, folders: &[&Path], options: Value) {
        self.initialize_with(
            folders,
            options,
            json!({
                "textDocument": {"publishDiagnostics": {"relatedInformation": true}},
                "window": {"showDocument": {"support": true}},
            }),
        );
    }

    /// `initialize` with these client `capabilities`; the server's answer.
    fn initialize_with(&mut self, folders: &[&Path], options: Value, capabilities: Value) -> Value {
        let folders: Vec<Value> = folders
            .iter()
            .map(|f| json!({"uri": uri(f), "name": "ws"}))
            .collect();
        let result = self.request(
            "initialize",
            json!({
                "processId": null,
                "workspaceFolders": folders,
                "initializationOptions": options,
                "capabilities": capabilities,
            }),
        );
        self.notify("initialized", json!({}));
        result
    }

    fn change(&mut self, uri: &str, version: i32, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]}),
        );
    }

    fn close(&mut self, uri: &str) {
        self.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": uri}}),
        );
    }

    /// The code actions over `range` of the document at `uri`.
    fn actions(&mut self, uri: &str, range: &Value) -> Vec<Value> {
        self.request(
            "textDocument/codeAction",
            json!({"textDocument": {"uri": uri}, "range": range, "context": {"diagnostics": []}}),
        )
        .as_array()
        .cloned()
        .unwrap_or_default()
    }

    fn open(&mut self, path: &Path) -> String {
        let uri = uri(path);
        let text = std::fs::read_to_string(path).unwrap();
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "x", "version": 1, "text": text}}),
        );
        uri
    }

    /// The response to a request, error or not.
    fn respond_to(&mut self, method: &str, params: Value) -> Response {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.send(Message::Request(Request::new(
            id.clone(),
            method.to_string(),
            params,
        )));
        match self.wait(|m| matches!(m, Message::Response(r) if r.id == id)) {
            Message::Response(response) => response,
            _ => unreachable!(),
        }
    }

    fn exit_code(mut self) -> Option<i32> {
        self.notify("exit", Value::Null);
        self.child.wait().unwrap().code()
    }

    fn shutdown(mut self) -> i32 {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        self.child.wait().unwrap().code().unwrap_or(-1)
    }
}

fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

/// A fresh folder with `files`, canonical like the paths the server sees.
fn workspace(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpd-lsp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::canonicalize(dir).unwrap()
}

/// Remove a workspace and the cache beside it.
fn remove(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir.with_extension("cache"));
}

fn codes(diagnostics: &[Value]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|d| d["code"].as_str().unwrap_or(""))
        .collect()
}

const TOTAL: &str = "export function total(items) {\n  let sum = 0;\n  for (const item of items) {\n    if (item.price > 0 && item.count > 0) {\n      sum += item.price * item.count;\n    }\n  }\n  return Math.round(sum * 100) / 100;\n}\n";
const SMALL: &str = r#"{"minTokens": 20, "minLines": 3}"#;

#[test]
fn a_clone_is_a_diagnostic_until_an_edit_takes_it_away() {
    let dir = workspace(
        "clones",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    assert_eq!(codes(&diagnostics), ["jscpd/duplicate-code"]);
    let clone = &diagnostics[0];
    assert_eq!(clone["source"], "jscpd");
    assert_eq!(clone["severity"], 2, "a clone is a warning");
    assert!(
        clone["message"].as_str().unwrap().contains("b.js:1-9"),
        "{clone}"
    );
    let related = &clone["relatedInformation"][0]["location"];
    assert!(
        related["uri"].as_str().unwrap().ends_with("/b.js"),
        "{related}"
    );

    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": a}, "position": clone["range"]["start"]}),
    );
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("return Math.round")
    );
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": a}, "range": clone["range"], "context": {"diagnostics": [clone]}}),
    );
    let titles: Vec<&str> = actions
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    assert!(
        titles.iter().any(|t| t.starts_with("Go to the other copy")),
        "{titles:?}"
    );
    let ignore = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["edit"].is_object())
        .expect("an ignore action");
    let edits = &ignore["edit"]["changes"][&a];
    assert_eq!(edits[0]["newText"], "// jscpd:ignore-start\n");
    // A client with a view of its own asks for the clones as
    // jscpd-report.json lists them.
    let report = lsp.request("jscpd/clones", Value::Null);
    let project = &report["projects"][0];
    assert_eq!(project["duplicates"].as_array().unwrap().len(), 1);
    assert_eq!(project["duplicates"][0]["kind"], "exact");
    assert_eq!(project["statistics"]["total"]["clones"], 1);

    // The buffer, not the disk: the clone goes once the editor's text
    // no longer has it.
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": a, "version": 2}, "contentChanges": [{"text": "export const x = 1;\n"}]}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

const ORDERS_PY: &str = "class Orders:\n    def __init__(self, db, cache):\n        self.db = db\n        self.cache = cache\n\n    def load(self, order_id):\n        key = f\"order:{order_id}\"\n        order = self.cache.get(key)\n        if order is None:\n            order = self.db.fetch(\"orders\", order_id)\n            self.cache.set(key, order)\n        return order\n";

#[test]
fn a_pair_of_methods_is_named_as_functions() {
    let invoices = ORDERS_PY
        .replace("Orders", "Invoices")
        .replace("order", "invoice")
        .replace("cache", "memo");
    let dir = workspace(
        "units",
        &[
            ("orders.py", ORDERS_PY),
            ("invoices.py", &invoices),
            (".jscpd.json", SMALL),
        ],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(
        &[&dir],
        json!({"lsp": {"clones": {"enabled": false}, "ast": {"enabled": true}}}),
    );
    let orders = lsp.open(&dir.join("orders.py"));
    let diagnostics = lsp.diagnostics(&orders, |d| !d.is_empty());
    assert_eq!(
        codes(&diagnostics),
        ["jscpd/similar-function"],
        "the method that loads; a class is no unit"
    );
    let pair = &diagnostics[0];
    let message = pair["message"].as_str().unwrap();
    assert!(
        message.starts_with("Same structure as the function at invoices.py:6-12"),
        "{message}"
    );
    assert_eq!(
        pair["relatedInformation"][0]["message"],
        "The similar function"
    );
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": orders}, "range": pair["range"], "context": {"diagnostics": [pair]}}),
    );
    assert_eq!(
        actions[0]["title"],
        "Go to the similar function in invoices.py:6-12"
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_copy_changed_on_disk_takes_the_clone_away() {
    let dir = workspace(
        "watched",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    // A checkout rewrites the other copy, which is not open.
    std::fs::write(dir.join("b.js"), "export const unrelated = 1;\n").unwrap();
    lsp.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri(&dir.join("b.js")), "type": 2}]}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn config_files_split_the_workspace_into_projects() {
    let dir = workspace("projects", &[("one/a.js", TOTAL), ("two/b.js", TOTAL)]);
    let (one, two) = (dir.join("one"), dir.join("two"));
    let mut lsp = Lsp::start(&dir, &["--min-tokens", "20", "--min-lines", "3"]);
    lsp.initialize(
        &[&one, &two],
        json!({"lsp": {"clones": {"warningTokens": 1000}}}),
    );
    let a = lsp.open(&one.join("a.js"));
    // No config: both folders are one project, and the copy across them
    // is a clone.
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    assert!(diagnostics[0]["message"].as_str().unwrap().contains("b.js"));
    assert_eq!(
        diagnostics[0]["severity"], 3,
        "below warningTokens, information"
    );
    // A config makes its folder a project of its own.
    std::fs::write(two.join(".jscpd.json"), "{}").unwrap();
    lsp.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri(&two.join(".jscpd.json")), "type": 1}]}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn each_analysis_has_a_switch_of_its_own() {
    let main = "import { used, unused } from \"./lib.js\";\nexport function route(req, res) {\n  if (req.method === \"GET\" && req.user) {\n    for (const item of req.items) {\n      if (item.ok || item.retry) { res.push(item); }\n    }\n  } else if (req.method === \"POST\") {\n    return req.body ? used : null;\n  }\n  return res;\n}\n";
    let dir = workspace(
        "switches",
        &[
            ("main.js", main),
            (
                "lib.js",
                "export const used = 1;\nexport const unused = 2;\n",
            ),
            (".jscpd.json", r#"{"entry": ["main.js"]}"#),
        ],
    );
    let mut lsp = Lsp::start(&dir, &["--lsp-analyses", "complexity"]);
    lsp.initialize(
        &[&dir],
        json!({"lsp": {"deadCode": {"enabled": true}, "complexity": {"functionLimit": 3}}}),
    );
    let main = lsp.open(&dir.join("main.js"));
    // Dead code arrives from the background: wait until both are there.
    let diagnostics = lsp.diagnostics(&main, |d| {
        let codes = codes(d);
        codes.contains(&"unused-import") && codes.contains(&"jscpd/complex-function")
    });
    let unused = diagnostics
        .iter()
        .find(|d| d["code"] == "unused-import")
        .unwrap();
    assert_eq!(unused["tags"], json!([1]), "faded, not underlined");
    assert_eq!(unused["severity"], 4);
    let report = lsp.request("jscpd/deadCode", Value::Null);
    let finding = &report["projects"][0]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["category"] == "unused-import")
        .expect("the unused import in the report")["path"];
    assert_eq!(
        finding.as_str().unwrap(),
        dir.join("main.js").to_str().unwrap()
    );
    let report = lsp.request("jscpd/complexity", Value::Null);
    let files = report["projects"][0]["summary"]["files"]
        .as_array()
        .unwrap();
    let main_path = dir.join("main.js");
    assert!(
        files
            .iter()
            .any(|f| f["path"].as_str() == main_path.to_str()),
        "main.js is in the summary: {report}"
    );
    // Switched off from the editor, dead code goes and complexity stays.
    lsp.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"lsp": {"deadCode": {"enabled": false}, "complexity": {"functionLimit": 3}}}}),
    );
    let diagnostics = lsp.diagnostics(&main, |d| !codes(d).contains(&"unused-import"));
    assert_eq!(codes(&diagnostics), ["jscpd/complex-function"]);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_semantic_pair_is_a_diagnostic_on_each_function() {
    let server = common::embeddings::Server::start();
    let dir = workspace("semantic", &[]);
    common::embeddings::cart_project(&dir);
    let mut lsp = Lsp::start(
        &dir,
        &[
            "--semantic-url",
            &server.url,
            "--semantic-model",
            "stand-in",
            "--min-tokens",
            "15",
            "--min-lines",
            "3",
        ],
    );
    lsp.initialize(
        &[&dir],
        json!({"lsp": {"clones": {"enabled": false}, "semantic": {"enabled": true}}}),
    );
    let cart = lsp.open(&dir.join("backend/src/cart.rs"));
    let diagnostics = lsp.diagnostics(&cart, |d| !d.is_empty());
    assert_eq!(codes(&diagnostics), ["jscpd/semantic-code"]);
    let message = diagnostics[0]["message"].as_str().unwrap();
    assert!(message.contains("Cart.svelte"), "{message}");
    // Over the function's first line, not its whole body.
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 0);
    assert_eq!(diagnostics[0]["range"]["end"]["line"], 0);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_mode_of_the_command_line_points_at_its_analysis() {
    let output = Command::new(cpd_bin())
        .args(["--lsp", "--dead-code"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--lsp-analyses dead-code"), "{stderr}");
}

#[test]
fn files_the_walk_skips_stay_out_when_they_appear_or_open() {
    let dir = workspace(
        "ignored",
        &[
            ("src/a.js", TOTAL),
            (".gitignore", "dist/\n"),
            (".jscpd.json", SMALL),
        ],
    );
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    lsp.open(&dir.join("src/a.js"));
    // A build writes a copy into the ignored folder, and the editor opens it.
    std::fs::create_dir_all(dir.join("dist")).unwrap();
    std::fs::write(dir.join("dist/a.js"), TOTAL).unwrap();
    lsp.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri(&dir.join("dist/a.js")), "type": 1}]}),
    );
    lsp.open(&dir.join("dist/a.js"));
    let report = lsp.request("jscpd/clones", Value::Null);
    assert_eq!(report["projects"][0]["duplicates"], json!([]), "{report}");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_folder_deleted_at_once_takes_its_clones_along() {
    let dir = workspace(
        "folder",
        &[
            ("src/a.js", TOTAL),
            ("old/b.js", TOTAL),
            (".jscpd.json", SMALL),
        ],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("src/a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    std::fs::remove_dir_all(dir.join("old")).unwrap();
    lsp.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": uri(&dir.join("old")), "type": 3}]}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn shutdown_and_exit_follow_the_protocol() {
    let dir = workspace("lifecycle", &[("a.js", TOTAL)]);
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    lsp.request("shutdown", Value::Null);
    let refused = lsp.respond_to("jscpd/statistics", Value::Null);
    assert_eq!(
        refused.response_result.err().map(|e| e.code),
        Some(lsp_server::ErrorCode::InvalidRequest as i32)
    );
    assert_eq!(lsp.exit_code(), Some(0));
    // `exit` without `shutdown` is an error exit.
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    assert_eq!(lsp.exit_code(), Some(1));
    remove(&dir);
}

#[test]
fn dead_code_waits_for_a_save_after_an_edit() {
    let lib = "export const used = 1;\nexport function unused() {\n  return 2;\n}\n";
    let dir = workspace(
        "dead-edit",
        &[
            (
                "main.js",
                "import { used } from \"./lib.js\";\nconsole.log(used);\n",
            ),
            ("lib.js", lib),
            (".jscpd.json", r#"{"entry": ["main.js"]}"#),
        ],
    );
    let mut lsp = Lsp::start(&dir, &["--lsp-analyses", "dead-code"]);
    lsp.initialize(&[&dir], json!({}));
    let uri_lib = lsp.open(&dir.join("lib.js"));
    let unused = |d: &[Value]| d.iter().any(|d| d["code"] == "unused-export");
    let found = lsp.diagnostics(&uri_lib, unused);
    let line = |d: &[Value]| {
        d.iter().find(|d| d["code"] == "unused-export").unwrap()["range"]["start"]["line"].clone()
    };
    assert_eq!(line(&found), 1);
    // Two lines typed on top: the run's offsets no longer fit the buffer.
    let edited = format!("// one\n// two\n{lib}");
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri_lib, "version": 2}, "contentChanges": [{"text": edited}]}),
    );
    lsp.diagnostics(&uri_lib, |d| !unused(d));
    // Saved, a new run puts it back on its new line.
    std::fs::write(dir.join("lib.js"), &edited).unwrap();
    lsp.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": uri_lib}}),
    );
    let found = lsp.diagnostics(&uri_lib, unused);
    assert_eq!(line(&found), 3);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

// ------------------------------------------------------------ helpers

/// The notifications of `method` among `messages`, by their params.
fn notifications<'m>(messages: &'m [Message], method: &str) -> Vec<&'m Value> {
    messages
        .iter()
        .filter_map(|m| match m {
            Message::Notification(n) if n.method == method => Some(&n.params),
            _ => None,
        })
        .collect()
}

/// The requests of `method` among `messages`, by their params.
fn requests<'m>(messages: &'m [Message], method: &str) -> Vec<&'m Value> {
    messages
        .iter()
        .filter_map(|m| match m {
            Message::Request(r) if r.method == method => Some(&r.params),
            _ => None,
        })
        .collect()
}

/// The texts of the `window/showMessage` notifications among `messages`.
fn shown(messages: &[Message]) -> Vec<(i64, String)> {
    notifications(messages, "window/showMessage")
        .into_iter()
        .map(|p| {
            (
                p["type"].as_i64().unwrap(),
                p["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Whether the server scanned the workspace again: each load ends with a
/// summary in the log.
fn reloaded(messages: &[Message]) -> bool {
    notifications(messages, "window/logMessage")
        .iter()
        .any(|p| p["message"].as_str().unwrap().contains(" files, "))
}

/// The edits of a workspace edit for `uri`, from `changes` or from
/// `documentChanges`.
fn edits_of<'e>(edit: &'e Value, uri: &str) -> &'e Vec<Value> {
    match edit["changes"][uri].as_array() {
        Some(edits) => edits,
        None => edit["documentChanges"][0]["edits"].as_array().unwrap(),
    }
}

/// The action of `actions` that edits the file: "Ignore this clone".
fn ignore_action(actions: &[Value]) -> &Value {
    actions
        .iter()
        .find(|a| a["edit"].is_object())
        .unwrap_or_else(|| panic!("an ignore action in {actions:?}"))
}

// ------------------------------------------------------------ initialize

#[test]
fn an_initialize_with_fields_in_unknown_forms_is_taken() {
    let dir = workspace(
        "init-forms",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    // A processId of -1 and a trace of `compact` are outside what the
    // protocol types allow; the server reads neither.
    let result = lsp.request(
        "initialize",
        json!({
            "processId": -1,
            "trace": "compact",
            "rootUri": uri(&dir),
            "capabilities": {},
        }),
    );
    lsp.notify("initialized", json!({}));
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-16");
    assert_eq!(result["serverInfo"]["name"], "jscpd");
    // The root URI, without workspace folders, is the workspace.
    let report = lsp.request("jscpd/statistics", Value::Null);
    assert_eq!(report["projects"][0]["files"], 2, "{report}");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn an_initialize_that_does_not_parse_is_refused() {
    let dir = workspace("init-bad", &[]);
    let mut lsp = Lsp::start(&dir, &[]);
    let response = lsp.respond_to("initialize", json!({"capabilities": 5}));
    let error = response.response_result.expect_err("an error response");
    assert_eq!(error.code, lsp_server::ErrorCode::InvalidParams as i32);
    assert!(
        error.message.starts_with("initialize:"),
        "{}",
        error.message
    );
    assert_eq!(lsp.child.wait().unwrap().code(), Some(1));
    remove(&dir);
}

// ------------------------------------------------------------ positions

/// A copy of [`TOTAL`] whose first line starts with a comment holding a
/// character outside the Basic Multilingual Plane: 1 char, 2 UTF-16 code
/// units, 4 bytes. The clone starts after it.
const TOTAL_AFTER_CLEF: &str = "/* \u{1D11E} */ export function total(items) {\n  let sum = 0;\n  for (const item of items) {\n    if (item.price > 0 && item.count > 0) {\n      sum += item.price * item.count;\n    }\n  }\n  return Math.round(sum * 100) / 100;\n}\n";

/// The column where the clone starts on the first line of `a.js`, and the
/// position of the edit that opens "Ignore this clone", as a client that
/// offers `encodings` sees them; also the encoding the server chose.
fn clone_start_column(name: &str, encodings: Value) -> (String, Value, Value, String) {
    let dir = workspace(
        name,
        &[
            ("a.js", TOTAL_AFTER_CLEF),
            ("b.js", TOTAL),
            (".jscpd.json", SMALL),
        ],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    let result = lsp.initialize_with(
        &[&dir],
        json!({}),
        json!({"general": {"positionEncodings": encodings}}),
    );
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let range = diagnostics[0]["range"].clone();
    let actions = lsp.actions(&a, &range);
    let edits = edits_of(&ignore_action(&actions)["edit"], &a).clone();
    // Hovering where the clone starts finds it.
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": a}, "position": range["start"]}),
    );
    assert!(hover["contents"]["value"].is_string(), "{hover}");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
    (
        result["capabilities"]["positionEncoding"]
            .as_str()
            .unwrap()
            .to_string(),
        range["start"].clone(),
        edits[0]["range"]["start"].clone(),
        edits[0]["newText"].as_str().unwrap().to_string(),
    )
}

#[test]
fn columns_count_utf16_code_units_by_default() {
    let (encoding, start, edit, text) = clone_start_column("utf16", json!(["utf-16"]));
    assert_eq!(encoding, "utf-16");
    // `/* ` 3, the clef 2 (a surrogate pair), ` */ ` 4.
    assert_eq!(start, json!({"line": 0, "character": 9}));
    // Code before the clone on its line: a block comment beside it, at
    // the clone's start, so the comment does not take that code along.
    assert_eq!(edit, json!({"line": 0, "character": 9}));
    assert_eq!(text, "/* jscpd:ignore-start */ ");
}

#[test]
fn columns_count_bytes_for_a_client_that_offers_utf8() {
    let (encoding, start, edit, text) = clone_start_column("utf8", json!(["utf-16", "utf-8"]));
    assert_eq!(encoding, "utf-8");
    // `/* ` 3, the clef 4, ` */ ` 4.
    assert_eq!(start, json!({"line": 0, "character": 11}));
    assert_eq!(edit, json!({"line": 0, "character": 11}));
    assert_eq!(text, "/* jscpd:ignore-start */ ");
}

#[test]
fn an_ignore_edit_keeps_the_line_breaks_of_the_file() {
    let crlf = TOTAL.replace('\n', "\r\n");
    let dir = workspace(
        "crlf",
        &[("a.js", &crlf), ("b.js", &crlf), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let range = &diagnostics[0]["range"];
    assert_eq!(range["start"], json!({"line": 0, "character": 0}));
    assert_eq!(range["end"]["line"], 8, "{range}");
    let actions = lsp.actions(&a, range);
    let edits = edits_of(&ignore_action(&actions)["edit"], &a);
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert_eq!(edits[0]["newText"], "// jscpd:ignore-start\r\n");
    assert_eq!(
        edits[0]["range"]["start"],
        json!({"line": 0, "character": 0})
    );
    // On a line of its own after the clone's last line.
    assert_eq!(edits[1]["newText"], "// jscpd:ignore-end\r\n");
    assert_eq!(
        edits[1]["range"]["start"],
        json!({"line": 9, "character": 0})
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn an_ignore_edit_at_the_end_of_a_file_without_a_last_line_break() {
    let dir = workspace(
        "no-eol",
        &[
            ("a.js", TOTAL.trim_end()),
            ("b.js", TOTAL),
            (".jscpd.json", SMALL),
        ],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let actions = lsp.actions(&a, &diagnostics[0]["range"]);
    let edits = edits_of(&ignore_action(&actions)["edit"], &a);
    // No line after the clone: the end comment goes after its last
    // character, on a new line.
    assert_eq!(edits[1]["newText"], "\n// jscpd:ignore-end");
    assert_eq!(
        edits[1]["range"]["start"],
        json!({"line": 8, "character": 1})
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn an_ignore_edit_beside_code_after_the_clone_is_a_block_comment() {
    let tail = TOTAL.trim_end().to_string() + " const after = [\"é\"];\n";
    let dir = workspace(
        "block-end",
        &[("a.js", &tail), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let range = diagnostics[0]["range"].clone();
    let actions = lsp.actions(&a, &range);
    let edits = edits_of(&ignore_action(&actions)["edit"], &a);
    assert_eq!(edits[1]["newText"], " /* jscpd:ignore-end */");
    assert_eq!(edits[1]["range"]["start"], range["end"]);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn an_ignore_edit_names_the_version_for_a_client_that_takes_document_changes() {
    let dir = workspace(
        "versioned",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize_with(
        &[&dir],
        json!({}),
        json!({"workspace": {"workspaceEdit": {"documentChanges": true}}}),
    );
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let range = diagnostics[0]["range"].clone();
    let actions = lsp.actions(&a, &range);
    let edit = &ignore_action(&actions)["edit"];
    assert!(edit["changes"].is_null(), "{edit}");
    assert_eq!(edit["documentChanges"][0]["textDocument"]["uri"], a);
    assert_eq!(edit["documentChanges"][0]["textDocument"]["version"], 1);
    // An edit that keeps the clone: the next edit is for its version.
    lsp.change(&a, 7, &format!("{TOTAL}// more\n"));
    let actions = lsp.actions(&a, &range);
    let edit = &ignore_action(&actions)["edit"];
    assert_eq!(edit["documentChanges"][0]["textDocument"]["version"], 7);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

// ------------------------------------------------------------ requests

#[test]
fn a_code_action_does_not_wait_for_the_quiet_period_after_an_edit() {
    let dir = workspace(
        "action-flush",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let range = diagnostics[0]["range"].clone();
    // Asked at once after an edit that takes the clone away: the actions
    // are for the text as edited, not for the clone that was there.
    lsp.change(&a, 2, "export const x = 1;\n");
    assert_eq!(lsp.actions(&a, &range), Vec::<Value>::new());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn go_to_the_other_copy_asks_the_client_to_show_it() {
    let dir = workspace(
        "show",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    let actions = lsp.actions(&a, &diagnostics[0]["range"]);
    let command = actions
        .iter()
        .find_map(|a| a["command"].as_object())
        .expect("a go-to action")
        .clone();
    assert_eq!(command["command"], "jscpd.showLocation");
    let (before, response) = lsp.messages_before(
        "workspace/executeCommand",
        json!({"command": command["command"], "arguments": command["arguments"]}),
    );
    assert_eq!(response.response_result.unwrap(), Value::Null);
    let shown = requests(&before, "window/showDocument");
    assert_eq!(shown.len(), 1, "{before:?}");
    assert_eq!(shown[0]["uri"], uri(&dir.join("b.js")));
    assert_eq!(shown[0]["takeFocus"], true);
    assert_eq!(shown[0]["selection"]["start"]["line"], 0);
    assert_eq!(shown[0]["selection"]["end"]["line"], 8);

    // Another command, or one without its arguments, does nothing.
    for params in [
        json!({"command": "jscpd.other", "arguments": command["arguments"]}),
        json!({"command": "jscpd.showLocation", "arguments": [uri(&dir.join("b.js"))]}),
        json!({"command": "jscpd.showLocation", "arguments": [1, 2]}),
    ] {
        let (before, response) = lsp.messages_before("workspace/executeCommand", params);
        assert_eq!(response.response_result.unwrap(), Value::Null);
        assert!(requests(&before, "window/showDocument").is_empty());
    }
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn go_to_the_other_copy_shows_nothing_to_a_client_without_show_document() {
    let dir = workspace(
        "no-show",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize_with(&[&dir], json!({}), json!({}));
    let target = json!({"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}});
    let (before, response) = lsp.messages_before(
        "workspace/executeCommand",
        json!({"command": "jscpd.showLocation", "arguments": [uri(&dir.join("b.js")), target]}),
    );
    assert_eq!(response.response_result.unwrap(), Value::Null);
    assert!(requests(&before, "window/showDocument").is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn requests_the_server_does_not_know_or_cannot_read_are_errors() {
    let dir = workspace("errors", &[("a.js", TOTAL)]);
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let unknown = lsp.respond_to("textDocument/definition", json!({}));
    assert_eq!(
        unknown.response_result.err().map(|e| e.code),
        Some(lsp_server::ErrorCode::MethodNotFound as i32)
    );
    let unreadable = lsp.respond_to("textDocument/hover", json!({"position": "here"}));
    assert_eq!(
        unreadable.response_result.err().map(|e| e.code),
        Some(lsp_server::ErrorCode::InvalidParams as i32)
    );
    // A hover where there is no finding is empty.
    let a = lsp.open(&dir.join("a.js"));
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": a}, "position": {"line": 0, "character": 0}}),
    );
    assert_eq!(hover, Value::Null);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn the_reports_of_the_server_answer_per_project() {
    let dir = workspace(
        "reports",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let statistics = lsp.request("jscpd/statistics", Value::Null);
    let project = &statistics["projects"][0];
    assert_eq!(project["files"], 2);
    assert_eq!(project["statistics"]["total"]["clones"], 1);
    assert_eq!(project["roots"], json!([dir]));
    assert_eq!(project["config"], json!(dir.join(".jscpd.json")));
    // Semantic clones are off: no project to report.
    let semantic = lsp.request("jscpd/semantic", Value::Null);
    assert_eq!(semantic, json!({"projects": []}));
    // A rescan reads the disk again.
    std::fs::write(dir.join("c.js"), TOTAL).unwrap();
    let (before, response) = lsp.messages_before("jscpd/rescan", Value::Null);
    assert_eq!(response.response_result.unwrap(), Value::Null);
    assert!(reloaded(&before), "{before:?}");
    let statistics = lsp.request("jscpd/statistics", Value::Null);
    assert_eq!(statistics["projects"][0]["files"], 3);
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_client_with_progress_and_watchers_is_told_what_runs() {
    let dir = workspace(
        "progress",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize_with(
        &[&dir],
        json!({}),
        json!({
            "window": {"workDoneProgress": true},
            "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}},
        }),
    );
    let (before, _) = lsp.messages_before("jscpd/statistics", Value::Null);
    let registrations = requests(&before, "client/registerCapability");
    assert_eq!(registrations.len(), 1, "{before:?}");
    assert_eq!(
        registrations[0]["registrations"][0]["method"],
        "workspace/didChangeWatchedFiles"
    );
    let created = requests(&before, "window/workDoneProgress/create");
    assert_eq!(created.len(), 1, "{before:?}");
    let token = &created[0]["token"];
    let progress: Vec<&Value> = notifications(&before, "$/progress")
        .into_iter()
        .filter(|p| p["token"] == *token)
        .collect();
    assert_eq!(progress.len(), 2, "a begin and an end: {progress:?}");
    assert_eq!(progress[0]["value"]["kind"], "begin");
    assert_eq!(progress[0]["value"]["message"], "Scanning the workspace");
    assert_eq!(progress[1]["value"]["kind"], "end");
    assert_eq!(progress[1]["value"]["message"], "2 files, 1 clone");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

// ------------------------------------------------------------ documents

#[test]
fn closing_a_document_goes_back_to_the_disk_and_clears_its_diagnostics() {
    let dir = workspace(
        "close",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    let b = lsp.open(&dir.join("b.js"));
    lsp.diagnostics(&b, |d| !d.is_empty());
    // The edit, not saved, takes the clone from both copies.
    lsp.change(&a, 2, "export const x = 1;\n");
    lsp.diagnostics(&b, |d| d.is_empty());
    // Closed without a save: the disk still has the copy.
    lsp.close(&a);
    lsp.diagnostics(&b, |d| codes(d) == ["jscpd/duplicate-code"]);
    // A closed file has no diagnostics left in the editor.
    lsp.close(&b);
    lsp.diagnostics(&b, |d| d.is_empty());
    // An edit of a document that is not open changes nothing.
    lsp.change(&a, 3, "export const y = 2;\n");
    let report = lsp.request("jscpd/clones", Value::Null);
    assert_eq!(
        report["projects"][0]["duplicates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_save_runs_the_waiting_edits_at_once() {
    let dir = workspace(
        "save-flush",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    lsp.change(&a, 2, "export const x = 1;\n");
    lsp.notify("textDocument/didSave", json!({"textDocument": {"uri": a}}));
    // No quiet period to wait for: the report right after is the edited one.
    let report = lsp.request("jscpd/clones", Value::Null);
    assert_eq!(report["projects"][0]["duplicates"], json!([]), "{report}");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_config_saved_in_the_editor_applies_at_once() {
    let dir = workspace(
        "save-config",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    std::fs::write(dir.join(".jscpd.json"), r#"{"minTokens": 500}"#).unwrap();
    lsp.notify(
        "textDocument/didSave",
        json!({"textDocument": {"uri": uri(&dir.join(".jscpd.json"))}}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_watched_change_to_an_open_file_leaves_its_buffer_alone() {
    let dir = workspace(
        "watched-open",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    let a = lsp.open(&dir.join("a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    // The disk changes under the open file: the buffer still has the copy.
    std::fs::write(dir.join("a.js"), "export const x = 1;\n").unwrap();
    lsp.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": a, "type": 2}]}),
    );
    let report = lsp.request("jscpd/clones", Value::Null);
    assert_eq!(
        report["projects"][0]["duplicates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

// ------------------------------------------------------------ settings

#[test]
fn the_editor_settings_reload_only_when_they_change() {
    let dir = workspace(
        "settings",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({"jscpd": {"minTokens": 20}}));
    let a = lsp.open(&dir.join("a.js"));
    lsp.diagnostics(&a, |d| !d.is_empty());
    // `null` asks the server to pull, and the same settings change nothing.
    for settings in [
        Value::Null,
        json!({"minTokens": 20}),
        json!({"jscpd": {"minTokens": 20}}),
    ] {
        lsp.notify(
            "workspace/didChangeConfiguration",
            json!({"settings": settings}),
        );
        let (before, _) = lsp.messages_before("jscpd/statistics", Value::Null);
        assert!(!reloaded(&before), "{settings}: {before:?}");
    }
    lsp.notify(
        "workspace/didChangeConfiguration",
        json!({"settings": {"jscpd": {"minTokens": 500}}}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn mistakes_in_the_settings_and_configs_are_shown() {
    let dir = workspace(
        "mistakes",
        &[
            ("broken/a.js", TOTAL),
            ("broken/.jscpd.json", "{\n  \"minTokens\": 20,\n}\n"),
            ("lsp/a.js", TOTAL),
            (
                "lsp/.jscpd.json",
                r#"{"lsp": {"clone": {"enabled": true}}}"#,
            ),
            ("secret/a.js", TOTAL),
            ("secret/b.js", TOTAL),
            (
                "secret/.jscpd.json",
                r#"{"minTokens": 20, "minLines": 3, "semantic": {"apiKey": "sk-123"}}"#,
            ),
        ],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({"lsp": {"bogus": {}}}));
    let (before, response) = lsp.messages_before("jscpd/statistics", Value::Null);
    let shown = shown(&before);
    let has = |typ: i64, part: &str| shown.iter().any(|(t, m)| *t == typ && m.contains(part));
    assert!(has(2, "the lsp settings of the editor"), "{shown:?}");
    assert!(has(2, "using the defaults"), "{shown:?}");
    assert!(has(2, "lsp/.jscpd.json"), "{shown:?}");
    assert!(has(1, "semantic.apiKey"), "{shown:?}");
    assert!(
        !shown.iter().any(|(_, m)| m.contains("sk-123")),
        "the key itself is never shown: {shown:?}"
    );
    // The project with the key does not run; the others do.
    let projects = response.response_result.unwrap()["projects"].clone();
    let roots: Vec<String> = projects
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["roots"][0].as_str().unwrap().to_string())
        .collect();
    assert!(!roots.iter().any(|r| r.ends_with("secret")), "{roots:?}");
    assert!(roots.iter().any(|r| r.ends_with("broken")), "{roots:?}");
    lsp.open(&dir.join("secret/b.js"));
    let report = lsp.request("jscpd/clones", Value::Null);
    assert!(
        !report.to_string().contains("secret"),
        "nothing from the refused project: {report}"
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn workspace_folders_added_and_removed_change_what_is_scanned() {
    let dir = workspace("folders", &[("one/a.js", TOTAL), ("two/b.js", TOTAL)]);
    let (one, two) = (dir.join("one"), dir.join("two"));
    let mut lsp = Lsp::start(&dir, &["--min-tokens", "20", "--min-lines", "3"]);
    lsp.initialize(&[&one], json!({}));
    let a = lsp.open(&one.join("a.js"));
    let report = lsp.request("jscpd/clones", Value::Null);
    assert_eq!(report["projects"][0]["duplicates"], json!([]));
    let folder = json!({"uri": uri(&two), "name": "two"});
    lsp.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [folder], "removed": []}}),
    );
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    assert!(diagnostics[0]["message"].as_str().unwrap().contains("b.js"));
    lsp.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({"event": {"added": [], "removed": [folder]}}),
    );
    lsp.diagnostics(&a, |d| d.is_empty());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

// ------------------------------------------------------------ lifecycle

#[test]
fn after_shutdown_notifications_are_ignored() {
    let dir = workspace(
        "after-shutdown",
        &[("a.js", TOTAL), ("b.js", TOTAL), (".jscpd.json", SMALL)],
    );
    let mut lsp = Lsp::start(&dir, &[]);
    lsp.initialize(&[&dir], json!({}));
    lsp.request("shutdown", Value::Null);
    let a = lsp.open(&dir.join("a.js"));
    // The open is not handled: no diagnostics come for it. The answer to
    // a request after it shows the server has read it.
    let (before, response) = lsp.messages_before("jscpd/clones", Value::Null);
    assert!(response.response_result.is_err());
    assert!(
        notifications(&before, "textDocument/publishDiagnostics").is_empty(),
        "{a}: {before:?}"
    );
    assert_eq!(lsp.exit_code(), Some(0));
    remove(&dir);
}

#[test]
fn a_client_that_goes_away_ends_the_server() {
    let dir = workspace("gone", &[("a.js", TOTAL)]);
    for shutdown in [true, false] {
        let mut lsp = Lsp::start(&dir, &[]);
        lsp.initialize(&[&dir], json!({}));
        if shutdown {
            lsp.request("shutdown", Value::Null);
        }
        let Lsp {
            mut child, stdin, ..
        } = lsp;
        drop(stdin);
        let code = child.wait().unwrap().code();
        assert_eq!(
            code,
            Some(if shutdown { 0 } else { 1 }),
            "shutdown: {shutdown}"
        );
    }
    remove(&dir);
}

// ------------------------------------------------------------ semantic model

/// A workspace for the semantic analysis with the local model, which is
/// not in the (empty) cache.
fn semantic_workspace(name: &str) -> PathBuf {
    let dir = workspace(name, &[("a.js", TOTAL), (".jscpd.json", SMALL)]);
    let _ = std::fs::remove_dir_all(dir.with_extension("cache"));
    dir
}

/// Start a server for `dir` with semantic clones on, and wait for its
/// question about the model.
fn ask_about_the_model(dir: &Path, capabilities: Value) -> (Lsp, Request) {
    let mut lsp = Lsp::start(dir, &[]);
    lsp.initialize_with(
        &[dir],
        json!({"lsp": {"semantic": {"enabled": true}}}),
        capabilities,
    );
    let ask =
        lsp.wait(|m| matches!(m, Message::Request(r) if r.method == "window/showMessageRequest"));
    match ask {
        Message::Request(request) => (lsp, request),
        _ => unreachable!(),
    }
}

#[test]
fn a_missing_model_is_asked_about_once_and_not_now_is_respected() {
    let dir = semantic_workspace("model-not-now");
    let (mut lsp, ask) = ask_about_the_model(&dir, json!({}));
    let message = ask.params["message"].as_str().unwrap();
    assert!(message.contains("Download it now?"), "{message}");
    let titles: Vec<&str> = ask.params["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Download", "Not now"]);
    lsp.send(Message::Response(Response::new_ok(
        ask.id,
        json!({"title": "Not now"}),
    )));
    // A save starts the analysis again: it does not ask a second time, and
    // nothing is downloaded.
    let a = uri(&dir.join("a.js"));
    lsp.notify("textDocument/didSave", json!({"textDocument": {"uri": a}}));
    let (before, _) = lsp.messages_before("jscpd/statistics", Value::Null);
    assert!(
        requests(&before, "window/showMessageRequest").is_empty(),
        "{before:?}"
    );
    assert!(shown(&before).is_empty(), "{before:?}");
    assert!(!dir.with_extension("cache").join("models").exists());
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn a_download_that_fails_is_shown_with_its_progress_ended() {
    let dir = semantic_workspace("model-download");
    let (mut lsp, ask) = ask_about_the_model(&dir, json!({"window": {"workDoneProgress": true}}));
    lsp.send(Message::Response(Response::new_ok(
        ask.id,
        json!({"title": "Download"}),
    )));
    // No network here: the download fails, and says so.
    let mut seen = Vec::new();
    let failed = lsp.wait_seeing(
        |m| matches!(m, Message::Notification(n) if n.method == "window/showMessage"),
        |m| seen.push(m.clone()),
    );
    let Message::Notification(failed) = failed else {
        unreachable!()
    };
    assert_eq!(failed.params["type"], 1, "an error");
    assert!(
        failed.params["message"]
            .as_str()
            .unwrap()
            .starts_with("jscpd: downloading the model:"),
        "{}",
        failed.params
    );
    let begun = notifications(&seen, "$/progress")
        .into_iter()
        .find(|p| p["value"]["message"] == "Downloading the embedding model")
        .expect("a progress for the download")
        .clone();
    let ended = notifications(&seen, "$/progress")
        .into_iter()
        .any(|p| p["token"] == begun["token"] && p["value"]["kind"] == "end");
    assert!(ended, "{seen:?}");
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}

#[test]
fn an_embeddings_api_that_is_not_there_is_shown() {
    let dir = workspace("semantic-down", &[]);
    common::embeddings::cart_project(&dir);
    let mut lsp = Lsp::start(
        &dir,
        &[
            "--semantic-url",
            "http://127.0.0.1:9/v1/embeddings",
            "--semantic-model",
            "stand-in",
            "--min-tokens",
            "15",
            "--min-lines",
            "3",
        ],
    );
    lsp.initialize(&[&dir], json!({"lsp": {"semantic": {"enabled": true}}}));
    let failed =
        lsp.wait(|m| matches!(m, Message::Notification(n) if n.method == "window/showMessage"));
    let Message::Notification(failed) = failed else {
        unreachable!()
    };
    assert_eq!(failed.params["type"], 1, "an error");
    assert!(
        failed.params["message"]
            .as_str()
            .unwrap()
            .starts_with("jscpd: semantic clones:"),
        "{}",
        failed.params
    );
    assert_eq!(lsp.shutdown(), 0);
    remove(&dir);
}
