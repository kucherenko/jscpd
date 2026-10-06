//! One project's scan, kept for as long as a server runs (`--lsp`,
//! `--mcp`): the tokens of every file, grouped into detection pools, and the
//! clones and similar functions they give. A file that changes in an editor
//! is tokenized again from its buffer, and only the pools it belongs to are
//! searched again.

use cpd_core::detect::{PathFilters, PreparedSource, detect_prepared, merge_gapped_clones};
use cpd_core::models::{CpdClone, SourceFile, Statistics};
use cpd_core::similarity::{
    collect_function_sources, discount_token_lines, find_similar_functions,
};
use cpd_finder::orchestrate::{
    FilePreparer, RunConfig, canonicalize_all, pool_key, prepare_files_in, walk_config,
};
use cpd_finder::statistics;
use cpd_finder::walker::{WalkConfig, ignored_by_files, walk_excluding};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// A file of the index: what a scan made of it.
struct IndexedFile {
    /// The format the walk gave the file.
    format: String,
    /// The canonical path when it differs from the id, else empty.
    real_path: String,
    sources: Vec<SourceFile>,
    prepared: Vec<PreparedSource>,
}

pub struct ScanIndex {
    run: RunConfig,
    scan_roots: Vec<PathBuf>,
    isolated_groups: Vec<Vec<PathBuf>>,
    /// The folders of nested projects, which those projects scan.
    exclude_dirs: Vec<PathBuf>,
    /// Every file of the scan, by source id, in order, so the files of a
    /// folder are one range.
    files: BTreeMap<String, IndexedFile>,
    /// The clones of the token passes (exact, renamed, merged across a
    /// gap), by detection pool.
    clones: HashMap<String, Vec<CpdClone>>,
    /// Pairs of functions with the same syntax-tree shape, when the ast
    /// analysis is on.
    similar: Vec<CpdClone>,
}

impl ScanIndex {
    /// Scan `run.paths`, leaving out the folders `exclude_dirs`, and search
    /// every pool. `run.similarity` below 1 turns the search for similar
    /// functions on.
    pub fn build(pool: &rayon::ThreadPool, run: RunConfig, exclude_dirs: Vec<PathBuf>) -> Self {
        let files = prepare_files_in(pool, &run, &exclude_dirs)
            .into_iter()
            .map(|file| {
                (
                    file.id,
                    IndexedFile {
                        format: file.format,
                        real_path: file.real_path,
                        sources: file.sources,
                        prepared: file.prepared,
                    },
                )
            })
            .collect();
        let mut index = Self {
            exclude_dirs,
            scan_roots: canonicalize_all(&run.paths),
            isolated_groups: run
                .skip_isolated
                .iter()
                .map(|g| canonicalize_all(g))
                .collect(),
            run,
            files,
            clones: HashMap::new(),
            similar: Vec::new(),
        };
        let keys: BTreeSet<String> = index.pool_keys_of_all();
        for key in keys {
            index.search_pool(pool, &key);
        }
        index.search_similar();
        index
    }

