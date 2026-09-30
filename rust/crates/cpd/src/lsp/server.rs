//! The server: the protocol loop, the open documents and the projects.
//!
//! State lives on the loop's thread. An edit marks its file and, once the
//! editor has been quiet for [`DEBOUNCE`], the file is tokenized again from
//! its buffer, its pools are searched again, and the open files get their
//! diagnostics again. Opening a file or saving one does not wait.

use super::findings::{Finding, Scope, Target, Text, clone_findings, comment_syntax, diagnostic};
use super::index::ScanIndex;
use super::position::{Encoding, path_to_uri, uri_to_path};
use super::project::{CONFIG_NAME, Project, find_config_dirs, plan};
use super::settings::Analysis;
use crate::cli::Cli;
use crossbeam_channel::RecvTimeoutError;
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::Notification as _;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Command, Diagnostic,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWatchedFilesRegistrationOptions, DidChangeWorkspaceFoldersParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, ExecuteCommandParams, FileSystemWatcher,
    GlobPattern, Hover, HoverContents, HoverParams, InitializeParams, MarkupContent, MarkupKind,
    MessageType, Position, ProgressParams, ProgressParamsValue, ProgressToken,
    PublishDiagnosticsParams, Range, Registration, RegistrationParams, ShowDocumentParams,
    ShowMessageParams, TextEdit, Uri, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressCreateParams, WorkDoneProgressEnd, WorkspaceEdit,
};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How long the editor must be quiet after an edit before its file is
/// searched again.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The command behind "Go to the other copy": `[uri, range]`.
pub const SHOW_LOCATION: &str = "jscpd.showLocation";

/// What the client can do, from `initialize`.
#[derive(Default)]
struct ClientCaps {
    progress: bool,
    related_information: bool,
    show_document: bool,
    watch_files: bool,
}

/// A file open in the editor.
struct Document {
    uri: Uri,
    version: i32,
    text: Rc<Text>,
}

pub struct Server {
    sender: crossbeam_channel::Sender<Message>,
    cli: Cli,
    defaults: Vec<Analysis>,
    encoding: Encoding,
    caps: ClientCaps,
    folders: Vec<PathBuf>,
    /// The editor's settings: the keys of `.jscpd.json`, merged over each
    /// project's config.
    settings: Value,
    projects: Vec<Project>,
    documents: HashMap<PathBuf, Document>,
    /// Files changed in the editor and waiting for the quiet period.
    pending: BTreeSet<PathBuf>,
    deadline: Option<Instant>,
    /// The findings last published per file, for code actions and hovers.
    findings: HashMap<PathBuf, Vec<Finding>>,
    /// Files with diagnostics shown, to clear the ones that go away.
    published: HashSet<PathBuf>,
    pool: rayon::ThreadPool,
    next_id: i32,
}

/// Serve on `connection` until the client shuts the server down.
pub fn run(connection: Connection, cli: &Cli, defaults: Vec<Analysis>) -> Result<(), String> {
    let (id, params) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: InitializeParams = serde_json::from_value(params).map_err(|e| e.to_string())?;
    let encoding = match params
        .capabilities
        .general
        .as_ref()
        .and_then(|g| g.position_encodings.as_ref())
        .is_some_and(|encodings| encodings.contains(&lsp_types::PositionEncodingKind::UTF8))
    {
        true => Encoding::Utf8,
        false => Encoding::Utf16,
    };
    connection
        .initialize_finish(id, capabilities(encoding))
        .map_err(|e| e.to_string())?;

    let mut server = Server::new(&connection, cli, defaults, encoding, &params);
    // `initialize_finish` has waited for the client's `initialized`, so the
    // server may ask it for things now.
    server.register_watchers();
    server.load();
    main_loop(&connection, &mut server)
}

