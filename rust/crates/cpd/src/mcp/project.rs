//! What the MCP tools search: the project's scan, in the variants requests
//! need, and what each kind of clone finds in it. Everything is kept until
//! `check_current_directory` scans again.
//!
//! How each kind is found:
//!
//! - `exact` (Type-1) and `renamed` (Type-2): the token passes of a scan.
//!   Renamed clones need normalized tokens: the normalization the options
//!   configure (`--ignore-identifiers`, `--ignore-literals`,
//!   `--ignore-annotations`), or, when they configure none, a second scan
//!   with identifiers and literals normalized. A request without `renamed`
//!   reads the configured scan, so its exact clones are the ones a `jscpd`
//!   run with the same options reports.
//! - `similar` (Type-3) by `gap`: the clones of one file pair merged across
//!   at most `--max-gap-lines` unmatched lines, 2 when the option is not set.
//! - `similar` by `ast`: functions whose normalized syntax trees share
//!   subtrees, at `--similarity`, 0.8 when not set, of at least
//!   `--min-nodes` nodes, from the flags or the config file.
//! - `semantic` (Type-4): functions that do the same job, by the model of
//!   `--semantic`. The model has to be on this machine or behind an
//!   embeddings API; the server never downloads it.
//!
//! A scan reads the syntax trees and the functions the last two kinds need
//! from the same text as the tokens, so every result describes the same
//! files. A request for a kind its scan did not read scans again.
//!
//! The token passes always run; the other kinds run when a request asks
//! for them or the options switch them on, as in a `jscpd` run. A pair of
//! functions an earlier detector reports is left out of the later ones: ast
//! pairs leave out what the token clones cover, and semantic pairs what the
//! token clones and, when ast runs, the ast pairs cover, whichever kinds
//! the request lists. An exact copy of a function is no ast pair.

use crate::index::{ScanIndex, host_file};
use crate::options::Options;
use cpd_core::detect::{PathFilters, PreparedSource, detect_prepared, merge_gapped_clones};
use cpd_core::models::{
    CloneKind, CpdClone, DetectionToken, Fragment, KindFilter, Location, SimilarityMethod,
    Statistics,
};
use cpd_finder::orchestrate::{
    RunConfig, build_thread_pool, canonicalize_all, pool_key, splits_blocks, strip_types_formats,
};
use cpd_finder::pass::{ClonePass, PassSource};
use cpd_semantic::search::{
    Embedder, SemanticParams, SourceVectors, UnitSource, find_semantic_matches, pair_embedded,
};
use cpd_semantic::{SemanticOptions, UnitReader};
use cpd_similarity::{Coverage, FormSource, SimilarityIndex, discount_token_lines};
use cpd_tokenizer::tokenizer::{
    TokenMap, TokenizeOptions, code_ignore_ranges, compile_ignore_patterns, tokenize_to_detection,
    tokenize_to_detection_maps,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

/// The source id of the snippet a check compares with the project. A block
/// of another language in it, a fence of a Markdown snippet or the script of
/// a component, is `snippet://check:<format>`, as a scan names a block.
pub(super) const SNIPPET_ID: &str = "snippet://check";

/// Whether `id` is the snippet's or one of its blocks'.
pub(super) fn is_snippet(id: &str) -> bool {
    id.strip_prefix(SNIPPET_ID)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
}

/// The two sides of a match: the snippet's fragment, the project's, and the
/// index of the project's in the clone (0 for fragment A).
pub(super) fn sides_of(clone: &CpdClone) -> (&Fragment, &Fragment, usize) {
    match is_snippet(&clone.fragment_a.source_id) {
        true => (&clone.fragment_a, &clone.fragment_b, 1),
        false => (&clone.fragment_b, &clone.fragment_a, 0),
    }
}

/// `--max-gap-lines` for gap clones when the options set none: a line or
/// two inserted, removed or changed in a copy.
const DEFAULT_GAP_LINES: usize = 2;
/// `--similarity` for ast clones when the options set none, as for the
/// language server.
const DEFAULT_AST_SIMILARITY: f64 = crate::lsp::settings::DEFAULT_AST_SIMILARITY;

/// What the server runs with, from the command line and the config file.
pub struct Settings {
    /// The scan: paths, filters, thresholds and the detectors configured.
    /// Its `kinds` (`--kind`) are the kinds the tools look for by default.
    run: RunConfig,
    /// The model and thresholds of the semantic kind and of compare_folders.
    semantic: SemanticOptions,
    /// Whether `--semantic` asked for semantic clones: the tools then look
    /// for them by default.
    semantic_requested: bool,
    /// The `--min-tokens` of compare_folders.
    compare_min_tokens: usize,
}

impl Settings {
    pub fn new(opts: &Options, run: RunConfig) -> Self {
        Self {
            run,
            semantic: opts.semantic.clone().unwrap_or_default(),
            semantic_requested: opts.semantic_requested,
            compare_min_tokens: opts.compare_min_tokens,
        }
    }

    /// The settings of a scan with `run` and the semantic defaults.
    #[cfg(test)]
    pub(super) fn of_run(run: RunConfig) -> Self {
        Self {
            run,
            semantic: SemanticOptions::default(),
            semantic_requested: false,
            compare_min_tokens: 30,
        }
    }

    /// The same settings with `semantic` as the model's, asked for by
    /// default when `requested`.
    #[cfg(test)]
    pub(super) fn with_semantic(self, semantic: SemanticOptions, requested: bool) -> Self {
        Self {
            semantic,
            semantic_requested: requested,
            ..self
        }
    }
}

/// The kinds of clone a request looks for: the four types, Type-3 by
/// either of the two ways jscpd finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) struct Kinds {
    pub exact: bool,
    pub renamed: bool,
    /// Type-3: clones merged across a gap of unmatched lines.
    pub gap: bool,
    /// Type-3: functions with the same syntax-tree shape.
    pub ast: bool,
    pub semantic: bool,
}

