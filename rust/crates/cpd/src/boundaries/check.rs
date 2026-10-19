//! The check itself: every copy of a rule's protected code in a place the
//! rule keeps it out of, found by every method jscpd has.
//!
//! - Token clones, exact or renamed, and the merged ones of `--max-gap-lines`:
//!   copy-paste between two files of one language.
//! - Functions of the same structure (`--similarity`): a copy that was
//!   edited.
//! - Functions that do the same job (`--semantic`, the pairing of
//!   `--compare`): the logic written again, often in another language.
//!
//! The search runs on its own, with the bars of each rule, whatever the
//! clone report of the run asks for: `--kind`, `--skip-local` and
//! `--skip-isolated` narrow that report and leave the rules alone, and test
//! files count.

use super::rules::{Kinds, Methods, Protection, Rule, Rules, Thresholds};
use crate::options::Options;
use cpd_core::boundaries::{BoundaryReport, Place, RuleReport, Violation};
use cpd_core::detect::{PathFilters, PreparedSource, detect_prepared, merge_gapped_clones};
use cpd_core::models::{CloneKind, CpdClone, Fragment, SimilarityMethod};
use cpd_finder::orchestrate::{
    FilePreparer, PreparedText, RunConfig, build_thread_pool, host_file, pool_key, walk_config,
};
use cpd_finder::walker::{DiscoveredFile, walk};
use cpd_semantic::compare::{CompareParams, compare};
use cpd_semantic::search::{Embedder, SemanticUnit, UnitSource};
use cpd_semantic::{SemanticOptions, UnitReader};
use cpd_similarity::{FormSource, find_similar};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

/// The rules of a config file with the model they compare functions with.
pub struct Checker {
    rules: Rules,
    semantic: SemanticOptions,
    /// `None` when there is no rule, and so nothing to embed, or when the
    /// local model is not downloaded: the rules then look for copies
    /// without it.
    embedder: Option<Arc<dyn Embedder>>,
    quiet: bool,
}

impl Checker {
    /// The rules of `setting`, checked, and the model, started. A rule that
    /// is wrong stops the run before it scans anything. A local model that
    /// is not downloaded only leaves the semantic search out, with a
    /// warning; any other problem with the model stops the run.
    pub fn new(setting: &super::Setting, paths: &[PathBuf], quiet: bool) -> Result<Self, String> {
        let rules = Rules::parse(&setting.raw, &setting.base)?;
        let semantic = &setting.semantic;
        for (what, value) in [
            ("semantic threshold", Some(semantic.threshold)),
            ("semantic same threshold", semantic.same_threshold),
        ] {
            if let Some(value) = value
                && !(value > 0.0 && value <= 1.0)
            {
                return Err(format!(
                    "the {what} is {value}, it must be a number in (0, 1]"
                ));
            }
        }
        // A rule that disables `semantic` needs no model.
        let embedder = if !rules.rules.iter().any(|rule| rule.kinds.semantic()) {
            None
        } else if let Some((id, bytes)) = cpd_semantic::missing_model(semantic) {
            // Even under --silent: the check finds less without the model,
            // and a CI log is where that has to show.
            eprintln!("{}", missing_model_warning(&semantic.model, &id, bytes));
            None
        } else {
            Some(cpd_semantic::embedder(semantic, paths, quiet)?)
        };
        Ok(Self {
            rules,
            semantic: setting.semantic.clone(),
            embedder,
            quiet,
        })
    }

