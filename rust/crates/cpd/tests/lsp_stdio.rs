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
        let mut child = Command::new(cpd_bin())
            .arg("--lsp")
            .args(args)
            .current_dir(dir)
            // Vectors of the semantic analysis go beside the workspace.
            .env("JSCPD_CACHE_DIR", dir.with_extension("cache"))
            .env_remove("JSCPD_SEMANTIC_API_KEY")
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
    /// on the way (progress, registrations, showDocument).
    fn wait(&mut self, accept: impl Fn(&Message) -> bool) -> Message {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self
                .messages
                .recv_timeout(left)
                .expect("the server answered in time");
            if let Message::Request(request) = &message {
                let answer = Response::new_ok(request.id.clone(), Value::Null);
                self.send(Message::Response(answer));
            }
            if accept(&message) {
                return message;
            }
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
        let folders: Vec<Value> = folders
            .iter()
            .map(|f| json!({"uri": uri(f), "name": "ws"}))
            .collect();
        self.request(
            "initialize",
            json!({
                "processId": null,
                "workspaceFolders": folders,
                "initializationOptions": options,
                "capabilities": {
                    "textDocument": {"publishDiagnostics": {"relatedInformation": true}},
                    "window": {"showDocument": {"support": true}},
                },
            }),
        );
        self.notify("initialized", json!({}));
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

#[test]
fn config_files_split_the_workspace_into_projects() {
    let dir = workspace("projects", &[("one/a.js", TOTAL), ("two/b.js", TOTAL)]);
    let (one, two) = (dir.join("one"), dir.join("two"));
    let mut lsp = Lsp::start(&dir, &["--min-tokens", "20", "--min-lines", "3"]);
    lsp.initialize(&[&one, &two], json!({}));
    let a = lsp.open(&one.join("a.js"));
    // No config: both folders are one project, and the copy across them
    // is a clone.
    let diagnostics = lsp.diagnostics(&a, |d| !d.is_empty());
    assert!(diagnostics[0]["message"].as_str().unwrap().contains("b.js"));
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