impl Kinds {
    /// The kinds named by `names`: the names of `--kind`, or `type1` to
    /// `type4` (`type-1` to `type-4`).
    pub fn parse<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut kinds = Kinds::default();
        for name in names {
            let filter = match name.trim().to_ascii_lowercase().replace('-', "").as_str() {
                "type1" => KindFilter::Exact,
                "type2" => KindFilter::Renamed,
                "type3" => KindFilter::Similar,
                "type4" => KindFilter::Semantic,
                other => other.parse().map_err(|_| {
                    format!(
                        "unknown clone kind '{name}': use exact (type1), renamed (type2), similar (type3; gap or ast for one of its two mechanisms) or semantic (type4)"
                    )
                })?,
            };
            kinds.add(filter);
        }
        Ok(kinds)
    }

    fn add(&mut self, filter: KindFilter) {
        match filter {
            KindFilter::Exact => self.exact = true,
            KindFilter::Renamed => self.renamed = true,
            KindFilter::Similar => {
                self.gap = true;
                self.ast = true;
            }
            KindFilter::Gap => self.gap = true,
            KindFilter::Ast => self.ast = true,
            KindFilter::Semantic => self.semantic = true,
        }
    }

    /// Whether `clone` is of one of the kinds.
    pub fn matches(&self, clone: &CpdClone) -> bool {
        match clone.kind {
            CloneKind::Exact => self.exact,
            CloneKind::Renamed => self.renamed,
            CloneKind::Similar => match clone.similarity_method {
                Some(SimilarityMethod::Ast) => self.ast,
                _ => self.gap,
            },
            CloneKind::Semantic => self.semantic,
        }
    }

    /// The kinds by name, as results list them.
    pub fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.exact {
            names.push("exact");
        }
        if self.renamed {
            names.push("renamed");
        }
        match (self.gap, self.ast) {
            (true, true) => names.push("similar"),
            (true, false) => names.push("gap"),
            (false, true) => names.push("ast"),
            (false, false) => {}
        }
        if self.semantic {
            names.push("semantic");
        }
        names
    }
}

/// A scan of the project with the configured options, or with identifiers
/// and literals normalized for renamed clones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Configured = 0,
    Renamed = 1,
}

/// What a scan reads besides the tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Reads {
    /// The syntax trees of JavaScript, TypeScript and Python functions, for
    /// ast clones.
    functions: bool,
    /// The functions the semantic model reads.
    units: bool,
}

impl Reads {
    fn includes(self, other: Reads) -> bool {
        (self.functions || !other.functions) && (self.units || !other.units)
    }

    fn and(self, other: Reads) -> Reads {
        Reads {
            functions: self.functions || other.functions,
            units: self.units || other.units,
        }
    }
}

/// One scan, and what the requests found in it, kept for the next ones.
struct Scan {
    index: ScanIndex,
    reads: Reads,
    /// The functions the semantic model reads, labelled for the path
    /// filters; empty unless the scan read them.
    units: Vec<UnitSource>,
    /// The JavaScript, TypeScript and Python functions, as an index to
    /// search for a snippet's; made on first use.
    functions: Option<SimilarityIndex>,
    /// The vectors of `units`, embedded on first use.
    vectors: Option<SourceVectors>,
    /// The semantic pairs among `units`, the ast pairs left out of them or
    /// not (see [`Project::clones`]).
    semantic: [Option<Vec<CpdClone>>; 2],
    /// What each set of kinds found.
    found: HashMap<Kinds, Arc<Found>>,
}

impl Scan {
    /// Scan with `run`, reading what `reads` says: ast pairs at
    /// `ast_similarity` when it reads syntax trees.
    fn make(
        pool: &rayon::ThreadPool,
        mut run: RunConfig,
        reads: Reads,
        ast_similarity: f64,
        filters: &PathFilters,
    ) -> Self {
        let reader = Arc::new(UnitReader::default());
        if reads.functions {
            run.similarity = Some(ast_similarity);
        }
        if reads.units {
            run.passes = vec![reader.clone()];
        }
        let index = ScanIndex::build(pool, run, Vec::new());
        let mut units = reader.take_sources();
        if filters.is_active() {
            for source in &mut units {
                source.path_label = filters.label(&source.id);
            }
        }
        Self {
            index,
            reads,
            units,
            functions: None,
            vectors: None,
            semantic: [None, None],
            found: HashMap::new(),
        }
    }

    /// The functions of the scan, to search for a snippet's.
    fn functions(&mut self, run: &RunConfig) -> &SimilarityIndex {
        let index = &self.index;
        self.functions.get_or_insert_with(|| {
            SimilarityIndex::build(
                index.function_sources(),
                run.min_nodes,
                run.min_lines as u32,
            )
        })
    }