    /// Check every rule against the files a scan with `run_config` walks.
    /// Fails only when the model does.
    pub fn check(&self, opts: &Options, run_config: &RunConfig) -> Result<BoundaryReport, String> {
        let rules = &self.rules.rules;
        if rules.is_empty() {
            return Ok(BoundaryReport::default());
        }
        let pool = build_thread_pool(opts.workers);
        let files = scan_files(run_config);
        let mut guards: Vec<Guard> = rules
            .iter()
            .map(|rule| Guard::new(rule, &self.rules, &files))
            .collect();
        let wanted: Vec<&DiscoveredFile> = files
            .iter()
            .filter(|file| {
                let id = source_id(file);
                guards.iter().any(|guard| guard.looks_at(&id))
            })
            .collect();
        let ast = rules.iter().any(|rule| rule.kinds.ast());
        let scan = Scan::read(&pool, run_config, &wanted, ast);
        for guard in &mut guards {
            guard.find_methods(&scan.units);
        }
        // Even under --silent: a rule with a typo in a path or a method
        // name checks nothing, and a CI log is where that has to show.
        for (rule, guard) in rules.iter().zip(&guards) {
            if !guard.protects_anything() {
                eprintln!(
                    "Warning: boundaries: rule '{}' protects no code of this scan; check the paths and method names of its code, and the scan paths",
                    rule.name
                );
            }
        }
        let bars: Vec<Bars> = rules
            .iter()
            .map(|rule| Bars::new(&rule.thresholds, opts, &self.semantic))
            .collect();

        let mut found: Vec<Vec<Violation>> = vec![Vec::new(); rules.len()];
        self.find_copies(&pool, opts, run_config, &scan, &guards, &bars, &mut found);
        self.find_rewrites(&pool, &scan, &guards, &bars, &mut found)?;

        let names = Names::new(&scan.units);
        Ok(BoundaryReport {
            rules: rules
                .iter()
                .zip(found)
                .map(|(rule, violations)| RuleReport {
                    name: rule.name.clone(),
                    violations: settle(violations, &names),
                })
                .collect(),
        })
    }

    /// Token clones and functions of the same structure, by the bars of the
    /// rules; rules with the same bars share one search.
    #[allow(clippy::too_many_arguments)]
    fn find_copies(
        &self,
        pool: &rayon::ThreadPool,
        opts: &Options,
        run_config: &RunConfig,
        scan: &Scan,
        guards: &[Guard],
        bars: &[Bars],
        found: &mut [Vec<Violation>],
    ) {
        let rules = &self.rules.rules;
        let mut groups: BTreeMap<(usize, usize, u32, u64), Vec<usize>> = BTreeMap::new();
        for (index, bar) in bars.iter().enumerate() {
            groups.entry(bar.key()).or_default().push(index);
        }
        let filters = PathFilters::default();
        for members in groups.values() {
            let bar = &bars[members[0]];
            let wants = |kind: fn(&Kinds) -> bool| members.iter().any(|&i| kind(&rules[i].kinds));
            let looked: HashSet<&str> = members.iter().flat_map(|&i| guards[i].files()).collect();
            let home: HashSet<&str> = members
                .iter()
                .flat_map(|&i| guards[i].home.keys().map(String::as_str))
                .collect();
            let clones = match wants(Kinds::tokens) {
                true => {
                    let mut pools: BTreeMap<String, Vec<PreparedSource>> = BTreeMap::new();
                    for source in scan
                        .prepared
                        .iter()
                        .filter(|s| looked.contains(host_file(&s.id)))
                    {
                        pools
                            .entry(pool_key(&source.format, &run_config.cross_formats))
                            .or_default()
                            .push(source.clone());
                    }
                    // Detection pairs every later copy of a fragment with
                    // the first one it saw. With the protected files first, a
                    // copy outside the boundary pairs with the protected
                    // code, not with another copy.
                    let outside = |source: &PreparedSource| !home.contains(host_file(&source.id));
                    for sources in pools.values_mut() {
                        sources.sort_by(|a, b| (outside(a), &a.id).cmp(&(outside(b), &b.id)));
                    }
                    pool.install(|| {
                        detect_prepared(
                            pools.into_values().collect(),
                            bar.min_tokens,
                            bar.min_lines,
                            &filters,
                        )
                    })
                }
                false => Vec::new(),
            };
            // A merge replaces the exact parts it joins, so a rule that
            // disables `gap` keeps the parts.
            let merged = (opts.max_gap_lines > 0 && wants(Kinds::gap))
                .then(|| merge_gapped_clones(clones.clone(), opts.max_gap_lines));
            let similar = match wants(Kinds::ast) {
                true => {
                    let forms: Vec<FormSource> = scan
                        .forms
                        .iter()
                        .filter(|f| looked.contains(host_file(&f.id)))
                        .cloned()
                        .collect();
                    // Every pair, not only the one that links each function
                    // to its closest look-alike, which may be another copy
                    // outside.
                    pool.install(|| {
                        find_similar(
                            forms,
                            bar.similarity,
                            bar.min_nodes,
                            bar.min_lines as u32,
                            &clones,
                            &filters,
                            true,
                        )
                        .all
                    })
                }
                false => Vec::new(),
            };
            for &index in members {
                let (guard, kinds) = (&guards[index], &rules[index].kinds);
                let tokens = match (&merged, kinds.gap()) {
                    (Some(merged), true) => merged,
                    _ => &clones,
                };
                found[index].extend(
                    tokens
                        .iter()
                        .chain(&similar)
                        .filter(|clone| kinds.takes(clone.kind, clone.similarity_method))
                        .filter_map(|clone| guard.violation(clone)),
                );
            }
        }
    }

