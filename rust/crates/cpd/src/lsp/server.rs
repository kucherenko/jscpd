//! The server: the protocol loop, the open documents and the projects.
//!
//! State lives on the loop's thread. An edit marks its file and, once the
//! editor has been quiet for [`DEBOUNCE`], the file is tokenized again from
//! its buffer, its pools are searched again, and the open files get their
//! diagnostics again. Opening a file or saving one does not wait.

use super::complexity::{Limits, complexity_findings};
use super::dead_code::dead_code_findings;
use super::findings::{
    Finding, Scope, Snapshot, Target, Text, block_comment_syntax, clone_findings, comment_syntax,
    diagnostic,
};
use super::position::{Encoding, path_to_uri, uri_to_path};
use super::project::{CONFIG_NAME, Project, find_config_dirs, plan};
use super::settings::{Analysis, LspSection};
use crate::cli::{Cli, ConfigDiagnostic};
use crate::index::{ScanIndex, host_file};
use crossbeam_channel::{Receiver, Sender};
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
/// Files that change what the dead-code analysis counts as used without
/// being in the index.
const MANIFESTS: [&str; 4] = [
    "package.json",
    "tsconfig.json",
    "jsconfig.json",
    "pyproject.toml",
];
/// The command behind "Go to the other copy": `[uri, range]`.
pub const SHOW_LOCATION: &str = "jscpd.showLocation";

/// What the client can do, from `initialize`.
#[derive(Default)]
struct ClientCaps {
    progress: bool,
    related_information: bool,
    show_document: bool,
    watch_files: bool,
    /// Edits that name the version of the document they apply to.
    document_changes: bool,
}

/// A background job that finished: its project, the load it belongs to
/// (a reload makes older results stale), and what it found.
pub enum JobDone {
    DeadCode {
        project: usize,
        generation: u64,
        run: super::dead_code::Run,
    },
    Semantic {
        project: usize,
        generation: u64,
        clones: Result<SemanticRun, String>,
    },
    /// The model of the semantic analysis finished downloading.
    Downloaded(Result<(), String>),
}

/// What a semantic run found, and the files as it read them.
pub struct SemanticRun {
    clones: Vec<cpd_core::models::CpdClone>,
    snapshots: HashMap<PathBuf, Snapshot>,
}

/// A request the server sent and waits to hear back about.
enum Prompt {
    /// "Download the model?" for the semantic analysis.
    Download(cpd_semantic::SemanticOptions),
}

/// The clones of one project by the files they touch, built once per
/// publish rather than searched for every file.
struct Groups<'p> {
    index: HashMap<&'p str, Vec<&'p cpd_core::models::CpdClone>>,
    semantic: HashMap<&'p str, Vec<&'p cpd_core::models::CpdClone>>,
}

impl<'p> Groups<'p> {
    fn of(project: &'p Project) -> Self {
        fn group<'c>(
            clones: impl Iterator<Item = &'c cpd_core::models::CpdClone>,
        ) -> HashMap<&'c str, Vec<&'c cpd_core::models::CpdClone>> {
            let mut files: HashMap<&str, Vec<&cpd_core::models::CpdClone>> = HashMap::new();
            for clone in clones {
                let a = host_file(&clone.fragment_a.source_id);
                let b = host_file(&clone.fragment_b.source_id);
                files.entry(a).or_default().push(clone);
                if b != a {
                    files.entry(b).or_default().push(clone);
                }
            }
            files
        }
        Self {
            index: group(project.index.iter().flat_map(|index| index.clones())),
            semantic: group(project.semantic.iter()),
        }
    }
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
    /// Files with diagnostics shown, by the URI they were shown at, to clear
    /// the ones that go away at the same URI.
    published: HashMap<PathBuf, Uri>,
    pool: rayon::ThreadPool,
    next_id: i32,
    /// Where background jobs report back.
    jobs: Sender<JobDone>,
    /// Counts the loads of the workspace; a job's result from an older load
    /// is dropped.
    generation: u64,
    /// Projects with a dead-code run in flight, with the progress it shows,
    /// and those that changed again meanwhile and need one more.
    dead_code_running: HashMap<usize, Option<ProgressToken>>,
    dead_code_again: HashSet<usize>,
    /// The same for semantic runs.
    semantic_running: HashMap<usize, Option<ProgressToken>>,
    semantic_again: HashSet<usize>,
    /// Requests the server sent, by id.
    prompts: HashMap<RequestId, Prompt>,
    /// Whether the user has been asked about the model, or it is on its way.
    model_asked: bool,
    downloading: Option<Option<ProgressToken>>,
}