    /// The vectors of the scan's functions, embedded on the first call.
    fn vectors(
        &mut self,
        pool: &rayon::ThreadPool,
        embedder: &dyn Embedder,
        params: &SemanticParams,
    ) -> Result<&SourceVectors, String> {
        if self.vectors.is_none() {
            let vectors = pool.install(|| SourceVectors::embed(&self.units, embedder, params))?;
            self.vectors = Some(vectors);
        }
        Ok(self
            .vectors
            .as_ref()
            .expect("the vectors were just embedded"))
    }

    /// The semantic pairs of the scan, leaving out the pairs its token
    /// clones cover and, `with_ast`, its ast pairs.
    fn semantic_pairs(
        &mut self,
        pool: &rayon::ThreadPool,
        embedder: &dyn Embedder,
        params: &SemanticParams,
        with_ast: bool,
    ) -> Result<&[CpdClone], String> {
        let slot = usize::from(with_ast);
        if self.semantic[slot].is_none() {
            let mut existing: Vec<CpdClone> = self.index.token_clones().cloned().collect();
            if with_ast {
                existing.extend(self.index.similar_pairs().iter().cloned());
            }
            self.vectors(pool, embedder, params)?;
            let vectors = self
                .vectors
                .as_ref()
                .expect("the vectors were just embedded");
            let pairs = pool.install(|| pair_embedded(&self.units, vectors, params, &existing))?;
            self.semantic[slot] = Some(pairs);
        }
        Ok(self.semantic[slot].as_deref().unwrap_or_default())
    }
}

/// The project's clones a request asked for.
pub(super) struct Found {
    /// Biggest first.
    pub clones: Vec<CpdClone>,
    pub statistics: Statistics,
    pub files: usize,
    /// The kinds asked for that could not be looked for, and why.
    pub unavailable: Vec<(&'static str, String)>,
}

/// A match of a snippet: the clone between the snippet ([`SNIPPET_ID`]) and
/// the project, with the names of the two functions when functions matched.
pub(super) struct Match {
    pub clone: CpdClone,
    /// The project's function, then the snippet's.
    pub names: Option<(String, String)>,
}

/// What a snippet check found.
pub(super) struct Checked {
    /// The format the snippet was read as.
    pub format: String,
    /// The kinds looked for: the request's, with ast as `similarity` set it.
    pub kinds: Kinds,
    /// Exact matches first, then renamed, similar and semantic ones.
    pub matches: Vec<Match>,
    /// What of the snippet could not match, and why: a snippet or a block
    /// of code in it too short for the token passes, a component without a
    /// script.
    pub note: Option<String>,
    pub unavailable: Vec<(&'static str, String)>,
}

pub(super) struct Project {
    settings: Settings,
    pool: Arc<rayon::ThreadPool>,
    /// The canonical scan roots, which displayed paths are relative to.
    roots: Vec<PathBuf>,
    /// The canonical folders of `--skip-isolated`.
    isolated: Vec<Vec<PathBuf>>,
    /// The kinds a request without `kinds` looks for.
    defaults: Kinds,
    /// The scans by [`Variant`].
    scans: [Option<Scan>; 2],
    /// The first scan, running in the background since the server started.
    pending: Option<(Variant, JoinHandle<Scan>)>,
    /// The model of the semantic kind, made once.
    embedder: Option<Arc<dyn Embedder>>,
}

impl Project {
    /// The project of `settings`, its scan for the default kinds started in
    /// the background.
    pub fn new(settings: Settings) -> Self {
        // Without --kind, the tools report what a `jscpd` run with the same
        // options reports, and a request asks for any other kind itself.
        let defaults = match settings.run.kinds.is_empty() {
            true => {
                let run = &settings.run;
                Kinds {
                    exact: true,
                    renamed: run.ignore_identifiers
                        || run.ignore_literals
                        || run.ignore_annotations,
                    gap: run.max_gap_lines > 0,
                    ast: run.similarity_threshold().is_some(),
                    semantic: settings.semantic_requested,
                }
            }
            false => {
                let mut kinds = Kinds::default();
                for filter in &settings.run.kinds {
                    kinds.add(*filter);
                }
                kinds
            }
        };
        let mut project = Self {
            pool: Arc::new(build_thread_pool(settings.run.workers)),
            roots: canonicalize_all(&settings.run.paths),
            isolated: settings
                .run
                .skip_isolated
                .iter()
                .map(|group| canonicalize_all(group))
                .collect(),
            defaults,
            settings,
            scans: [None, None],
            pending: None,
            embedder: None,
        };
        let variant = project.variant(defaults);
        let reads = project.reads_for(defaults);
        let run = project.run_for(variant);
        let ast_similarity = project.ast_similarity();
        let (pool, roots, isolated) = (
            project.pool.clone(),
            project.roots.clone(),
            project.isolated.clone(),
        );
        let started = std::time::Instant::now();
        let scan = std::thread::Builder::new()
            .name("jscpd-mcp-scan".to_string())
            .spawn(move || {
                let filters = PathFilters {
                    skip_local: run.skip_local,
                    scan_roots: &roots,
                    isolated_groups: &isolated,
                };
                let scan = Scan::make(&pool, run.clone(), reads, ast_similarity, &filters);
                eprintln!(
                    "jscpd MCP server: scanned {} files in {:.0}ms",
                    scan.index.file_count(),
                    started.elapsed().as_secs_f64() * 1000.0
                );
                scan
            });
        match scan {
            Ok(handle) => project.pending = Some((variant, handle)),
            Err(_) => project.scans[variant as usize] = Some(project.make_scan(variant, reads)),
        }
        project
    }