    /// Functions outside a boundary that do what protected ones do, paired
    /// as `--compare` pairs two codebases. Every function of every rule is
    /// embedded in one call, so the model's cache keeps them all.
    fn find_rewrites(
        &self,
        pool: &rayon::ThreadPool,
        scan: &Scan,
        guards: &[Guard],
        bars: &[Bars],
        found: &mut [Vec<Violation>],
    ) -> Result<(), String> {
        let Some(embedder) = &self.embedder else {
            return Ok(());
        };
        let sides: Vec<Option<[Vec<UnitSource>; 2]>> = guards
            .iter()
            .zip(&self.rules.rules)
            .map(|(guard, rule)| match rule.kinds.semantic() {
                true => guard.sides(&scan.units),
                false => None,
            })
            .collect();
        let mut texts: Vec<&str> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        let (mut inside, mut outside) = (0, 0);
        for [protected, elsewhere] in sides.iter().flatten() {
            for (side, count) in [(protected, &mut inside), (elsewhere, &mut outside)] {
                // The pairing leaves anonymous functions out, as pieces of
                // the function around them.
                let named = side
                    .iter()
                    .flat_map(|s| &s.units)
                    .filter(|u| !u.name.starts_with('<'));
                for unit in named {
                    *count += 1;
                    if seen.insert(unit.text.as_str()) {
                        texts.push(unit.text.as_str());
                    }
                }
            }
        }
        if texts.is_empty() {
            return Ok(());
        }
        if !self.quiet {
            eprintln!(
                "Boundaries: comparing {inside} protected functions with {outside} functions outside their boundaries"
            );
        }
        let vectors = embedder.embed(&texts)?;
        let embedded = Embedded::new(&texts, vectors);
        for (index, sides) in sides.iter().enumerate() {
            let Some([protected, elsewhere]) = sides else {
                continue;
            };
            let params = CompareParams {
                thresholds: bars[index].semantic,
                min_tokens: bars[index].min_tokens,
                min_lines: bars[index].min_lines,
            };
            let comparison = pool.install(|| {
                compare(
                    [protected.as_slice(), elsewhere.as_slice()],
                    &embedded,
                    &params,
                )
            })?;
            for pair in &comparison.pairs {
                let (a, b) = (comparison.functions[pair.a], comparison.functions[pair.b]);
                let (from, to) = (&protected[a.source], &elsewhere[b.source]);
                let (original, copy) = (&from.units[a.unit], &to.units[b.unit]);
                found[index].push(Violation {
                    kind: CloneKind::Semantic,
                    method: None,
                    similarity: Some(pair.similarity),
                    tokens: original.token_count.min(copy.token_count),
                    copy: unit_place(&to.id, copy),
                    protected: unit_place(&from.id, original),
                });
            }
        }
        Ok(())
    }
}