/// Serve on `connection` until the client shuts the server down, and
/// return the exit code: 0 after `shutdown` and `exit`, 1 for an `exit`
/// without `shutdown` or a client that went away.
pub fn run(connection: Connection, cli: &Cli, defaults: Vec<Analysis>) -> Result<i32, String> {
    let (id, params) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params = match initialize_params(params) {
        Ok(params) => params,
        Err(error) => {
            let response = Response::new_err(id, ErrorCode::InvalidParams as i32, error.clone());
            let _ = connection.sender.send(Message::Response(response));
            // Give the writer a moment to send the answer before the exit.
            std::thread::sleep(Duration::from_millis(100));
            return Err(error);
        }
    };
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

    let (jobs, done) = crossbeam_channel::unbounded();
    let mut server = Server::new(&connection, cli, defaults, encoding, &params, jobs);
    // `initialize_finish` has waited for the client's `initialized`, so the
    // server may ask it for things now.
    server.register_watchers();
    server.load();
    main_loop(&connection, &done, &mut server)
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

/// The params of `initialize`. A client may send fields in forms this
/// server does not read and lsp-types does not know, such as a `processId`
/// of -1 or a `trace` of `compact`; those fields are left out rather than
/// the whole request refused.
fn initialize_params(mut params: Value) -> Result<InitializeParams, String> {
    match serde_json::from_value::<InitializeParams>(params.clone()) {
        Ok(params) => Ok(params),
        Err(first) => {
            if let Some(object) = params.as_object_mut() {
                for unread in [
                    "processId",
                    "trace",
                    "clientInfo",
                    "locale",
                    "workDoneToken",
                ] {
                    object.remove(unread);
                }
            }
            serde_json::from_value(params).map_err(|_| format!("initialize: {first}"))
        }
    }
}

fn main_loop(
    connection: &Connection,
    done: &Receiver<JobDone>,
    server: &mut Server,
) -> Result<i32, String> {
    // After `shutdown`, only `exit` means anything (LSP spec).
    let mut shut_down = false;
    loop {
        // No edit waiting: wait for a message or a job as long as it takes.
        let wait = server.deadline.map_or(Duration::from_secs(3600), |d| {
            d.saturating_duration_since(Instant::now())
        });
        let message = crossbeam_channel::select! {
            recv(connection.receiver) -> message => match message {
                Ok(message) => Some(message),
                // The client went away without `exit`.
                Err(_) => return Ok(if shut_down { 0 } else { 1 }),
            },
            recv(done) -> job => {
                if let Ok(job) = job
                    && !shut_down
                {
                    server.job_done(job);
                }
                continue;
            },
            default(wait) => None,
        };
        match message {
            None => {
                if !shut_down && server.deadline.is_some_and(|d| d <= Instant::now()) {
                    server.flush();
                }
            }
            Some(Message::Request(request)) if shut_down => {
                server.respond(Response::new_err(
                    request.id,
                    ErrorCode::InvalidRequest as i32,
                    "the server is shutting down".to_string(),
                ));
            }
            Some(Message::Request(request)) if request.method == "shutdown" => {
                shut_down = true;
                server.respond(Response::new_ok(request.id, ()));
            }
            Some(Message::Request(request)) => server.request(request),
            Some(Message::Notification(notification))
                if notification.method == lsp_types::notification::Exit::METHOD =>
            {
                return Ok(if shut_down { 0 } else { 1 });
            }
            Some(Message::Notification(_)) if shut_down => {}
            Some(Message::Notification(notification)) => server.notification(notification),
            Some(Message::Response(response)) => server.response(response),
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
        jobs: Sender<JobDone>,
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
            document_changes: caps
                .workspace
                .as_ref()
                .and_then(|w| w.workspace_edit.as_ref())
                .and_then(|e| e.document_changes)
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
            published: HashMap::new(),
            pool: cpd_finder::orchestrate::build_thread_pool(cli.workers),
            next_id: 0,
            jobs,
            generation: 0,
            dead_code_running: HashMap::new(),
            dead_code_again: HashSet::new(),
            semantic_running: HashMap::new(),
            semantic_again: HashSet::new(),
            prompts: HashMap::new(),
            model_asked: false,
            downloading: None,
        }
    }

    // ------------------------------------------------------------ projects

    /// Find the projects of the workspace and scan them, then publish.
    fn load(&mut self) {
        self.generation += 1;
        let running = std::mem::take(&mut self.dead_code_running)
            .into_values()
            .chain(std::mem::take(&mut self.semantic_running).into_values());
        for token in running.collect::<Vec<_>>() {
            self.progress_end(token, None);
        }
        self.dead_code_again.clear();
        self.semantic_again.clear();
        let token = self.progress_begin("jscpd", "Scanning the workspace");
        // A mistake in the editor's `lsp` settings leaves them out, not the
        // `lsp` section of each project's config they are merged over.
        let mut settings = self.settings.clone();
        if let Some(lsp) = settings.get("lsp")
            && let Err(error) = serde_json::from_value::<LspSection>(lsp.clone())
        {
            self.show(
                MessageType::WARNING,
                format!("jscpd: the lsp settings of the editor: {error}; left out"),
            );
            if let Some(settings) = settings.as_object_mut() {
                settings.remove("lsp");
            }
        }
        let config_dirs = find_config_dirs(&self.folders);
        self.projects = plan(&self.folders, &config_dirs)
            .into_iter()
            .map(|plan| Project::new(plan, &self.cli, &self.defaults, &settings))
            .collect();
        let mut notes = Vec::new();
        for project in &mut self.projects {
            for diagnostic in &project.diagnostics {
                // A config that does not parse is left out as a whole, and a
                // wrong `lsp` section changes what runs: the user has to see
                // those. The rest goes to the log.
                let shown = match diagnostic {
                    ConfigDiagnostic::ParseError { .. } => {
                        Some(format!("jscpd: {diagnostic}; using the defaults"))
                    }
                    ConfigDiagnostic::InvalidValue { field, .. } if field == "lsp" => {
                        Some(format!("jscpd: {diagnostic}"))
                    }
                    _ => None,
                };
                notes.push(match shown {
                    Some(message) => (MessageType::WARNING, message, true),
                    None => (MessageType::WARNING, diagnostic.to_string(), false),
                });
            }
            if let Some(reason) = &project.refused {
                notes.push((MessageType::ERROR, format!("jscpd: {reason}"), true));
                continue;
            }
            if project.analyses.has(Analysis::DeadCode) {
                let (config, problems) = super::dead_code::config_of(project);
                project.dead_code_config = config;
                for problem in problems {
                    let typ = match problem.starts_with("Error") {
                        true => MessageType::ERROR,
                        false => MessageType::WARNING,
                    };
                    notes.push((typ, format!("jscpd: dead code: {problem}"), true));
                }
            }
            project.index = Some(ScanIndex::build(
                &self.pool,
                project.run.clone(),
                project.plan.excluded.clone(),
            ));
        }
        for (typ, message, shown) in notes {
            match shown {
                true => self.show(typ, message),
                false => self.log(typ, message),
            }
        }
        // Files open before the scan hold text the disk may not have yet.
        let open: Vec<(PathBuf, Option<Rc<Text>>)> = self
            .documents
            .iter()
            .map(|(path, doc)| (path.clone(), Some(doc.text.clone())))
            .collect();
        self.update(&open);
        let summary = self.scan_summary();
        self.log(MessageType::INFO, format!("jscpd: {summary}"));
        self.progress_end(token, Some(summary));
        self.publish_all();
        for project in 0..self.projects.len() {
            self.start_background(project);
        }
    }

    /// Show `message` to the user.
    fn show(&self, typ: MessageType, message: String) {
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::ShowMessage>(
                ShowMessageParams { typ, message },
            ));
    }

    /// Write `message` to the editor's log.
    fn log(&self, typ: MessageType, message: String) {
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::LogMessage>(
                lsp_types::LogMessageParams { typ, message },
            ));
    }

    /// The background analyses of `project`: dead code and semantic clones.
    fn start_background(&mut self, project: usize) {
        self.start_dead_code(project);
        self.start_semantic(project);
    }

    /// Run the semantic analysis of `project` in the background: embed the
    /// functions that changed since the vectors in the cache, and pair them.
    /// Asks before downloading a missing model.
    fn start_semantic(&mut self, project: usize) {
        let Some(p) = self.projects.get(project) else {
            return;
        };
        let Some(options) = p.semantic_options.clone() else {
            return;
        };
        if p.refused.is_some() {
            return;
        }
        if self.semantic_running.contains_key(&project) {
            self.semantic_again.insert(project);
            return;
        }
        if let Some((model, size)) = cpd_semantic::missing_model(&options) {
            self.ask_download(options, &model, size);
            return;
        }
        let run = p.run.clone();
        let excluded = p.plan.excluded.clone();
        let jobs = self.jobs.clone();
        let generation = self.generation;
        let token = self.progress_begin("jscpd", "Finding semantic clones");
        self.semantic_running.insert(project, token);
        std::thread::spawn(move || {
            let clones = semantic_clones(&run, &excluded, &options);
            let _ = jobs.send(JobDone::Semantic {
                project,
                generation,
                clones,
            });
        });
    }

    fn ask_download(&mut self, options: cpd_semantic::SemanticOptions, model: &str, size: u64) {
        if self.model_asked || self.downloading.is_some() {
            return;
        }
        self.model_asked = true;
        let params = lsp_types::ShowMessageRequestParams {
            typ: MessageType::INFO,
            message: format!(
                "jscpd: the semantic analysis needs the model {model} ({:.0} MB). Download it now?",
                size as f64 / 1e6
            ),
            actions: Some(vec![
                lsp_types::MessageActionItem {
                    title: "Download".to_string(),
                    properties: HashMap::new(),
                },
                lsp_types::MessageActionItem {
                    title: "Not now".to_string(),
                    properties: HashMap::new(),
                },
            ]),
        };
        let id = self.send_request::<lsp_types::request::ShowMessageRequest>(params);
        self.prompts.insert(id, Prompt::Download(options));
    }

    /// A reply to a request the server sent.
    fn response(&mut self, response: Response) {
        let Some(prompt) = self.prompts.remove(&response.id) else {
            return;
        };
        match prompt {
            Prompt::Download(options) => {
                let chosen = response
                    .response_result
                    .ok()
                    .and_then(|r| serde_json::from_value::<lsp_types::MessageActionItem>(r).ok());
                if !chosen.is_some_and(|c| c.title == "Download") {
                    return;
                }
                let token = self.progress_begin("jscpd", "Downloading the embedding model");
                self.downloading = Some(token);
                let jobs = self.jobs.clone();
                std::thread::spawn(move || {
                    let result = cpd_semantic::download(&options, true).map(|_| ());
                    let _ = jobs.send(JobDone::Downloaded(result));
                });
            }
        }
    }

    /// Run the dead-code analysis of `project` in the background, or, when a
    /// run is in flight, once more after it.
    fn start_dead_code(&mut self, project: usize) {
        let Some(p) = self.projects.get(project) else {
            return;
        };
        if !p.analyses.has(Analysis::DeadCode) || p.refused.is_some() {
            return;
        }
        if self.dead_code_running.contains_key(&project) {
            self.dead_code_again.insert(project);
            return;
        }
        let Some(config) = p.dead_code_config.clone() else {
            return;
        };
        let token = self.progress_begin("jscpd", "Finding dead code");
        self.dead_code_running.insert(project, token);
        let jobs = self.jobs.clone();
        let generation = self.generation;
        std::thread::spawn(move || {
            let run = super::dead_code::run(&config);
            let _ = jobs.send(JobDone::DeadCode {
                project,
                generation,
                run,
            });
        });
    }

    fn job_done(&mut self, job: JobDone) {
        match job {
            JobDone::DeadCode {
                project,
                generation,
                run,
            } => {
                if generation != self.generation {
                    return;
                }
                let token = self.dead_code_running.remove(&project);
                let mut found = 0;
                if let Some(p) = self.projects.get_mut(project) {
                    // Another project's folder is that project's to report.
                    p.dead_code = run
                        .findings
                        .into_iter()
                        .filter(|(path, _)| p.owns(path))
                        .collect();
                    p.dead_code_snapshots = run.snapshots;
                    found = p.dead_code.len();
                }
                if let Some(token) = token {
                    self.progress_end(token, Some(format!("{found} found")));
                }
                if self.dead_code_again.remove(&project) {
                    self.start_dead_code(project);
                }
                self.publish_all();
            }
            JobDone::Semantic {
                project,
                generation,
                clones,
            } => {
                if generation != self.generation {
                    return;
                }
                let token = self.semantic_running.remove(&project);
                let found = clones.as_ref().map_or(0, |run| run.clones.len());
                if let Some(token) = token {
                    self.progress_end(token, Some(format!("{found} found")));
                }
                match clones {
                    Ok(run) => {
                        if let Some(p) = self.projects.get_mut(project) {
                            p.semantic = run.clones;
                            p.semantic_snapshots = run.snapshots;
                        }
                    }
                    Err(error) => self.show(
                        MessageType::ERROR,
                        format!("jscpd: semantic clones: {error}"),
                    ),
                }
                if self.semantic_again.remove(&project) {
                    self.start_semantic(project);
                }
                self.publish_all();
            }
            JobDone::Downloaded(result) => {
                if let Some(token) = self.downloading.take() {
                    self.progress_end(token, None);
                }
                match result {
                    Ok(()) => {
                        for project in 0..self.projects.len() {
                            self.start_semantic(project);
                        }
                    }
                    Err(error) => {
                        let _ = self
                            .sender
                            .send(notify::<lsp_types::notification::ShowMessage>(
                                ShowMessageParams {
                                    typ: MessageType::ERROR,
                                    message: format!("jscpd: downloading the model: {error}"),
                                },
                            ));
                    }
                }
            }
        }
    }

    fn project_of(&self, path: &Path) -> Option<usize> {
        self.projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.owns(path))
            .max_by_key(|(_, p)| p.depth())
            .map(|(i, _)| i)
    }

    /// Tokenize `files` again, from their texts or, without one, the disk,
    /// in every project whose scan holds them or would take them: the
    /// project of the nearest config, and one whose `path` entries reach
    /// into another's folder. Each project searches each pool once.
    fn update(&mut self, files: &[(PathBuf, Option<Rc<Text>>)]) {
        let pool = &self.pool;
        for project in &mut self.projects {
            let Some(index) = project.index.as_mut() else {
                continue;
            };
            let batch: Vec<(String, Option<&str>)> = files
                .iter()
                .filter(|(path, _)| index.covers(path))
                .map(|(path, text)| {
                    (
                        path.to_string_lossy().into_owned(),
                        text.as_deref().map(|t| t.text.as_str()),
                    )
                })
                .collect();
            if !batch.is_empty() {
                index.update_all(pool, &batch);
            }
        }
    }

    // ------------------------------------------------------------ publishing

    fn text_of(&self, path: &Path) -> Option<Rc<Text>> {
        match self.documents.get(path) {
            Some(doc) => Some(doc.text.clone()),
            None => std::fs::read_to_string(path)
                .ok()
                .map(|t| Rc::new(Text::from_disk(t))),
        }
    }

    /// The findings of the file at `path`, with the clones of each project
    /// grouped by file in `groups`.
    fn findings_of(&self, path: &Path, groups: &[Groups]) -> Vec<Finding> {
        let Some(number) = self.project_of(path) else {
            return Vec::new();
        };
        let project = &self.projects[number];
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
        let id = path.to_string_lossy();
        let none = Vec::new();
        let index_clones = groups[number].index.get(id.as_ref()).unwrap_or(&none);
        let semantic_clones = groups[number].semantic.get(id.as_ref()).unwrap_or(&none);
        let mut findings = clone_findings(
            path,
            &text,
            index_clones.iter().copied(),
            &scope,
            &mut other_text,
            None,
        );
        findings.extend(clone_findings(
            path,
            &text,
            semantic_clones.iter().copied(),
            &scope,
            &mut other_text,
            Some(&project.semantic_snapshots),
        ));
        if project.analyses.has(Analysis::Complexity)
            && let Some(format) = index.walk_format(path)
        {
            let limits = Limits {
                function: project.analyses.function_limit,
                file: project
                    .options
                    .health
                    .complex_file
                    .unwrap_or(cpd_core::health::COMPLEX_FILE),
            };
            findings.extend(complexity_findings(
                &text,
                &format,
                project.options.mode,
                &limits,
                self.encoding,
            ));
        }
        if project.analyses.has(Analysis::DeadCode) {
            findings.extend(dead_code_findings(path, &text, project, self.encoding));
        }
        findings.sort_by_key(|f| (f.range.start.line, f.range.start.character));
        findings
    }

    /// Publish the diagnostics of every open file and, for projects that
    /// ask, of every file with findings; clear the files that have none
    /// now. A file whose findings did not change is not sent again.
    fn publish_all(&mut self) {
        let mut files: BTreeSet<PathBuf> = self.documents.keys().cloned().collect();
        for project in &self.projects {
            if !project.analyses.all_files {
                continue;
            }
            let clones = project.index.iter().flat_map(|index| index.clones());
            for clone in clones.chain(project.semantic.iter()) {
                for fragment in [&clone.fragment_a, &clone.fragment_b] {
                    files.insert(PathBuf::from(host_file(&fragment.source_id)));
                }
            }
            files.extend(project.dead_code.iter().map(|(path, _)| path.clone()));
        }
        let groups: Vec<Groups> = self.projects.iter().map(Groups::of).collect();
        let found: Vec<(PathBuf, Vec<Finding>)> = files
            .iter()
            .map(|path| (path.clone(), self.findings_of(path, &groups)))
            .collect();
        drop(groups);
        let stale: Vec<PathBuf> = self
            .published
            .keys()
            .filter(|path| !files.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            self.send_diagnostics(&path, Vec::new());
            self.findings.remove(&path);
        }
        for (path, findings) in found {
            let same = self.findings.get(&path) == Some(&findings)
                && self.uri_of(&path).as_ref() == self.published.get(&path);
            if same {
                continue;
            }
            let diagnostics = findings
                .iter()
                .map(|f| diagnostic(f, self.caps.related_information))
                .collect();
            self.send_diagnostics(&path, diagnostics);
            self.findings.insert(path, findings);
        }
    }

    /// The URI to publish the diagnostics of `path` at: the one the editor
    /// opened it by, then the one its diagnostics were last shown at, so a
    /// file reached through a link is not shown twice.
    fn uri_of(&self, path: &Path) -> Option<Uri> {
        match self.documents.get(path) {
            Some(doc) => Some(doc.uri.clone()),
            None => self
                .published
                .get(path)
                .cloned()
                .or_else(|| path_to_uri(path)),
        }
    }

    fn send_diagnostics(&mut self, path: &Path, diagnostics: Vec<Diagnostic>) {
        let Some(uri) = self.uri_of(path) else {
            return;
        };
        let version = self.documents.get(path).map(|doc| doc.version);
        // Shown elsewhere before: clear it there.
        if let Some(old) = self.published.get(path)
            && *old != uri
        {
            let old = old.clone();
            self.publish(old, Vec::new(), None);
        }
        if diagnostics.is_empty() {
            if self.published.remove(path).is_none() {
                return;
            }
        } else {
            self.published.insert(path.to_path_buf(), uri.clone());
        }
        self.publish(uri, diagnostics, version);
    }

    fn publish(&self, uri: Uri, diagnostics: Vec<Diagnostic>, version: Option<i32>) {
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
        let files: Vec<(PathBuf, Option<Rc<Text>>)> = pending
            .into_iter()
            .map(|path| {
                let text = self.documents.get(&path).map(|d| d.text.clone());
                (path, text)
            })
            .collect();
        self.update(&files);
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
                self.update(&[(path, Some(text))]);
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
                let saved = serde_json::from_value::<lsp_types::DidSaveTextDocumentParams>(
                    notification.params,
                )
                .ok()
                .and_then(|p| document_path(&p.text_document.uri));
                // A config saved in the editor applies at once, also for a
                // client that does not watch files.
                if saved
                    .as_deref()
                    .is_some_and(|path| path.file_name().is_some_and(|n| n == CONFIG_NAME))
                {
                    self.load();
                    return;
                }
                // The analyses in the background read the disk, which has
                // the buffer now, so they need not wait for the search below.
                if let Some(path) = &saved {
                    for project in self.projects_covering(path) {
                        self.start_background(project);
                    }
                }
                // Anything waiting for the quiet period runs at once.
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
                self.update(&[(path, None)]);
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
                // `null` asks a server to pull its settings, which this one
                // does not; settings that did not change need no rescan.
                if !params.settings.is_object() {
                    return;
                }
                let settings = settings_of(Some(&params.settings));
                if settings == self.settings {
                    return;
                }
                self.settings = settings;
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
        let mut changed: Vec<PathBuf> = Vec::new();
        let mut manifests: Vec<PathBuf> = Vec::new();
        for change in params.changes {
            let Some(path) = uri_to_path(&change.uri).map(canonical) else {
                continue;
            };
            // Git's own files change with every commit; ignored folders such
            // as node_modules change with every install.
            if path.components().any(|c| c.as_os_str() == ".git") || self.ignored(&path) {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // They move the borders between projects, or what a walk takes.
            if matches!(name, CONFIG_NAME | ".gitignore" | ".ignore") {
                self.load();
                return;
            }
            // An open file is the editor's: its buffer wins over the disk,
            // and its saves arrive as didSave.
            if self.documents.contains_key(&path) {
                continue;
            }
            // Files outside the index that change what dead code counts as
            // used: entry points, path aliases.
            if MANIFESTS.contains(&name) {
                manifests.push(path.clone());
            }
            changed.push(path);
        }
        let mut touched: BTreeSet<usize> = manifests
            .iter()
            .flat_map(|path| self.projects_covering(path))
            .collect();
        let pool = &self.pool;
        for (number, project) in self.projects.iter_mut().enumerate() {
            let Some(index) = project.index.as_mut() else {
                continue;
            };
            let ids: Vec<String> = changed
                .iter()
                .filter(|path| index.covers(path))
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            if !ids.is_empty() && index.refresh(pool, &ids) {
                touched.insert(number);
            }
        }
        if touched.is_empty() {
            return;
        }
        self.publish_all();
        for project in touched {
            self.start_background(project);
        }
    }

    /// The projects whose scans reach the file at `path`.
    fn projects_covering(&self, path: &Path) -> Vec<usize> {
        (0..self.projects.len())
            .filter(|&i| {
                self.projects[i]
                    .index
                    .as_ref()
                    .is_some_and(|index| index.covers(path))
            })
            .collect()
    }

    /// Whether the ignore files of the workspace leave `path` out, the way
    /// its scans do.
    fn ignored(&self, path: &Path) -> bool {
        let Some(folder) = self.folders.iter().find(|folder| path.starts_with(folder)) else {
            return true;
        };
        let no_gitignore = self
            .project_of(path)
            .is_some_and(|i| self.projects[i].options.no_gitignore);
        cpd_finder::walker::ignored_by_files(path, folder, false, no_gitignore)
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
            CodeActionRequest::METHOD => {
                serde_json::from_value::<CodeActionParams>(request.params).map(|p| {
                    // An edit still in the quiet period would leave the
                    // findings behind the text the actions edit.
                    if document_path(&p.text_document.uri)
                        .is_some_and(|path| self.pending.contains(&path))
                    {
                        self.flush();
                    }
                    serde_json::to_value(self.code_actions(p)).unwrap_or(Value::Null)
                })
            }
            HoverRequest::METHOD => serde_json::from_value::<HoverParams>(request.params)
                .map(|p| serde_json::to_value(self.hover(p)).unwrap_or(Value::Null)),
            ExecuteCommand::METHOD => {
                serde_json::from_value::<ExecuteCommandParams>(request.params)
                    .map(|p| self.execute(p))
            }
            "jscpd/clones" => Ok(self.clones_report()),
            "jscpd/statistics" => Ok(self.statistics()),
            "jscpd/deadCode" => Ok(self.dead_code_report()),
            "jscpd/semantic" => Ok(self.semantic_report()),
            "jscpd/complexity" => Ok(self.complexity_report()),
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
                    Analysis::Ast | Analysis::Semantic => format!(
                        "Go to the similar {} in {}",
                        finding.unit.noun(),
                        target.label
                    ),
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

    /// Wrap a finding in `jscpd:ignore-start` and `jscpd:ignore-end`
    /// comments: on lines of their own, indented like its first line, where
    /// the finding starts and ends its lines, and as block comments beside it
    /// where other code shares them, so the code around keeps its meaning
    /// (a comment line inside `<script>export function …` would land in the
    /// markup). The first line of a file that starts with `#!` or `<?` stays
    /// first.
    fn ignore_edit(&self, uri: &Uri, finding: &Finding) -> Option<WorkspaceEdit> {
        let (open, close) = comment_syntax(&finding.format)?;
        let block = block_comment_syntax(&finding.format);
        let path = document_path(uri)?;
        let text = self.text_of(&path)?;
        let eol = match text.text.contains("\r\n") {
            true => "\r\n",
            false => "\n",
        };
        let line = |n: u32| text.index.line(&text.text, n as usize);
        let lines = text.index.line_count() as u32;
        let indent: String = line(finding.first_line)
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let (start, end) = (finding.range.start, finding.range.end);
        let before = &line(start.line)[..byte_at(line(start.line), start.character, self.encoding)];
        let after = &line(end.line)[byte_at(line(end.line), end.character, self.encoding)..];
        let at =
            |position: Position, text: String| TextEdit::new(Range::new(position, position), text);
        let mut edits = Vec::new();
        match (before.trim().is_empty(), block) {
            (false, Some((block_open, block_close))) => {
                edits.push(at(
                    start,
                    format!("{block_open}jscpd:ignore-start{block_close} "),
                ));
            }
            _ => {
                let first = line(finding.first_line);
                let keep_first =
                    finding.first_line == 0 && (first.starts_with("#!") || first.starts_with("<?"));
                let (row, indent) = match keep_first {
                    true if lines > 1 => (
                        1,
                        line(1).chars().take_while(|c| c.is_whitespace()).collect(),
                    ),
                    true => return None,
                    false => (finding.first_line, indent.clone()),
                };
                edits.push(at(
                    Position::new(row, 0),
                    format!("{indent}{open}jscpd:ignore-start{close}{eol}"),
                ));
            }
        }
        match (after.trim().is_empty(), block) {
            (false, Some((block_open, block_close))) => {
                edits.push(at(
                    end,
                    format!(" {block_open}jscpd:ignore-end{block_close}"),
                ));
            }
            _ if finding.last_line + 1 < lines => {
                edits.push(at(
                    Position::new(finding.last_line + 1, 0),
                    format!("{indent}{open}jscpd:ignore-end{close}{eol}"),
                ));
            }
            _ => {
                let last = line(finding.last_line);
                let column = match self.encoding {
                    Encoding::Utf8 => last.len(),
                    Encoding::Utf16 => last.encode_utf16().count(),
                };
                edits.push(at(
                    Position::new(finding.last_line, column as u32),
                    format!("{eol}{indent}{open}jscpd:ignore-end{close}"),
                ));
            }
        }
        // Edits that name the version they were made for, when the client
        // takes them, so a stale edit is refused rather than misplaced.
        if self.caps.document_changes
            && let Some(doc) = self.documents.get(&path)
        {
            return Some(WorkspaceEdit {
                document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                    lsp_types::TextDocumentEdit {
                        text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                            uri: uri.clone(),
                            version: Some(doc.version),
                        },
                        edits: edits.into_iter().map(lsp_types::OneOf::Left).collect(),
                    },
                ])),
                ..WorkspaceEdit::default()
            });
        }
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
            if !finding.notes.is_empty() {
                section.push_str("\n\nMight be wrong: ");
                section.push_str(&finding.notes.join("; "));
            }
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

    /// The answer of a custom request: one entry per project that `each`
    /// has something for, with the project's roots and config file.
    fn per_project(&self, each: impl Fn(&Project) -> Option<Value>) -> Value {
        let projects: Vec<Value> = self
            .projects
            .iter()
            .filter_map(|p| {
                let mut entry = each(p)?;
                entry["roots"] = json!(p.options.paths);
                entry["config"] = json!(p.plan.config_dir.as_ref().map(|d| d.join(CONFIG_NAME)));
                Some(entry)
            })
            .collect();
        json!({ "projects": projects })
    }

    /// Clones as `jscpd-report.json` lists them, with the fragments taken
    /// from the editor's buffers where a file is open.
    fn duplicates<'c>(
        &self,
        clones: impl Iterator<Item = &'c cpd_core::models::CpdClone>,
    ) -> Vec<Value> {
        let mut texts: HashMap<String, String> = self
            .documents
            .iter()
            .map(|(path, doc)| (path.to_string_lossy().into_owned(), doc.text.text.clone()))
            .collect();
        clones
            .map(|clone| cpd_reporter::json_reporter::clone_to_dup(clone, false, &mut texts))
            .collect()
    }

    /// Every clone of each project, with its statistics: the two keys of
    /// `jscpd-report.json`.
    fn clones_report(&self) -> Value {
        self.per_project(|p| {
            let index = p.index.as_ref()?;
            // The order of a scan's report.
            let mut clones: Vec<&cpd_core::models::CpdClone> = index.clones().collect();
            clones.sort_by(|a, b| a.position_key().cmp(&b.position_key()));
            Some(json!({
                "statistics": index.statistics(),
                "duplicates": self.duplicates(clones.into_iter()),
            }))
        })
    }

    /// The last semantic pairs of each project with the analysis on, in the
    /// shape of `jscpd/clones`.
    fn semantic_report(&self) -> Value {
        self.per_project(|p| {
            p.analyses
                .has(Analysis::Semantic)
                .then(|| json!({ "duplicates": self.duplicates(p.semantic.iter()) }))
        })
    }

    /// The last dead-code findings of each project with the analysis on, as
    /// `basta-report.json` lists them, with absolute paths.
    fn dead_code_report(&self) -> Value {
        self.per_project(|p| {
            p.analyses.has(Analysis::DeadCode).then(|| {
                let findings: Vec<Value> = p
                    .dead_code
                    .iter()
                    .map(|(path, finding)| {
                        let mut finding = json!(finding);
                        finding["path"] = json!(path);
                        finding
                    })
                    .collect();
                json!({ "findings": findings })
            })
        })
    }

    /// The complexity summary of each project, as `--complexity --absolute`
    /// computes it: every file with its CX, and the folders. The server has
    /// the clones too, so the duplication columns are filled in.
    fn complexity_report(&self) -> Value {
        self.per_project(|p| {
            let index = p.index.as_ref()?;
            let clones: Vec<cpd_core::models::CpdClone> = index.clones().cloned().collect();
            let summary = cpd_core::summary::compute_summary(
                &index.sources(),
                &clones,
                usize::MAX,
                cpd_core::summary::SummaryMetric::Complexity,
                str::to_string,
            );
            Some(json!({ "summary": summary }))
        })
    }

    fn statistics(&self) -> Value {
        self.per_project(|p| {
            let index = p.index.as_ref()?;
            Some(json!({
                "files": index.file_count(),
                "statistics": index.statistics(),
            }))
        })
    }

    // ------------------------------------------------------------ progress

    fn send_request<R: lsp_types::request::Request>(&mut self, params: R::Params) -> RequestId {
        self.next_id += 1;
        let id = RequestId::from(format!("jscpd-{}", self.next_id));
        let request = Request::new(id.clone(), R::METHOD.to_string(), params);
        let _ = self.sender.send(Message::Request(request));
        id
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

    /// End a progress, with what it found for an editor to show last.
    fn progress_end(&mut self, token: Option<ProgressToken>, message: Option<String>) {
        let Some(token) = token else { return };
        let _ = self
            .sender
            .send(notify::<lsp_types::notification::Progress>(
                ProgressParams {
                    token,
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                        WorkDoneProgressEnd { message },
                    )),
                },
            ));
    }

    /// What the scan found, for the end of its progress and the log: the
    /// one sign of a working server in a project without findings.
    fn scan_summary(&self) -> String {
        let indexes: Vec<&ScanIndex> = self
            .projects
            .iter()
            .filter_map(|p| p.index.as_ref())
            .collect();
        let files: usize = indexes.iter().map(|index| index.file_count()).sum();
        let clones: usize = indexes.iter().map(|index| index.clones().count()).sum();
        let counted = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let found = format!(
            "{}, {}",
            counted(files, "file", "files"),
            counted(clones, "clone", "clones")
        );
        match indexes.len() {
            0 => "no folder to scan".to_string(),
            1 => found,
            n => format!("{n} projects, {found}"),
        }
    }
}