    /// The kinds a request without `kinds` looks for.
    pub fn defaults(&self) -> Kinds {
        self.defaults
    }

    /// The scan `kinds` read: the normalized one when they include renamed
    /// clones and the options normalize nothing.
    fn variant(&self, kinds: Kinds) -> Variant {
        let run = &self.settings.run;
        let normalizes = run.ignore_identifiers || run.ignore_literals || run.ignore_annotations;
        match kinds.renamed && !normalizes {
            true => Variant::Renamed,
            false => Variant::Configured,
        }
    }

    /// What a scan reads for `kinds`: syntax trees for ast clones, and for
    /// semantic ones when `--similarity` puts the ast pairs among the
    /// clones they leave out; the semantic model's functions for semantic
    /// clones.
    fn reads_for(&self, kinds: Kinds) -> Reads {
        Reads {
            functions: kinds.ast || (kinds.semantic && self.ast_configured()),
            units: kinds.semantic,
        }
    }

    /// The options of a scan of `variant`: the server's, without the passes
    /// the server runs itself or the filter the tools apply.
    fn run_for(&self, variant: Variant) -> RunConfig {
        let mut run = self.settings.run.clone();
        run.kinds.clear();
        run.similarity = None;
        run.passes.clear();
        if variant == Variant::Renamed {
            run.ignore_identifiers = true;
            run.ignore_literals = true;
        }
        run
    }

    fn make_scan(&self, variant: Variant, reads: Reads) -> Scan {
        let run = self.run_for(variant);
        let filters = PathFilters {
            skip_local: run.skip_local,
            scan_roots: &self.roots,
            isolated_groups: &self.isolated,
        };
        Scan::make(&self.pool, run, reads, self.ast_similarity(), &filters)
    }

    /// Wait for the scan running in the background, if one is.
    fn settle(&mut self) {
        if let Some((variant, handle)) = self.pending.take() {
            let scan = match handle.join() {
                Ok(scan) => scan,
                Err(_) => self.make_scan(variant, Reads::default()),
            };
            self.scans[variant as usize] = Some(scan);
        }
    }

    /// The scan of `variant`, made now when there is none, or again when
    /// the one there did not read what `reads` asks for.
    fn scan(&mut self, variant: Variant, reads: Reads) -> &mut Scan {
        self.settle();
        let slot = variant as usize;
        let read = self.scans[slot].as_ref().map(|scan| scan.reads);
        if read.is_none_or(|read| !read.includes(reads)) {
            let reads = read.map_or(reads, |read| read.and(reads));
            self.scans[slot] = Some(self.make_scan(variant, reads));
        }
        self.scans[slot].as_mut().expect("the scan was just made")
    }

    /// Scan the project again for `kinds`; every other kind is found again
    /// from the files as they are when a request asks for it.
    pub fn rescan(&mut self, kinds: Kinds) {
        self.settle();
        self.scans = [None, None];
        self.scan(self.variant(kinds), self.reads_for(kinds));
    }

    /// The embedder of the semantic kind, made on first use and kept. A
    /// failure is not kept: the model may be downloaded while the server
    /// runs.
    fn embedder(&mut self) -> Result<Arc<dyn Embedder>, String> {
        if let Some(embedder) = &self.embedder {
            return Ok(embedder.clone());
        }
        let embedder =
            cpd_semantic::embedder(&self.settings.semantic, &self.settings.run.paths, false)
                .map_err(|e| self.model_error(e))?;
        self.embedder = Some(embedder.clone());
        Ok(embedder)
    }

    /// `error`, from making an embedder, as a tool reports it: a model to
    /// download is the user's call, so the assistant is told to ask.
    fn model_error(&self, error: String) -> String {
        match cpd_semantic::missing_model(&self.settings.semantic) {
            Some(_) => format!("{error}. Ask the user before downloading the model"),
            None => error,
        }
    }

    fn semantic_params(&self) -> SemanticParams {
        SemanticParams {
            thresholds: self.settings.semantic.thresholds(),
            min_tokens: self.settings.run.min_tokens,
            min_lines: self.settings.run.min_lines,
            scope: self.settings.semantic.scope,
        }
    }

    /// The `--max-gap-lines` of a request: the configured value, which the
    /// scan applied already, or [`DEFAULT_GAP_LINES`] when `kinds` ask for
    /// gap clones and the options set none.
    fn gap_lines(&self, kinds: Kinds) -> usize {
        match self.settings.run.max_gap_lines {
            0 if kinds.gap => DEFAULT_GAP_LINES,
            configured => configured,
        }
    }

    fn ast_configured(&self) -> bool {
        self.settings.run.similarity_threshold().is_some()
    }

    fn ast_similarity(&self) -> f64 {
        self.settings
            .run
            .similarity_threshold()
            .unwrap_or(DEFAULT_AST_SIMILARITY)
    }