/// The warning of a run whose local model `name` (`id`, of `bytes` bytes)
/// is not downloaded: what the rules leave out, and how to fetch it.
fn missing_model_warning(name: &str, id: &str, bytes: u64) -> String {
    let download = match name == cpd_semantic::DEFAULT_LOCAL_MODEL {
        true => "jscpd --semantic-download".to_string(),
        false => format!("jscpd --semantic-download {name}"),
    };
    format!(
        "Warning: boundaries: the model {id} is not downloaded, so the rules look for copies without it and miss logic written again in another language. Run `{download}` once ({:.0} MB), or use an embeddings API with --semantic-url",
        bytes as f64 / 1e6
    )
}

/// The files a scan with `run_config` walks, in path order.
fn scan_files(run_config: &RunConfig) -> Vec<DiscoveredFile> {
    let mut files = walk(&walk_config(run_config));
    files.retain(|f| !run_config.skip_files.contains(&f.real_path));
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

/// The source id a scan gives the file: its path as the walk found it.
fn source_id(file: &DiscoveredFile) -> String {
    file.path.to_string_lossy().into_owned()
}

/// What the files the rules look at hold, read once for every rule.
struct Scan {
    /// Detection-ready sources, as a clone run prepares them.
    prepared: Vec<PreparedSource>,
    /// The units `--similarity` compares, tests included.
    forms: Vec<FormSource>,
    /// The functions the model compares and the reports name, every one
    /// counted as code: a copy in a test is a copy.
    units: Vec<UnitSource>,
}

impl Scan {
    /// The files `files`, prepared as a clone run prepares them, with the
    /// units of `--similarity` when `forms` asks for them.
    fn read(
        pool: &rayon::ThreadPool,
        run_config: &RunConfig,
        files: &[&DiscoveredFile],
        forms: bool,
    ) -> Self {
        let reader = Arc::new(UnitReader::default());
        let config = RunConfig {
            // Every file is read, however small: the bars of each rule
            // apply to what is found in them.
            min_tokens: 0,
            min_lines: 0,
            similarity: forms.then_some(cpd_similarity::DEFAULT_THRESHOLD),
            similarity_tests: true,
            passes: vec![reader.clone()],
            ..run_config.clone()
        };
        let preparer = FilePreparer::new(&config);
        let texts: Vec<PreparedText> = pool.install(|| {
            files
                .par_iter()
                .filter_map(|file| {
                    let content = std::fs::read_to_string(&file.real_path).ok()?;
                    let real_path = match file.real_path == file.path {
                        true => String::new(),
                        false => file.real_path.to_string_lossy().into_owned(),
                    };
                    preparer.prepare(source_id(file), real_path, &file.format, &content)
                })
                .collect()
        });
        let mut units = reader.take_sources();
        for unit in units.iter_mut().flat_map(|source| &mut source.units) {
            unit.test = false;
        }
        let (mut prepared, mut forms) = (Vec::new(), Vec::new());
        for text in texts {
            prepared.extend(text.prepared);
            forms.extend(text.functions);
        }
        Self {
            prepared,
            forms,
            units,
        }
    }
}

/// A rule's place for each file of the scan, by source id.
struct Guard<'r> {
    /// The files the rule protects, all of each or some of its methods.
    home: HashMap<String, Home<'r>>,
    /// The files a copy must not be in.
    forbidden: HashSet<String>,
}

enum Home<'r> {
    Whole,
    /// The methods the rule picks in the file, and, once the file is read,
    /// the lines they span.
    Methods {
        picks: Vec<&'r Methods>,
        lines: Vec<(u32, u32)>,
    },
}