    #[cfg(test)]
    fn contains(&self, id: &str) -> bool {
        self.files.contains_key(id)
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Every file of the scan, in order: its source id, the format the walk
    /// gave it, its canonical path when that differs from the id (else
    /// empty), and its detection-ready sources.
    pub fn files(&self) -> impl Iterator<Item = (&str, &str, &str, &[PreparedSource])> {
        self.files.iter().map(|(id, file)| {
            (
                id.as_str(),
                file.format.as_str(),
                file.real_path.as_str(),
                file.prepared.as_slice(),
            )
        })
    }

    /// The detection-ready sources of every file, in file order.
    pub fn prepared(&self) -> impl Iterator<Item = &PreparedSource> {
        self.files.values().flat_map(|file| file.prepared.iter())
    }

    /// The sources of the detection pool `key` (see [`pool_key`]), in the
    /// order a scan searches them.
    pub fn pool_sources(&self, key: &str) -> Vec<PreparedSource> {
        let mut sources: Vec<PreparedSource> = self
            .prepared()
            .filter(|p| pool_key(&p.format, &self.run.cross_formats) == key)
            .cloned()
            .collect();
        // The deterministic order of a scan's pools.
        sources.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));
        sources
    }

    /// Every clone of the project: the token passes', then the similar
    /// functions'.
    pub fn clones(&self) -> impl Iterator<Item = &CpdClone> {
        self.token_clones().chain(self.similar.iter())
    }

    /// The clones of the token passes (exact, renamed, merged across a
    /// gap), pool by pool.
    pub fn token_clones(&self) -> impl Iterator<Item = &CpdClone> {
        let mut keys: Vec<&String> = self.clones.keys().collect();
        keys.sort();
        keys.into_iter().flat_map(|key| self.clones[key].iter())
    }

    /// The pairs of functions with the same syntax-tree shape, when the
    /// run's `similarity` is below 1.
    pub fn similar_pairs(&self) -> &[CpdClone] {
        &self.similar
    }

    /// The display sources of every file, for summaries.
    pub fn sources(&self) -> Vec<SourceFile> {
        self.files
            .values()
            .flat_map(|f| f.sources.iter().cloned())
            .collect()
    }

    pub fn statistics(&self) -> Statistics {
        let mut clones: Vec<CpdClone> = self.clones().cloned().collect();
        // A line that a token clone reports counts once, as in a report.
        discount_token_lines(&mut clones);
        self.statistics_of(&clones)
    }

    /// The statistics of the scanned files with `clones` as their clones.
    pub fn statistics_of(&self, clones: &[CpdClone]) -> Statistics {
        statistics::compute(self.files.values().flat_map(|f| f.sources.iter()), clones)
    }

    /// Tokenize the file `id` again, from `text` (an editor's buffer) or,
    /// without it, from the disk, and search its pools again. A file the
    /// index does not know joins it when the walk would have taken it, in
    /// the format `walk_format` says. Returns whether anything changed.
    #[cfg(test)]
    pub fn update(&mut self, pool: &rayon::ThreadPool, id: &str, text: Option<&str>) -> bool {
        self.update_all(pool, &[(id.to_string(), text)])
    }

    /// [`ScanIndex::update`] for several files, searching each pool they
    /// touch once.
    pub fn update_all(
        &mut self,
        pool: &rayon::ThreadPool,
        files: &[(String, Option<&str>)],
    ) -> bool {
        let mut keys = BTreeSet::new();
        let mut changed = false;
        for (id, text) in files {
            if let Some(touched) = self.replace(id, *text) {
                keys.extend(touched);
                changed = true;
            }
        }
        if changed {
            self.search(pool, keys);
        }
        changed
    }

    /// Whether the file at `path` is this scan's or would be: under one of
    /// its roots and outside the folders of nested projects. Whether the
    /// walk takes it is for [`ScanIndex::update`] to find out.
    pub fn covers(&self, path: &Path) -> bool {
        self.files.contains_key(path.to_string_lossy().as_ref())
            || (self.scan_roots.iter().any(|root| path.starts_with(root))
                && !self.exclude_dirs.iter().any(|dir| path.starts_with(dir)))
    }

    /// Read the paths `ids` again from the disk and search each pool they
    /// touch once. A path that is gone drops out; a folder stands for the
    /// indexed files under it and the files a walk finds in it now, for
    /// editors that report a folder moved or deleted as one change. Returns
    /// whether anything changed.
    pub fn refresh(&mut self, pool: &rayon::ThreadPool, ids: &[String]) -> bool {
        let mut keys = BTreeSet::new();
        let mut changed = false;
        let files: BTreeSet<String> = ids.iter().flat_map(|id| self.files_at(id)).collect();
        for id in files {
            if let Some(touched) = self.replace(&id, None) {
                keys.extend(touched);
                changed = true;
            }
        }
        if changed {
            self.search(pool, keys);
        }
        changed
    }

    /// The files a change at `id` concerns: the file itself, or for a
    /// folder, or a path that is gone and was one, the files under it.
    fn files_at(&self, id: &str) -> Vec<String> {
        let path = Path::new(id);
        if self.files.contains_key(id) || path.is_file() {
            return vec![id.to_string()];
        }
        let prefix = format!("{id}{}", std::path::MAIN_SEPARATOR);
        let mut ids: Vec<String> = self
            .files
            .range(prefix.clone()..)
            .take_while(|(known, _)| known.starts_with(&prefix))
            .map(|(known, _)| known.clone())
            .collect();
        if path.is_dir()
            && let Some(root) = self.scan_roots.iter().find(|root| path.starts_with(root))
            && !self.exclude_dirs.iter().any(|dir| path.starts_with(dir))
            && !ignored_by_files(path, root, true, self.run.no_gitignore)
        {
            let config = WalkConfig {
                paths: vec![path.to_path_buf()],
                ..walk_config(&self.run)
            };
            ids.extend(
                walk_excluding(&config, &self.exclude_dirs)
                    .into_iter()
                    .map(|file| file.path.to_string_lossy().into_owned()),
            );
        }
        ids
    }

    /// Tokenize the file `id` again without searching: the pools to search
    /// again, or `None` when the index does not take the file.
    fn replace(&mut self, id: &str, text: Option<&str>) -> Option<BTreeSet<String>> {
        let (format, real_path) = match self.files.get(id) {
            Some(file) => (file.format.clone(), file.real_path.clone()),
            None => (self.walk_format(Path::new(id))?, String::new()),
        };
        let from_disk;
        let text = match text {
            Some(text) => text,
            None => {
                let path = match real_path.is_empty() {
                    true => id,
                    false => real_path.as_str(),
                };
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        from_disk = content;
                        &from_disk
                    }
                    // Deleted: its pools lose it.
                    Err(_) => {
                        let keys = self.pool_keys_of(id);
                        return self.files.remove(id).map(|_| keys);
                    }
                }
            }
        };
        let Some((sources, prepared)) =
            FilePreparer::new(&self.run).prepare(id.to_string(), real_path.clone(), &format, text)
        else {
            // Too short or too long for the run, as a scan would find it:
            // its pools lose it.
            let keys = self.pool_keys_of(id);
            return self.files.remove(id).map(|_| keys);
        };
        // The same tokens at the same places, as when an editor opens a file
        // with the text the index has: the pools would find what they found.
        let unchanged = self
            .files
            .get(id)
            .is_some_and(|old| same_tokens(&old.prepared, &prepared));
        let mut keys = self.pool_keys_of(id);
        let file = IndexedFile {
            format,
            real_path,
            sources,
            prepared,
        };
        keys.extend(
            file.prepared
                .iter()
                .map(|p| pool_key(&p.format, &self.run.cross_formats)),
        );
        self.files.insert(id.to_string(), file);
        match unchanged {
            true => None,
            false => Some(keys),
        }
    }

    fn search(&mut self, pool: &rayon::ThreadPool, keys: BTreeSet<String>) {
        for key in keys {
            self.search_pool(pool, &key);
        }
        self.search_similar();
    }

    /// The format a walk would give the file at `path`, if it would take it.
    pub fn walk_format(&self, path: &Path) -> Option<String> {
        let root = self.scan_roots.iter().find(|root| path.starts_with(root))?;
        if self.exclude_dirs.iter().any(|dir| path.starts_with(dir)) {
            return None;
        }
        cpd_finder::walker::accepts(path, root, &walk_config(&self.run))
    }

    fn pool_keys_of(&self, id: &str) -> BTreeSet<String> {
        self.files
            .get(id)
            .into_iter()
            .flat_map(|file| file.prepared.iter())
            .map(|p| pool_key(&p.format, &self.run.cross_formats))
            .collect()
    }

    fn pool_keys_of_all(&self) -> BTreeSet<String> {
        self.files
            .values()
            .flat_map(|file| file.prepared.iter())
            .map(|p| pool_key(&p.format, &self.run.cross_formats))
            .collect()
    }

    /// Search one detection pool again, as a scan does: the token passes,
    /// the merge across gaps and the `--kind` filter.
    fn search_pool(&mut self, pool: &rayon::ThreadPool, key: &str) {
        let sources = self.pool_sources(key);
        if sources.is_empty() {
            self.clones.remove(key);
            return;
        }
        let filters = PathFilters {
            skip_local: self.run.skip_local,
            scan_roots: &self.scan_roots,
            isolated_groups: &self.isolated_groups,
        };
        let found = pool.install(|| {
            detect_prepared(
                vec![sources],
                self.run.min_tokens,
                self.run.min_lines,
                &filters,
            )
        });
        let mut found = merge_gapped_clones(found, self.run.max_gap_lines);
        if !self.run.kinds.is_empty() {
            found.retain(|clone| self.run.kinds.iter().any(|kind| kind.matches(clone)));
        }
        self.clones.insert(key.to_string(), found);
    }

    /// Pair the functions of every file again, when the ast analysis is on.
    fn search_similar(&mut self) {
        let Some(threshold) = self.run.similarity_threshold() else {
            self.similar.clear();
            return;
        };
        let mut prepared: Vec<PreparedSource> = self
            .files
            .values()
            .flat_map(|file| file.prepared.iter())
            .filter(|p| !p.functions.is_empty())
            .cloned()
            .collect();
        // The order of a scan: a pair's format is its first function's, and
        // the pairing keeps a bounded number of candidates per bucket.
        prepared.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));
        let existing: Vec<CpdClone> = self.clones.values().flatten().cloned().collect();
        let filters = PathFilters {
            skip_local: self.run.skip_local,
            scan_roots: &self.scan_roots,
            isolated_groups: &self.isolated_groups,
        };
        let mut similar = find_similar_functions(
            collect_function_sources(&prepared),
            threshold,
            self.run.min_tokens,
            self.run.min_lines,
            &existing,
            &filters,
        );
        if !self.run.kinds.is_empty() {
            similar.retain(|clone| self.run.kinds.iter().any(|kind| kind.matches(clone)));
        }
        self.similar = similar;
    }
}