    /// The project's clones of `kinds`, biggest first. The same kinds get
    /// the same result until the next scan, except when a kind could not
    /// be looked for: it may be found next time.
    pub fn clones(&mut self, kinds: Kinds) -> Arc<Found> {
        let variant = self.variant(kinds);
        let reads = self.reads_for(kinds);
        let merge_gaps = kinds.gap && self.settings.run.max_gap_lines == 0;
        // Semantic clones leave out the ast pairs when ast runs, as with
        // `jscpd --similarity --semantic`.
        let with_ast = kinds.ast || self.ast_configured();
        let params = self.semantic_params();
        let embedder = kinds.semantic.then(|| self.embedder());
        let pool = self.pool.clone();
        let scan = self.scan(variant, reads);
        if let Some(found) = scan.found.get(&kinds) {
            return found.clone();
        }

        let mut clones: Vec<CpdClone> = scan.index.token_clones().cloned().collect();
        if merge_gaps {
            clones = merge_gapped_clones(clones, DEFAULT_GAP_LINES);
        }
        if kinds.ast {
            // A pair the token clones of this request cover is reported
            // once, as those clones: a copy merged across a gap, say, or a
            // third copy that detection pairs with the first copy only.
            let uncovered: Vec<CpdClone> = {
                let coverage = Coverage::new(&clones);
                scan.index
                    .similar_pairs()
                    .iter()
                    .filter(|pair| !coverage.covers_clone(pair))
                    .cloned()
                    .collect()
            };
            clones.extend(uncovered);
        }
        let mut unavailable = Vec::new();
        match embedder {
            Some(Ok(embedder)) => {
                match scan.semantic_pairs(&pool, embedder.as_ref(), &params, with_ast) {
                    Ok(pairs) => clones.extend(pairs.iter().cloned()),
                    Err(e) => unavailable.push(("semantic", e)),
                }
            }
            Some(Err(e)) => unavailable.push(("semantic", e)),
            None => {}
        }
        clones.retain(|clone| kinds.matches(clone));
        // A line that a token clone reports counts once, as in a report.
        discount_token_lines(&mut clones);
        // Biggest first, so every `limit` keeps the clones that matter
        // most; detection emits them in path order.
        clones.sort_by(|a, b| {
            b.token_count
                .cmp(&a.token_count)
                .then_with(|| a.position_key().cmp(&b.position_key()))
        });
        let found = Arc::new(Found {
            statistics: scan.index.statistics_of(&clones),
            files: scan.index.file_count(),
            clones,
            unavailable,
        });
        if found.unavailable.is_empty() {
            scan.found.insert(kinds, found.clone());
        }
        found
    }