impl<'r> Guard<'r> {
    fn new(rule: &'r Rule, rules: &Rules, files: &[DiscoveredFile]) -> Self {
        let mut home = HashMap::new();
        let mut forbidden = HashSet::new();
        for file in files {
            let location = rules.locate(&file.path);
            match rule.protects(&location) {
                Some(Protection::Whole) => {
                    home.insert(source_id(file), Home::Whole);
                }
                Some(Protection::Methods(picks)) => {
                    home.insert(
                        source_id(file),
                        Home::Methods {
                            picks,
                            lines: Vec::new(),
                        },
                    );
                }
                None if rule.forbids(&location) => {
                    forbidden.insert(source_id(file));
                }
                None => {}
            }
        }
        Self { home, forbidden }
    }

    /// Whether the rule needs the file: one it protects or one it keeps
    /// copies out of, when both kinds exist.
    fn looks_at(&self, id: &str) -> bool {
        !self.home.is_empty()
            && !self.forbidden.is_empty()
            && (self.home.contains_key(id) || self.forbidden.contains(id))
    }

    /// The files the rule needs.
    fn files(&self) -> impl Iterator<Item = &str> {
        let both = !self.home.is_empty() && !self.forbidden.is_empty();
        self.home
            .keys()
            .chain(&self.forbidden)
            .filter(move |_| both)
            .map(String::as_str)
    }

    /// Whether the rule protects some code: a file, or a method it names.
    /// A method of a file the rules do not read, for want of files to keep
    /// its copies out of, counts as found.
    fn protects_anything(&self) -> bool {
        let read = !self.forbidden.is_empty();
        self.home.values().any(|home| match home {
            Home::Whole => true,
            Home::Methods { lines, .. } => !read || !lines.is_empty(),
        })
    }

    /// The lines of the methods the rule picks, now that the files are read.
    fn find_methods(&mut self, units: &[UnitSource]) {
        for source in units {
            if let Some(Home::Methods { picks, lines }) = self.home.get_mut(host_file(&source.id)) {
                lines.extend(
                    source
                        .units
                        .iter()
                        .filter(|unit| picks.iter().any(|pick| pick.matches(&unit.name)))
                        .map(|unit| (unit.start.line, unit.end.line)),
                );
            }
        }
    }

    fn protects(&self, fragment: &Fragment) -> bool {
        match self.home.get(host_file(&fragment.source_id)) {
            Some(Home::Whole) => true,
            Some(Home::Methods { lines, .. }) => lines
                .iter()
                .any(|&(from, to)| fragment.start.line <= to && fragment.end.line >= from),
            None => false,
        }
    }

    fn forbids(&self, fragment: &Fragment) -> bool {
        self.forbidden.contains(host_file(&fragment.source_id))
    }

    /// The violation `clone` is, if one side of it is protected code and
    /// the other a place the rule keeps the code out of.
    fn violation(&self, clone: &CpdClone) -> Option<Violation> {
        let (a, b) = (&clone.fragment_a, &clone.fragment_b);
        let (protected, copy) = if self.protects(a) && self.forbids(b) {
            (a, b)
        } else if self.protects(b) && self.forbids(a) {
            (b, a)
        } else {
            return None;
        };
        Some(Violation {
            kind: clone.kind,
            method: clone.similarity_method,
            similarity: clone.similarity,
            tokens: clone.token_count,
            copy: Place {
                fragment: copy.clone(),
                function: None,
            },
            protected: Place {
                fragment: protected.clone(),
                function: None,
            },
        })
    }