fn same_tokens(a: &[PreparedSource], b: &[PreparedSource]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.id == b.id
                && a.format == b.format
                && a.embedded == b.embedded
                && a.hashes == b.hashes
                && a.raw_hashes == b.raw_hashes
                && a.spans == b.spans
                && a.functions == b.functions
        })
}

/// The file a fragment with `source_id` lies in: an embedded block
/// (`<path>:<format>`, such as the script of a component) belongs to its
/// host file. The two fragments of a semantic pair can be in different
/// languages, so the suffix is any format's name, not the clone's, or the
/// name of a block that is no format of its own (`html` for the markup of a
/// component, `text` for a code fence without a language).
pub fn host_file(source_id: &str) -> &str {
    static FORMATS: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    let formats =
        FORMATS.get_or_init(|| cpd_tokenizer::formats::list_formats().into_iter().collect());
    match source_id.rsplit_once(':') {
        Some((host, suffix)) if formats.contains(suffix) || is_block_name(host, suffix) => host,
        _ => source_id,
    }
}

/// Whether `suffix` names a block of the file `host`: a bare name after a
/// file with an extension, unlike the rest of `C:\\p\\a.js` after its drive or
/// a colon inside a file name such as `a:b.js`.
fn is_block_name(host: &str, suffix: &str) -> bool {
    let name = host.rsplit(['/', '\\']).next().unwrap_or(host);
    name.contains('.')
        && !suffix.is_empty()
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '#'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::project;

    const BODY: &str = "export function total(items) {\n  let sum = 0;\n  for (const item of items) {\n    if (item.price > 0 && item.count > 0) {\n      sum += item.price * item.count;\n    }\n  }\n  return Math.round(sum * 100) / 100;\n}\n";

    #[test]
    fn a_buffer_replaces_its_file_and_its_pool_is_searched_again() {
        let dir = project(&[("a.js", BODY), ("b.js", BODY)]);
        let pool = cpd_finder::orchestrate::build_thread_pool(Some(2));
        let run = RunConfig {
            paths: vec![dir.clone()],
            min_tokens: 20,
            min_lines: 3,
            ..RunConfig::default()
        };
        let mut index = ScanIndex::build(&pool, run, Vec::new());
        assert_eq!(index.clones().count(), 1);
        let a = dir.join("a.js").to_string_lossy().into_owned();
        assert!(index.update(&pool, &a, Some("export const x = 1;\n")));
        assert_eq!(
            index.clones().count(),
            0,
            "the edited copy no longer matches"
        );
        assert!(index.update(&pool, &a, None), "back to the disk version");
        assert_eq!(index.clones().count(), 1);
        // A new file the walk would take joins the index.
        let c = dir.join("c.js").to_string_lossy().into_owned();
        assert!(index.update(&pool, &c, Some(BODY)));
        assert!(index.contains(&c));
        // A third copy pairs with the first, as in a scan.
        assert_eq!(index.clones().count(), 2);
        assert!(!index.update(
            &pool,
            &dir.join("notes.unknownext").to_string_lossy(),
            Some(BODY)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn host_file_folds_an_embedded_block_into_its_file() {
        assert_eq!(host_file("/p/README.md:javascript"), "/p/README.md");
        assert_eq!(host_file("/p/Form.svelte:typescript"), "/p/Form.svelte");
        assert_eq!(host_file("/p/a.js"), "/p/a.js");
        assert_eq!(host_file(r"C:\p\a.js"), r"C:\p\a.js");
        assert_eq!(host_file("/p/Form.vue:html"), "/p/Form.vue");
        assert_eq!(host_file(r"C:\p\Form.vue:html"), r"C:\p\Form.vue");
        assert_eq!(host_file("/p/notes.md:text"), "/p/notes.md");
        assert_eq!(host_file("/p/a:b.js"), "/p/a:b.js");
    }
}