    /// Compare `code`, a snippet in `format`, with the project: the clones
    /// of `kinds` between the two. `similarity` sets the threshold of ast
    /// matches and asks for them.
    pub fn check(
        &mut self,
        code: &str,
        format: &str,
        mut kinds: Kinds,
        similarity: Option<f64>,
    ) -> Result<Checked, String> {
        let format = self.resolve_format(format)?;
        let ast_threshold = match similarity {
            Some(ratio) => {
                kinds.ast = true;
                Some(ratio)
            }
            None => kinds.ast.then(|| self.ast_similarity()),
        };
        let variant = self.variant(kinds);
        let run = self.run_for(variant);
        let mut checked = Checked {
            format: format.clone(),
            kinds,
            matches: Vec::new(),
            note: None,
            unavailable: Vec::new(),
        };
        // The snippet skips the code `--ignore-pattern` matches, as the
        // project's files do, in its tokens and in its units.
        let code_ignore_regexes = compile_ignore_patterns(&run.code_ignore_patterns);
        let options = TokenizeOptions {
            mode: run.mode,
            ignore_case: run.ignore_case,
            ignore_identifiers: run.ignore_identifiers,
            ignore_literals: run.ignore_literals,
            ignore_annotations: run.ignore_annotations,
            ignore_ranges: code_ignore_ranges(code, &code_ignore_regexes),
            code_ignore_regexes,
            strip_types_formats: strip_types_formats(&run.cross_formats),
        };
        // The snippet split as a scan splits a file of its format: a
        // Markdown file or a component into its blocks (issue #1224), each a
        // source of its own, so detection, the merge across gaps and the ast
        // coverage keep them apart. Not tokenize_to_detection_maps for every
        // format: it would split a Razor view, which a scan reads as one
        // stream since #1222.
        let consistent = run.consistent_identifiers();
        let maps = match splits_blocks(&format) {
            true => tokenize_to_detection_maps(&format, code, &options),
            false => vec![TokenMap {
                format: format.clone(),
                tokens: tokenize_to_detection(&format, code, &options),
            }],
        };
        let component = is_component(&format);
        // The host's own stream comes first: the prose of a Markdown snippet,
        // the one stream of any other. A component has none, and its blocks
        // of code are its scripts; those of a Markdown snippet, its fences.
        let host = |i: usize, map: &TokenMap| i == 0 && map.format == format;
        let scripts: Vec<String> = match component {
            true => cpd_tokenizer::sfc::script_blocks(code, &format)
                .into_iter()
                .map(|(format, _)| format)
                .collect(),
            false => Vec::new(),
        };
        let is_code = |i: usize, map: &TokenMap| match component {
            true => scripts.contains(&map.format),
            false => !host(i, map),
        };
        let mut blocks: Vec<PreparedSource> = Vec::new();
        if component {
            // A scan reads a component in a fence of a Markdown file as one
            // stream of its format, so the whole snippet looks there too. It
            // goes first, so in a pool it shares with a block, as with
            // --cross-formats html,vue, a copy pairs with it whole.
            let tokens = tokenize_to_detection(&format, code, &options);
            blocks.push(snippet_source(
                SNIPPET_ID.to_string(),
                &format,
                &tokens,
                consistent,
                false,
            ));
        }
        for (i, map) in maps.iter().enumerate() {
            // Only a different language is embedded. A block is named after
            // its language, as a scan names it, and a fence in the host's
            // own, ```md in a Markdown snippet, after its place too.
            let embedded = map.format != format;
            let id = match (host(i, map), embedded) {
                (true, _) => SNIPPET_ID.to_string(),
                (false, true) => format!("{SNIPPET_ID}:{}", map.format),
                (false, false) => format!("{SNIPPET_ID}:{}:{i}", map.format),
            };
            blocks.push(snippet_source(
                id,
                &map.format,
                &map.tokens,
                consistent,
                embedded,
            ));
        }
        // A snippet too small for the token passes still has its units
        // compared: --similarity does not apply --min-tokens to them. The
        // note names the blocks of code too short for them, or the snippet's
        // own stream when no block holds code.
        let (min_tokens, min_lines) = (run.min_tokens, run.min_lines);
        let code_blocks: Vec<&TokenMap> = maps
            .iter()
            .enumerate()
            .filter(|&(i, map)| is_code(i, map) && !map.tokens.is_empty())
            .map(|(_, map)| map)
            .collect();
        let small: Vec<String> = match code_blocks.is_empty() {
            false => code_blocks
                .iter()
                .filter(|map| map.tokens.len() < min_tokens)
                .map(|map| format!("{} tokens of {}", map.tokens.len(), map.format))
                .collect(),
            true => {
                let tokens: usize = maps
                    .iter()
                    .enumerate()
                    .filter(|&(i, map)| host(i, map))
                    .map(|(_, map)| map.tokens.len())
                    .sum();
                match !component && tokens < min_tokens {
                    true => vec![format!("{tokens} tokens")],
                    false => Vec::new(),
                }
            }
        };
        let mut notes = Vec::new();
        if let Some((last, rest)) = small.split_last() {
            let small = match rest.is_empty() {
                true => last.clone(),
                false => format!("{} and {last}", rest.join(", ")),
            };
            notes.push(format!(
                "snippet has {small}, below the detection threshold of {min_tokens} (--min-tokens)"
            ));
        }
        if component && scripts.is_empty() {
            notes.push(format!(
                "the {format} snippet has no script, so only its markup and styles were compared"
            ));
        }
        checked.note = (!notes.is_empty()).then(|| notes.join("; "));
        // The blocks the token passes search and the semantic model reads, as
        // in a scan: a block shorter than --min-tokens matches nothing. The
        // snippet is no file of the project, so --max-lines, which skips a
        // file, does not apply to it.
        let detected: Vec<&PreparedSource> = blocks
            .iter()
            .filter(|block| block.hashes.len() >= min_tokens)
            .collect();
        let short = detected.is_empty();
        if short && ast_threshold.is_none() {
            return Ok(checked);
        }
        // The functions of those blocks, those of a component in its scripts,
        // read before anything is loaded or scanned for them: a snippet with
        // none needs neither the model nor the project's functions.
        let semantic = kinds.semantic && !short;
        let units = semantic && cpd_semantic::units::supports_units(&format);
        let queries = match units {
            true => {
                let reader = UnitReader::default();
                let sources: Vec<PassSource<'_>> = detected
                    .iter()
                    .map(|block| PassSource {
                        id: &block.id,
                        format: &block.format,
                        spans: &block.spans,
                    })
                    .collect();
                reader.read(&format, code, &sources);
                reader.take_sources()
            }
            false => Vec::new(),
        };
        let gap = self.gap_lines(kinds);
        let params = self.semantic_params();
        let embedder = (!queries.is_empty()).then(|| self.embedder());
        let pool = self.pool.clone();
        let reads = Reads {
            functions: ast_threshold.is_some(),
            units: !queries.is_empty(),
        };
        let scan = self.scan(variant, reads);

        // Exact and renamed matches, merged across gaps.
        let mut existing = Vec::new();
        if !short {
            // The blocks of one pool search it together, each under its own
            // id. Detection pairs a fragment with the first source that has
            // it, so the snippet goes first; two copies in the project that
            // also share the code around the snippet's still pair with each
            // other.
            let mut pools: BTreeMap<String, Vec<PreparedSource>> = BTreeMap::new();
            for block in &detected {
                pools
                    .entry(pool_key(&block.format, &run.cross_formats))
                    .or_default()
                    .push((*block).clone());
            }
            let searches: Vec<Vec<PreparedSource>> = pools
                .into_iter()
                .map(|(key, mut sources)| {
                    sources.extend(scan.index.pool_sources(&key));
                    sources
                })
                .collect();
            let found = pool.install(|| {
                detect_prepared(searches, min_tokens, min_lines, &PathFilters::default())
            });
            // A clone between the snippet and the project; one inside the
            // snippet matches nothing in the project.
            let found: Vec<CpdClone> = found
                .into_iter()
                .filter(|c| {
                    is_snippet(&c.fragment_a.source_id) != is_snippet(&c.fragment_b.source_id)
                })
                .collect();
            existing = merge_gapped_clones(found, gap);
            checked.matches.extend(existing.iter().map(|clone| Match {
                clone: clone.clone(),
                names: None,
            }));
        }

        // Functions with a similar structure.
        if let Some(threshold) = ast_threshold {
            let names = match run.blind_identifiers {
                true => cpd_similarity::Names::Blind,
                false => cpd_similarity::Names::Consistent,
            };
            if let Some(queries) =
                snippet_forms(code, &format, &blocks, &options.ignore_ranges, names)
            {
                let index = scan.functions(&run);
                for query in &queries {
                    for clone in index.query_clones(query, threshold, &existing) {
                        let names = names_of(&clone, |id, start| {
                            let sources = match is_snippet(id) {
                                true => queries.as_slice(),
                                false => index.sources(),
                            };
                            // By where it starts, since two units can share a
                            // line.
                            sources
                                .iter()
                                .filter(|s| s.id == id)
                                .flat_map(|s| &s.forms)
                                .find(|f| f.start.offset == start.offset)
                                .map(|f| f.name.clone())
                        });
                        existing.push(clone.clone());
                        checked.matches.push(Match { clone, names });
                    }
                }
            } else {
                checked.unavailable.push((
                    "ast",
                    format!(
                        "similar functions by syntax tree are found in {} snippets, and in the code blocks of markdown, vue, svelte and astro snippets",
                        cpd_similarity::supported_formats().join(", ")
                    ),
                ));
            }
        }

        // Functions that do the same job, searched together as the blocks of
        // one file.
        match embedder {
            Some(Ok(embedder)) => {
                let found = scan
                    .vectors(&pool, embedder.as_ref(), &params)
                    .map(|_| ())
                    .and_then(|()| {
                        let vectors = scan.vectors.as_ref().expect("the vectors were embedded");
                        pool.install(|| {
                            find_semantic_matches(
                                &queries,
                                &scan.units,
                                vectors,
                                embedder.as_ref(),
                                &params,
                                &existing,
                            )
                        })
                    });
                match found {
                    Ok(found) => {
                        for clone in found {
                            let names = names_of(&clone, |id, start| {
                                queries
                                    .iter()
                                    .chain(&scan.units)
                                    .filter(|s| s.id == id)
                                    .flat_map(|s| &s.units)
                                    .find(|u| u.start.offset == start.offset)
                                    .map(|u| u.name.clone())
                            });
                            checked.matches.push(Match { clone, names });
                        }
                    }
                    Err(e) => checked.unavailable.push(("semantic", e)),
                }
            }
            Some(Err(e)) => checked.unavailable.push(("semantic", e)),
            None if semantic && !units => checked.unavailable.push((
                "semantic",
                format!(
                    "the semantic model reads functions of {} snippets",
                    unit_formats().join(", ")
                ),
            )),
            None => {}
        }

        checked.matches.retain(|m| kinds.matches(&m.clone));
        // The surest evidence first: exact, renamed, similar, semantic; the
        // biggest or the most similar first within a kind.
        let rank = |kind: CloneKind| match kind {
            CloneKind::Exact => 0,
            CloneKind::Renamed => 1,
            CloneKind::Similar => 2,
            CloneKind::Semantic => 3,
        };
        checked.matches.sort_by(|a, b| {
            let (a, b) = (&a.clone, &b.clone);
            rank(a.kind)
                .cmp(&rank(b.kind))
                .then_with(|| {
                    b.similarity
                        .unwrap_or(1.0)
                        .total_cmp(&a.similarity.unwrap_or(1.0))
                })
                .then_with(|| b.token_count.cmp(&a.token_count))
                .then_with(|| a.position_key().cmp(&b.position_key()))
        });
        Ok(checked)
    }