    /// The functions the model compares for the rule: the protected ones,
    /// and every function of the places it keeps them out of. `None` when
    /// either side has none.
    fn sides(&self, units: &[UnitSource]) -> Option<[Vec<UnitSource>; 2]> {
        let (mut protected, mut elsewhere) = (Vec::new(), Vec::new());
        for source in units {
            let host = host_file(&source.id);
            if let Some(home) = self.home.get(host) {
                let picked: Vec<SemanticUnit> = source
                    .units
                    .iter()
                    .filter(|unit| match home {
                        Home::Whole => true,
                        Home::Methods { picks, .. } => {
                            picks.iter().any(|pick| pick.matches(&unit.name))
                        }
                    })
                    .cloned()
                    .collect();
                if !picked.is_empty() {
                    protected.push(UnitSource {
                        units: picked,
                        ..source.clone()
                    });
                }
            } else if self.forbidden.contains(host) {
                elsewhere.push(source.clone());
            }
        }
        (!protected.is_empty() && !elsewhere.is_empty()).then_some([protected, elsewhere])
    }
}

/// The bars of one rule: its own thresholds, else the project's.
struct Bars {
    min_tokens: usize,
    min_lines: usize,
    min_nodes: u32,
    similarity: f64,
    semantic: cpd_semantic::Thresholds,
}

impl Bars {
    fn new(own: &Thresholds, opts: &Options, semantic: &SemanticOptions) -> Self {
        let across = own.semantic_threshold.unwrap_or(semantic.threshold);
        let within = own.semantic_same_threshold.or(semantic.same_threshold);
        Self {
            min_tokens: own.min_tokens.unwrap_or(opts.min_tokens),
            min_lines: own.min_lines.unwrap_or(opts.min_lines),
            min_nodes: own.min_nodes.unwrap_or(opts.min_nodes),
            // The search runs for the rules whether the clone report asks
            // for it or not, at the ratio the report would use.
            similarity: own
                .similarity
                .or(opts.similarity)
                .unwrap_or(cpd_similarity::DEFAULT_THRESHOLD),
            semantic: cpd_semantic::embed::catalog::thresholds(
                &semantic.model,
                Some(across),
                within,
            ),
        }
    }

    /// What the token and structure searches depend on.
    fn key(&self) -> (usize, usize, u32, u64) {
        (
            self.min_tokens,
            self.min_lines,
            self.min_nodes,
            self.similarity.to_bits(),
        )
    }
}

/// The vectors of one embedding call, looked up by text, for the pairing of
/// each rule.
struct Embedded {
    index: HashMap<String, usize>,
    vectors: Vec<Vec<f32>>,
}

impl Embedded {
    fn new(texts: &[&str], vectors: Vec<Vec<f32>>) -> Self {
        Self {
            index: texts
                .iter()
                .enumerate()
                .map(|(i, text)| (text.to_string(), i))
                .collect(),
            vectors,
        }
    }
}

impl Embedder for Embedded {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        texts
            .iter()
            .map(|text| {
                self.index
                    .get(*text)
                    .and_then(|&i| self.vectors.get(i).cloned())
                    .ok_or_else(|| "a function was compared before it was embedded".to_string())
            })
            .collect()
    }
}

/// A function, as one side of a violation.
fn unit_place(source_id: &str, unit: &SemanticUnit) -> Place {
    Place {
        fragment: Fragment::new(source_id, unit.start.clone(), unit.end.clone(), unit.range),
        function: named(&unit.name),
    }
}

/// A function's name; `None` for an anonymous one (`<arrow>`).
fn named(name: &str) -> Option<String> {
    (!name.starts_with('<')).then(|| name.to_string())
}

/// The named functions of every file, to name the function a fragment lies
/// in.
struct Names<'u> {
    by_file: HashMap<&'u str, Vec<&'u SemanticUnit>>,
}

impl<'u> Names<'u> {
    fn new(units: &'u [UnitSource]) -> Self {
        let mut by_file: HashMap<&str, Vec<&SemanticUnit>> = HashMap::new();
        for source in units {
            by_file
                .entry(host_file(&source.id))
                .or_default()
                .extend(source.units.iter().filter(|u| !u.name.starts_with('<')));
        }
        Self { by_file }
    }