fn capabilities(encoding: Encoding) -> Value {
    json!({
        "capabilities": {
            "positionEncoding": match encoding { Encoding::Utf8 => "utf-8", Encoding::Utf16 => "utf-16" },
            "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": false } },
            "hoverProvider": true,
            "codeActionProvider": true,
            "executeCommandProvider": { "commands": [SHOW_LOCATION] },
            "workspace": { "workspaceFolders": { "supported": true, "changeNotifications": true } },
        },
        "serverInfo": { "name": "jscpd", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn main_loop(connection: &Connection, server: &mut Server) -> Result<(), String> {
    loop {
        let message = match server.deadline {
            Some(deadline) => match connection.receiver.recv_deadline(deadline) {
                Ok(message) => Some(message),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            },
            None => match connection.receiver.recv() {
                Ok(message) => Some(message),
                Err(_) => return Ok(()),
            },
        };
        match message {
            None => server.flush(),
            Some(Message::Request(request)) => {
                if connection
                    .handle_shutdown(&request)
                    .map_err(|e| e.to_string())?
                {
                    return Ok(());
                }
                server.request(request);
            }
            Some(Message::Notification(notification)) => {
                if notification.method == lsp_types::notification::Exit::METHOD {
                    return Ok(());
                }
                server.notification(notification);
            }
            Some(Message::Response(_)) => {}
        }
    }
}

impl Server {
    fn new(
        connection: &Connection,
        cli: &Cli,
        defaults: Vec<Analysis>,
        encoding: Encoding,
        params: &InitializeParams,
    ) -> Self {
        let caps = &params.capabilities;
        let caps = ClientCaps {
            progress: caps
                .window
                .as_ref()
                .and_then(|w| w.work_done_progress)
                .unwrap_or(false),
            related_information: caps
                .text_document
                .as_ref()
                .and_then(|t| t.publish_diagnostics.as_ref())
                .and_then(|p| p.related_information)
                .unwrap_or(false),
            show_document: caps
                .window
                .as_ref()
                .and_then(|w| w.show_document.as_ref())
                .is_some_and(|s| s.support),
            watch_files: caps
                .workspace
                .as_ref()
                .and_then(|w| w.did_change_watched_files.as_ref())
                .and_then(|d| d.dynamic_registration)
                .unwrap_or(false),
        };
        #[allow(deprecated)]
        let folders: Vec<PathBuf> = match &params.workspace_folders {
            Some(folders) if !folders.is_empty() => {
                folders.iter().filter_map(|f| uri_to_path(&f.uri)).collect()
            }
            _ => params
                .root_uri
                .as_ref()
                .and_then(uri_to_path)
                .into_iter()
                .collect(),
        };
        let folders = folders
            .into_iter()
            .map(|f| std::fs::canonicalize(&f).unwrap_or(f))
            .collect();
        Self {
            sender: connection.sender.clone(),
            cli: cli.clone(),
            defaults,
            encoding,
            caps,
            folders,
            settings: settings_of(params.initialization_options.as_ref()),
            projects: Vec::new(),
            documents: HashMap::new(),
            pending: BTreeSet::new(),
            deadline: None,
            findings: HashMap::new(),
            published: HashSet::new(),
            pool: cpd_finder::orchestrate::build_thread_pool(cli.workers),
            next_id: 0,
        }
    }

    // ------------------------------------------------------------ projects

    /// Find the projects of the workspace and scan them, then publish.
    fn load(&mut self) {
        let token = self.progress_begin("jscpd", "Scanning the workspace");
        let config_dirs = find_config_dirs(&self.folders);
        self.projects = plan(&self.folders, &config_dirs)
            .into_iter()
            .map(|plan| Project::new(plan, &self.cli, &self.defaults, &self.settings))
            .collect();
        for project in &mut self.projects {
            for diagnostic in &project.diagnostics {
                let _ = self
                    .sender
                    .send(notify::<lsp_types::notification::LogMessage>(
                        lsp_types::LogMessageParams {
                            typ: MessageType::WARNING,
                            message: diagnostic.to_string(),
                        },
                    ));
            }
            if let Some(reason) = &project.refused {
                let _ = self
                    .sender
                    .send(notify::<lsp_types::notification::ShowMessage>(
                        ShowMessageParams {
                            typ: MessageType::ERROR,
                            message: format!("jscpd: {reason}"),
                        },
                    ));
                continue;
            }
            project.index = Some(ScanIndex::build(&self.pool, project.run.clone()));
        }
        // Files open before the scan hold text the disk may not have yet.
        let open: Vec<(PathBuf, Rc<Text>)> = self
            .documents
            .iter()
            .map(|(path, doc)| (path.clone(), doc.text.clone()))
            .collect();
        for (path, text) in open {
            self.update(&path, Some(&text.text));
        }
        self.progress_end(token);
        self.publish_all();
    }

    fn project_of(&self, path: &Path) -> Option<usize> {
        self.projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.owns(path))
            .max_by_key(|(_, p)| p.depth())
            .map(|(i, _)| i)
    }

    /// Tokenize the file at `path` again, from `text` or the disk.
    fn update(&mut self, path: &Path, text: Option<&str>) {
        let Some(project) = self.project_of(path) else {
            return;
        };
        if let Some(index) = &mut self.projects[project].index {
            index.update(&self.pool, &path.to_string_lossy(), text);
        }
    }

    // ------------------------------------------------------------ publishing

    fn text_of(&self, path: &Path) -> Option<Rc<Text>> {
        match self.documents.get(path) {
            Some(doc) => Some(doc.text.clone()),
            None => std::fs::read_to_string(path)
                .ok()
                .map(|t| Rc::new(Text::new(t))),
        }
    }

    /// The findings of the file at `path`.
    fn findings_of(&self, path: &Path) -> Vec<Finding> {
        let Some(project) = self.project_of(path).map(|i| &self.projects[i]) else {
            return Vec::new();
        };
        let Some(index) = &project.index else {
            return Vec::new();
        };
        let Some(text) = self.text_of(path) else {
            return Vec::new();
        };
        let mut cache: HashMap<PathBuf, Option<Rc<Text>>> = HashMap::new();
        let mut other_text = |other: &Path| {
            cache
                .entry(other.to_path_buf())
                .or_insert_with(|| self.text_of(other))
                .clone()
        };
        let scope = Scope {
            analyses: &project.analyses,
            roots: &project.options.paths,
            encoding: self.encoding,
        };
        clone_findings(path, &text, index, &scope, &mut other_text)
    }

    /// Publish the diagnostics of every open file and, for projects that
    /// ask, of every file with findings; clear the files that have none now.
    fn publish_all(&mut self) {
        let mut files: BTreeSet<PathBuf> = self.documents.keys().cloned().collect();
        for project in &self.projects {
            if !project.analyses.all_files {
                continue;
            }
            if let Some(index) = &project.index {
                for clone in index.clones() {
                    for fragment in [&clone.fragment_a, &clone.fragment_b] {
                        files.insert(PathBuf::from(super::index::host_file(
                            &fragment.source_id,
                            &clone.format,
                        )));
                    }
                }
            }
        }
        let stale: Vec<PathBuf> = self
            .published
            .iter()
            .filter(|path| !files.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            self.send_diagnostics(&path, Vec::new());
            self.findings.remove(&path);
        }
        for path in files {
            let findings = self.findings_of(&path);
            let diagnostics = findings
                .iter()
                .map(|f| diagnostic(f, self.caps.related_information))
                .collect();
            self.send_diagnostics(&path, diagnostics);
            self.findings.insert(path, findings);
        }
    }

    fn send_diagnostics(&mut self, path: &Path, diagnostics: Vec<Diagnostic>) {
        let (uri, version) = match self.documents.get(path) {
            Some(doc) => (doc.uri.clone(), Some(doc.version)),
            None => match path_to_uri(path) {
                Some(uri) => (uri, None),
                None => return,
            },
        };
        if diagnostics.is_empty() {
            if !self.published.remove(path) {
                return;
            }
        } else {
            self.published.insert(path.to_path_buf());
        }
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::PublishDiagnostics>(
                PublishDiagnosticsParams {
                    uri,
                    diagnostics,
                    version,
                },
            ));
    }

    /// The quiet period is over: search the changed files again.
    fn flush(&mut self) {
        self.deadline = None;
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            return;
        }
        for path in pending {
            let text = self.documents.get(&path).map(|d| d.text.clone());
            self.update(&path, text.as_deref().map(|t| t.text.as_str()));
        }
        self.publish_all();
    }

    // ------------------------------------------------------------ notifications

    fn notification(&mut self, notification: Notification) {
        use lsp_types::notification::*;
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidOpenTextDocumentParams>(notification.params)
                else {
                    return;
                };
                let Some(path) = document_path(&params.text_document.uri) else {
                    return;
                };
                let text = Rc::new(Text::new(params.text_document.text));
                self.documents.insert(
                    path.clone(),
                    Document {
                        uri: params.text_document.uri,
                        version: params.text_document.version,
                        text: text.clone(),
                    },
                );
                self.update(&path, Some(&text.text));
                self.publish_all();
            }
            DidChangeTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidChangeTextDocumentParams>(notification.params)
                else {
                    return;
                };
                let Some(path) = document_path(&params.text_document.uri) else {
                    return;
                };
                // Full sync: the last change holds the whole text.
                let Some(change) = params.content_changes.into_iter().last() else {
                    return;
                };
                if let Some(doc) = self.documents.get_mut(&path) {
                    doc.version = params.text_document.version;
                    doc.text = Rc::new(Text::new(change.text));
                    self.pending.insert(path);
                    self.deadline = Some(Instant::now() + DEBOUNCE);
                }
            }
            DidSaveTextDocument::METHOD => {
                // The buffer is on disk now; anything waiting runs at once.
                if !self.pending.is_empty() {
                    self.flush();
                }
            }
            DidCloseTextDocument::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidCloseTextDocumentParams>(notification.params)
                else {
                    return;
                };
                let Some(path) = document_path(&params.text_document.uri) else {
                    return;
                };
                self.documents.remove(&path);
                self.pending.remove(&path);
                // Back to what the disk holds.
                self.update(&path, None);
                self.publish_all();
            }
            DidChangeWatchedFiles::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidChangeWatchedFilesParams>(notification.params)
                else {
                    return;
                };
                self.files_changed(params);
            }
            DidChangeConfiguration::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidChangeConfigurationParams>(notification.params)
                else {
                    return;
                };
                self.settings = settings_of(Some(&params.settings));
                self.load();
            }
            DidChangeWorkspaceFolders::METHOD => {
                let Ok(params) =
                    serde_json::from_value::<DidChangeWorkspaceFoldersParams>(notification.params)
                else {
                    return;
                };
                for removed in params.event.removed {
                    if let Some(path) = uri_to_path(&removed.uri) {
                        let path = std::fs::canonicalize(&path).unwrap_or(path);
                        self.folders.retain(|f| *f != path);
                    }
                }
                for added in params.event.added {
                    if let Some(path) = uri_to_path(&added.uri) {
                        self.folders
                            .push(std::fs::canonicalize(&path).unwrap_or(path));
                    }
                }
                self.load();
            }
            _ => {}
        }
    }

    fn files_changed(&mut self, params: DidChangeWatchedFilesParams) {
        let mut reload = false;
        for change in params.changes {
            let Some(path) = uri_to_path(&change.uri) else {
                continue;
            };
            if path.file_name().is_some_and(|name| name == CONFIG_NAME) {
                reload = true;
                continue;
            }
            // The editor's buffer wins over the disk while the file is open.
            if self.documents.contains_key(&path) {
                continue;
            }
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            if change.typ == lsp_types::FileChangeType::DELETED {
                if let Some(project) = self.project_of(&path)
                    && let Some(index) = &mut self.projects[project].index
                {
                    index.remove(&self.pool, &path.to_string_lossy());
                }
            } else {
                self.update(&path, None);
            }
        }
        match reload {
            true => self.load(),
            false => self.publish_all(),
        }
    }

    /// Ask the client to watch the files of the workspace, so a checkout or
    /// a change to `.jscpd.json` reaches the server.
    fn register_watchers(&mut self) {
        if !self.caps.watch_files {
            return;
        }
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/*".to_string()),
                kind: None,
            }],
        };
        let params = RegistrationParams {
            registrations: vec![Registration {
                id: "jscpd-watch".to_string(),
                method: lsp_types::notification::DidChangeWatchedFiles::METHOD.to_string(),
                register_options: serde_json::to_value(options).ok(),
            }],
        };
        self.send_request::<lsp_types::request::RegisterCapability>(params);
    }

    // ------------------------------------------------------------ requests

    fn request(&mut self, request: Request) {
        use lsp_types::request::*;
        let id = request.id.clone();
        let result = match request.method.as_str() {
            CodeActionRequest::METHOD => serde_json::from_value::<CodeActionParams>(request.params)
                .map(|p| serde_json::to_value(self.code_actions(p)).unwrap_or(Value::Null)),
            HoverRequest::METHOD => serde_json::from_value::<HoverParams>(request.params)
                .map(|p| serde_json::to_value(self.hover(p)).unwrap_or(Value::Null)),
            ExecuteCommand::METHOD => {
                serde_json::from_value::<ExecuteCommandParams>(request.params)
                    .map(|p| self.execute(p))
            }
            "jscpd/clones" => Ok(self.all_clones()),
            "jscpd/statistics" => Ok(self.statistics()),
            "jscpd/rescan" => {
                self.load();
                Ok(Value::Null)
            }
            _ => {
                self.respond(Response::new_err(
                    id,
                    ErrorCode::MethodNotFound as i32,
                    format!("unknown request {}", request.method),
                ));
                return;
            }
        };
        let response = match result {
            Ok(value) => Response::new_ok(id, value),
            Err(error) => Response::new_err(id, ErrorCode::InvalidParams as i32, error.to_string()),
        };
        self.respond(response);
    }

    fn respond(&self, response: Response) {
        let _ = self.sender.send(Message::Response(response));
    }

    /// The findings of a document that touch `range`.
    fn findings_at(&self, uri: &Uri, range: Range) -> Vec<&Finding> {
        let Some(path) = document_path(uri) else {
            return Vec::new();
        };
        self.findings
            .get(&path)
            .map(|findings| {
                findings
                    .iter()
                    .filter(|f| overlaps(f.range, range))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn code_actions(&self, params: CodeActionParams) -> Vec<CodeActionOrCommand> {
        let mut actions = Vec::new();
        let uri = &params.text_document.uri;
        for finding in self.findings_at(uri, params.range) {
            for target in &finding.targets {
                let title = match finding.analysis {
                    Analysis::Ast => format!("Go to the similar function in {}", target.label),
                    _ => format!("Go to the other copy in {}", target.label),
                };
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: title.clone(),
                    command: Some(Command::new(
                        title,
                        SHOW_LOCATION.to_string(),
                        Some(vec![json!(target.uri), json!(target.range)]),
                    )),
                    ..CodeAction::default()
                }));
            }
            if finding.analysis == Analysis::Clones
                && let Some(edit) = self.ignore_edit(uri, finding)
            {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: "Ignore this clone (jscpd:ignore-start … jscpd:ignore-end)".to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    edit: Some(edit),
                    ..CodeAction::default()
                }));
            }
        }
        actions
    }

    /// Wrap a finding's lines in `jscpd:ignore-start` and `jscpd:ignore-end`
    /// comments, indented like its first line.
    fn ignore_edit(&self, uri: &Uri, finding: &Finding) -> Option<WorkspaceEdit> {
        let (open, close) = comment_syntax(&finding.format)?;
        let path = document_path(uri)?;
        let text = self.text_of(&path)?;
        let first = text.index.line(&text.text, finding.first_line as usize);
        let indent: String = first.chars().take_while(|c| c.is_whitespace()).collect();
        let after = finding.last_line + 1;
        let end = match (after as usize) < text.index.line_count() {
            true => Position::new(after, 0),
            false => Position::new(
                finding.last_line,
                text.index
                    .line(&text.text, finding.last_line as usize)
                    .encode_utf16()
                    .count() as u32,
            ),
        };
        let trailing = match (after as usize) < text.index.line_count() {
            true => "\n",
            false => "",
        };
        let leading = match trailing.is_empty() {
            true => "\n",
            false => "",
        };
        let edits = vec![
            TextEdit::new(
                Range::new(
                    Position::new(finding.first_line, 0),
                    Position::new(finding.first_line, 0),
                ),
                format!("{indent}{open}jscpd:ignore-start{close}\n"),
            ),
            TextEdit::new(
                Range::new(end, end),
                format!("{leading}{indent}{open}jscpd:ignore-end{close}{trailing}"),
            ),
        ];
        #[allow(clippy::mutable_key_type)]
        let changes = HashMap::from([(uri.clone(), edits)]);
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        })
    }

    fn hover(&self, params: HoverParams) -> Option<Hover> {
        let position = params.text_document_position_params.position;
        let uri = &params.text_document_position_params.text_document.uri;
        let findings = self.findings_at(uri, Range::new(position, position));
        if findings.is_empty() {
            return None;
        }
        let mut sections = Vec::new();
        for finding in findings {
            let mut section = format!("**{}** `{}`", finding.message, finding.rule);
            if let Some(target) = finding.targets.first()
                && let Some(preview) = self.preview(target, &finding.format)
            {
                section.push_str("\n\n");
                section.push_str(&preview);
            }
            sections.push(section);
        }
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: sections.join("\n\n---\n\n"),
            }),
            range: None,
        })
    }

    /// The first lines of another copy, in a code fence.
    fn preview(&self, target: &Target, format: &str) -> Option<String> {
        let text = self.text_of(&target.path)?;
        let first = target.range.start.line as usize;
        let last = (target.range.end.line as usize).min(first + 7);
        let lines: Vec<&str> = (first..=last)
            .map(|line| text.index.line(&text.text, line))
            .collect();
        Some(format!("```{format}\n{}\n```", lines.join("\n")))
    }

    fn execute(&mut self, params: ExecuteCommandParams) -> Value {
        if params.command != SHOW_LOCATION {
            return Value::Null;
        }
        let mut arguments = params.arguments.into_iter();
        let (Some(uri), Some(range)) = (arguments.next(), arguments.next()) else {
            return Value::Null;
        };
        let (Ok(uri), Ok(range)) = (
            serde_json::from_value::<Uri>(uri),
            serde_json::from_value::<Range>(range),
        ) else {
            return Value::Null;
        };
        if self.caps.show_document {
            self.send_request::<lsp_types::request::ShowDocument>(ShowDocumentParams {
                uri,
                external: Some(false),
                take_focus: Some(true),
                selection: Some(range),
            });
        }
        Value::Null
    }

    fn all_clones(&self) -> Value {
        let clones: Vec<&cpd_core::models::CpdClone> = self
            .projects
            .iter()
            .filter_map(|p| p.index.as_ref())
            .flat_map(|index| index.clones())
            .collect();
        json!({ "clones": clones })
    }

    fn statistics(&self) -> Value {
        let projects: Vec<Value> = self
            .projects
            .iter()
            .filter_map(|p| {
                let index = p.index.as_ref()?;
                Some(json!({
                    "roots": p.options.paths,
                    "config": p.plan.config_dir.as_ref().map(|d| d.join(CONFIG_NAME)),
                    "files": index.file_count(),
                    "statistics": index.statistics(),
                }))
            })
            .collect();
        json!({ "projects": projects })
    }

    // ------------------------------------------------------------ progress

    fn send_request<R: lsp_types::request::Request>(&mut self, params: R::Params) {
        self.next_id += 1;
        let request = Request::new(
            RequestId::from(format!("jscpd-{}", self.next_id)),
            R::METHOD.to_string(),
            params,
        );
        let _ = self.sender.send(Message::Request(request));
    }

    fn progress_begin(&mut self, title: &str, message: &str) -> Option<ProgressToken> {
        if !self.caps.progress {
            return None;
        }
        self.next_id += 1;
        let token = ProgressToken::String(format!("jscpd-progress-{}", self.next_id));
        self.send_request::<lsp_types::request::WorkDoneProgressCreate>(
            WorkDoneProgressCreateParams {
                token: token.clone(),
            },
        );
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::Progress>(
                ProgressParams {
                    token: token.clone(),
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                        WorkDoneProgressBegin {
                            title: title.to_string(),
                            message: Some(message.to_string()),
                            ..WorkDoneProgressBegin::default()
                        },
                    )),
                },
            ));
        Some(token)
    }

    fn progress_end(&mut self, token: Option<ProgressToken>) {
        let Some(token) = token else { return };
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::Progress>(
                ProgressParams {
                    token,
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                        WorkDoneProgressEnd { message: None },
                    )),
                },
            ));
    }
}

fn notify<N: lsp_types::notification::Notification>(params: N::Params) -> Message {
    Message::Notification(Notification::new(N::METHOD.to_string(), params))
}

/// The path of a document, canonical like the paths a scan gives its files.
fn document_path(uri: &Uri) -> Option<PathBuf> {
    let path = uri_to_path(uri)?;
    Some(std::fs::canonicalize(&path).unwrap_or(path))
}

/// The editor's settings: the object itself, or its `jscpd` key when the
/// editor sends its settings by section.
fn settings_of(value: Option<&Value>) -> Value {
    match value {
        Some(Value::Object(map)) => match map.get("jscpd") {
            Some(section @ Value::Object(_)) => section.clone(),
            _ => Value::Object(map.clone()),
        },
        _ => json!({}),
    }
}

fn overlaps(a: Range, b: Range) -> bool {
    let before = |x: Position, y: Position| (x.line, x.character) < (y.line, y.character);
    !before(a.end, b.start) && !before(b.end, a.start)
}