    /// Compare two folders of the scanned ones function by function, as
    /// `--compare` does: the JSON report of `--compare`. A relative path is
    /// taken from the scanned folder that holds it.
    pub fn compare(&self, left: &str, right: &str) -> Result<Value, String> {
        let left = self.resolve_path(left)?;
        let right = self.resolve_path(right)?;
        crate::compare::side_roots([&left, &right])?;
        let embedder = cpd_semantic::embedder(
            &self.settings.semantic,
            &[left.clone(), right.clone()],
            false,
        )
        .map_err(|e| self.model_error(e))?;
        let settings = crate::compare::Settings {
            semantic: &self.settings.semantic,
            min_tokens: self.settings.compare_min_tokens,
            min_lines: self.settings.run.min_lines,
            workers: self.settings.run.workers,
        };
        let compared = crate::compare::compare_folders(
            [&left, &right],
            embedder.as_ref(),
            &settings,
            &self.settings.run,
        )?;
        serde_json::to_value(&compared.report).map_err(|e| e.to_string())
    }

    /// The model compare_folders and the semantic kind run.
    pub fn model(&self) -> &str {
        &self.settings.semantic.model
    }

    /// `path` as a tool gives it, absolute or relative to a scanned folder,
    /// when it lies inside the scanned folders: the tools read nothing else.
    fn resolve_path(&self, path: &str) -> Result<PathBuf, String> {
        let given = Path::new(path);
        let found = match given.is_absolute() {
            true => given.exists().then(|| given.to_path_buf()),
            false => self
                .settings
                .run
                .paths
                .iter()
                .map(|root| root.join(given))
                .find(|p| p.exists()),
        };
        let roots = || {
            self.roots
                .iter()
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let Some(found) = found else {
            return Err(format!(
                "'{path}' does not exist: give a folder inside the scanned folders ({}), relative to one of them or absolute",
                roots()
            ));
        };
        let canonical = std::fs::canonicalize(&found).unwrap_or_else(|_| found.clone());
        if !self.roots.iter().any(|root| canonical.starts_with(root)) {
            return Err(format!(
                "'{path}' is outside the scanned folders ({}): the server compares folders it scans; to compare two projects, start it with both",
                roots()
            ));
        }
        Ok(found)
    }

    /// A format argument: a format name passes through, a file extension
    /// ("js", "py") resolves to its format ("javascript", "python").
    pub fn resolve_format(&self, format: &str) -> Result<String, String> {
        if cpd_tokenizer::formats::list_formats().contains(&format)
            || self.settings.run.formats_exts.contains_key(format)
        {
            return Ok(format.to_string());
        }
        cpd_tokenizer::formats::get_format_by_extension(format)
            .map(str::to_string)
            .ok_or_else(|| {
                format!("unknown format '{format}': run `jscpd --list` for supported formats")
            })
    }

    /// A source id as results show it: the file an embedded block is in,
    /// relative to the scan root holding it, with `/` between folders.
    pub fn display_path(&self, id: &str) -> String {
        let file = Path::new(host_file(id));
        for root in &self.roots {
            if let Ok(relative) = file.strip_prefix(root) {
                return relative.to_string_lossy().replace('\\', "/");
            }
        }
        file.to_string_lossy().replace('\\', "/")
    }

    /// Whether a file that `path` names, relative as results show it or
    /// absolute, is part of the scan `kinds` read.
    pub fn scanned(&mut self, kinds: Kinds, path: &str) -> bool {
        let variant = self.variant(kinds);
        let reads = self.reads_for(kinds);
        let absolute = std::fs::canonicalize(path)
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        let ids: Vec<String> = self
            .scan(variant, reads)
            .index
            .files()
            .map(|(id, ..)| id.to_string())
            .collect();
        ids.iter()
            .any(|id| self.display_path(id) == path || Some(id) == absolute.as_ref())
    }
}

/// The snippet's block `tokens` of `format` as a source for detection, under
/// `id`; `embedded` when it is code of another language in the snippet.
fn snippet_source(
    id: String,
    format: &str,
    tokens: &[DetectionToken],
    consistent: bool,
    embedded: bool,
) -> PreparedSource {
    let mut source = PreparedSource::from_detection_tokens(id, format.to_string(), tokens);
    if consistent {
        source.track_renames(tokens);
    }
    source.embedded = embedded;
    source
}

/// Whether `format` is that of a component: Vue, Svelte or Astro.
fn is_component(format: &str) -> bool {
    matches!(format, "vue" | "svelte" | "astro")
}

/// The units of a snippet, with the detection tokens each holds: its own
/// functions in a language with units, or those of its code blocks, each
/// with the id and the tokens of its block, when it is a Markdown file or a
/// component. `None` when the snippet's format has none to compare. Their
/// names compare by `names`.
fn snippet_forms(
    code: &str,
    format: &str,
    blocks: &[PreparedSource],
    ignored: &[[usize; 2]],
    names: cpd_similarity::Names,
) -> Option<Vec<FormSource>> {
    let mut by_format: HashMap<String, Vec<cpd_similarity::Form>> = HashMap::new();
    if cpd_similarity::supports(format) {
        // A language with units is no file of blocks: the snippet is its one
        // block.
        by_format.insert(
            format.to_string(),
            cpd_similarity::forms(code, format, ignored, names),
        );
    } else if cpd_similarity::embeds(format) {
        for (block, form) in cpd_similarity::embedded_forms(code, format, ignored, names) {
            by_format.entry(block).or_default().push(form);
        }
    } else {
        return None;
    }
    Some(
        blocks
            .iter()
            .filter_map(|block| Some(FormSource::new(block, by_format.remove(&block.format)?)))
            .collect(),
    )
}

/// The names of the two functions of a snippet match: the project's, then
/// the snippet's, by `name(source id, start)`.
fn names_of(
    clone: &CpdClone,
    name: impl Fn(&str, &Location) -> Option<String>,
) -> Option<(String, String)> {
    let (snippet, file, _) = sides_of(clone);
    Some((
        name(&file.source_id, &file.start)?,
        name(&snippet.source_id, &snippet.start)?,
    ))
}

/// The formats the semantic model reads functions of.
fn unit_formats() -> Vec<&'static str> {
    let mut formats: Vec<&'static str> = cpd_tokenizer::formats::list_formats()
        .into_iter()
        .filter(|f| cpd_semantic::units::supports_units(f))
        .collect();
    formats.sort_unstable();
    formats
}