    /// The named function that holds most of the lines of `fragment`, the
    /// innermost of equals. A token clone can start on the last line of the
    /// function before the one it copies, and a callback named after its
    /// variable (`const total = items.reduce(...)`) lies inside the function
    /// that holds the copy.
    fn of(&self, fragment: &Fragment) -> Option<String> {
        let (from, to) = (fragment.start.line, fragment.end.line);
        let shared = |unit: &SemanticUnit| {
            (to.min(unit.end.line) + 1).saturating_sub(from.max(unit.start.line))
        };
        self.by_file
            .get(host_file(&fragment.source_id))?
            .iter()
            .filter(|unit| shared(unit) > 0)
            .max_by_key(|unit| {
                (
                    shared(unit),
                    std::cmp::Reverse(unit.end.line - unit.start.line),
                )
            })
            .map(|unit| unit.name.clone())
    }
}

/// How sure a kind of match is, the surest first: a copy that two methods
/// find is reported once, as the surer one found it.
fn rank(violation: &Violation) -> u8 {
    match (violation.kind, violation.method) {
        (CloneKind::Exact, _) => 0,
        (CloneKind::Renamed, _) => 1,
        (CloneKind::Similar, Some(SimilarityMethod::Gap)) => 2,
        (CloneKind::Similar, _) => 3,
        (CloneKind::Semantic, _) => 4,
    }
}

/// One violation per copy: of the matches whose copies overlap, the surest,
/// then the longest, then the closest. Named, in the order of the copies.
fn settle(mut found: Vec<Violation>, names: &Names) -> Vec<Violation> {
    found.sort_by(|x, y| {
        rank(x)
            .cmp(&rank(y))
            .then(y.copy_lines().cmp(&x.copy_lines()))
            .then(
                y.similarity
                    .unwrap_or(1.0)
                    .total_cmp(&x.similarity.unwrap_or(1.0)),
            )
            .then_with(|| position(x).cmp(&position(y)))
    });
    let mut kept: Vec<Violation> = Vec::with_capacity(found.len());
    for violation in found {
        if !kept.iter().any(|other| same_copy(other, &violation)) {
            kept.push(violation);
        }
    }
    for violation in &mut kept {
        for place in [&mut violation.copy, &mut violation.protected] {
            if place.function.is_none() {
                place.function = names.of(&place.fragment);
            }
        }
    }
    kept.sort_by(|x, y| position(x).cmp(&position(y)));
    kept
}

/// Where a violation is: its copy, then its protected code.
fn position(violation: &Violation) -> (&str, u32, &str, u32) {
    (
        host_file(&violation.copy.fragment.source_id),
        violation.copy.fragment.start.line,
        host_file(&violation.protected.fragment.source_id),
        violation.protected.fragment.start.line,
    )
}