/// The semantic pairs of a project, by a run of `--semantic` over its files
/// on disk; the vectors of functions that did not change come from the
/// cache.
fn semantic_clones(
    run: &cpd_finder::orchestrate::RunConfig,
    excluded: &[PathBuf],
    options: &cpd_semantic::SemanticOptions,
) -> Result<SemanticRun, String> {
    let started = std::time::SystemTime::now();
    let embedder = cpd_semantic::embedder(options, &run.paths, true)?;
    let mut config = run.clone();
    // The index finds the other kinds; this run is for the pairs alone.
    config.similarity = 1.0;
    config.passes = vec![std::sync::Arc::new(cpd_semantic::SemanticPass::new(
        embedder,
        options.thresholds(),
        options.scope,
    ))];
    let result =
        cpd_finder::orchestrate::run_excluding(&config, excluded).map_err(|e| e.to_string())?;
    let clones: Vec<cpd_core::models::CpdClone> = result
        .clones
        .into_iter()
        .filter(|clone| clone.kind.is_semantic())
        .collect();
    let snapshots = clones
        .iter()
        .flat_map(|clone| [&clone.fragment_a.source_id, &clone.fragment_b.source_id])
        .map(|id| PathBuf::from(host_file(id)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| Some((path.clone(), Snapshot::of_disk(&path, started)?)))
        .collect();
    Ok(SemanticRun { clones, snapshots })
}

fn notify<N: lsp_types::notification::Notification>(params: N::Params) -> Message {
    Message::Notification(Notification::new(N::METHOD.to_string(), params))
}

/// The byte of `line` at `character`, a column counted in `encoding`.
fn byte_at(line: &str, character: u32, encoding: Encoding) -> usize {
    let mut units = 0;
    for (byte, c) in line.char_indices() {
        if units >= character as usize {
            return byte;
        }
        units += match encoding {
            Encoding::Utf8 => c.len_utf8(),
            Encoding::Utf16 => c.len_utf16(),
        };
    }
    line.len()
}

/// The path of a document, canonical like the paths a scan gives its files.
fn document_path(uri: &Uri) -> Option<PathBuf> {
    uri_to_path(uri).map(canonical)
}

/// `path` as the index keeps it, with its links resolved. A deleted file,
/// or a file in a deleted folder, has no such path of its own, so it gets
/// the one of the nearest folder above it that is still there.
fn canonical(path: PathBuf) -> PathBuf {
    for base in path.ancestors() {
        if let Ok(real) = std::fs::canonicalize(base) {
            return match path.strip_prefix(base) {
                Ok(rest) if !rest.as_os_str().is_empty() => real.join(rest),
                _ => real,
            };
        }
    }
    path
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_under_a_deleted_folder_is_anchored_at_the_nearest_folder_left() {
        let dir = std::env::temp_dir();
        let gone = dir
            .join("cpd-lsp-canonical-gone")
            .join("deeper")
            .join("a.js");
        assert_eq!(
            canonical(gone),
            std::fs::canonicalize(&dir)
                .unwrap()
                .join("cpd-lsp-canonical-gone")
                .join("deeper")
                .join("a.js")
        );
    }

    #[test]
    fn a_column_is_found_in_bytes_for_either_encoding() {
        let line = "é𝄞x";
        assert_eq!(byte_at(line, 3, Encoding::Utf16), line.find('x').unwrap());
        assert_eq!(byte_at(line, 6, Encoding::Utf8), line.find('x').unwrap());
        assert_eq!(byte_at(line, 99, Encoding::Utf16), line.len());
    }
}