/// Whether two violations are about lines of one copy.
fn same_copy(x: &Violation, y: &Violation) -> bool {
    let (a, b) = (&x.copy.fragment, &y.copy.fragment);
    host_file(&a.source_id) == host_file(&b.source_id)
        && a.start.line <= b.end.line
        && b.start.line <= a.end.line
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::Location;

    fn place(file: &str, from: u32, to: u32) -> Place {
        Place {
            fragment: Fragment::new(
                file,
                Location::new(from, 0, 0),
                Location::new(to, 0, 0),
                [0, 0],
            ),
            function: None,
        }
    }

    fn violation(kind: CloneKind, copy: (&str, u32, u32), similarity: Option<f32>) -> Violation {
        Violation {
            kind,
            method: (kind == CloneKind::Similar).then_some(SimilarityMethod::Ast),
            similarity,
            tokens: 50,
            copy: place(copy.0, copy.1, copy.2),
            protected: place("/p/backend/Fees.kt", 10, 30),
        }
    }

    #[test]
    fn a_copy_two_methods_find_is_reported_once_as_the_surer_one() {
        let names = Names::new(&[]);
        let settled = settle(
            vec![
                violation(CloneKind::Semantic, ("/p/web/cart.ts", 5, 40), Some(0.7)),
                violation(CloneKind::Similar, ("/p/web/cart.ts", 5, 40), Some(0.9)),
                violation(CloneKind::Exact, ("/p/web/cart.ts", 12, 30), None),
                violation(CloneKind::Exact, ("/p/web/tax.ts", 1, 20), None),
            ],
            &names,
        );
        let kinds: Vec<(&str, CloneKind)> = settled
            .iter()
            .map(|v| (v.copy.fragment.source_id.as_str(), v.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("/p/web/cart.ts", CloneKind::Exact),
                ("/p/web/tax.ts", CloneKind::Exact)
            ]
        );
    }

    #[test]
    fn copies_in_other_lines_of_one_file_are_two_violations() {
        let settled = settle(
            vec![
                violation(CloneKind::Exact, ("/p/web/cart.ts", 40, 60), None),
                violation(CloneKind::Exact, ("/p/web/cart.ts", 1, 20), None),
            ],
            &Names::new(&[]),
        );
        let lines: Vec<u32> = settled.iter().map(|v| v.copy.fragment.start.line).collect();
        assert_eq!(lines, vec![1, 40]);
    }

    #[test]
    fn a_fragment_is_named_after_the_function_that_holds_most_of_it() {
        let unit = |name: &str, from: u32, to: u32| SemanticUnit {
            grammar: "oxc",
            name: name.to_string(),
            start: Location::new(from, 0, 0),
            end: Location::new(to, 0, 0),
            range: [0, 0],
            token_count: 10,
            text: name.to_string(),
            test: false,
        };
        let units = vec![UnitSource {
            id: "/p/web/cart.ts".to_string(),
            format: "typescript".to_string(),
            units: vec![
                unit("checkout", 1, 50),
                unit("total", 10, 20),
                unit("<arrow>", 12, 14),
                unit("badge", 52, 60),
                unit("discount", 62, 70),
                unit("subtotal", 64, 64),
            ],
            path_label: Default::default(),
        }];
        let names = Names::new(&units);
        let name = |file: &str, from: u32, to: u32| names.of(&place(file, from, to).fragment);
        // Inside both: the inner one.
        assert_eq!(name("/p/web/cart.ts", 13, 18).as_deref(), Some("total"));
        assert_eq!(name("/p/web/cart.ts", 30, 40).as_deref(), Some("checkout"));
        // Mostly in the outer one.
        assert_eq!(name("/p/web/cart.ts", 19, 30).as_deref(), Some("checkout"));
        // A clone that starts on the last line of the function before.
        assert_eq!(name("/p/web/cart.ts", 59, 70).as_deref(), Some("discount"));
        assert_eq!(name("/p/web/cart.ts", 80, 90), None);
        assert_eq!(name("/p/web/other.ts", 13, 18), None);
    }

    #[test]
    fn the_warning_names_the_download_of_the_model_in_use() {
        let default = missing_model_warning("CodeRankEmbed", "nomic-ai/CodeRankEmbed", 548_000_000);
        assert!(
            default.contains("Run `jscpd --semantic-download` once (548 MB)"),
            "{default}"
        );
        let other = missing_model_warning(
            "jina-embeddings-v2-base-code",
            "jinaai/jina-embeddings-v2-base-code",
            324_000_000,
        );
        assert!(
            other.contains(
                "Run `jscpd --semantic-download jina-embeddings-v2-base-code` once (324 MB)"
            ),
            "{other}"
        );
    }

    #[test]
    fn the_vectors_of_one_call_serve_every_rule() {
        let embedded = Embedded::new(&["a", "b"], vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
        assert_eq!(
            embedded.embed(&["b", "a"]).unwrap(),
            vec![vec![0.0, 1.0], vec![1.0, 0.0]]
        );
        assert!(embedded.embed(&["c"]).is_err());
    }
}
