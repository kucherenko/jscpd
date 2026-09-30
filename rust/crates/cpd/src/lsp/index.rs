//! One project's scan, kept for as long as the server runs: the tokens of
//! every file, grouped into detection pools, and the clones and similar
//! functions they give. A file that changes in an editor is tokenized again
//! from its buffer, and only the pools it belongs to are searched again.

use cpd_core::detect::{PathFilters, PreparedSource, detect_prepared, merge_gapped_clones};
use cpd_core::models::{CpdClone, SourceFile, Statistics};
use cpd_core::similarity::{collect_function_sources, find_similar_functions};
use cpd_finder::orchestrate::{
    FilePreparer, RunConfig, canonicalize_all, pool_key, prepare_files_in, walk_config,
};
use cpd_finder::statistics;
use std::collections::{BTreeSet, HashMap};
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
    /// Every file of the scan, by source id.
    files: HashMap<String, IndexedFile>,
    /// The clones of the token passes (exact, renamed, merged across a
    /// gap), by detection pool.
    clones: HashMap<String, Vec<CpdClone>>,
    /// Pairs of functions with the same syntax-tree shape, when the ast
    /// analysis is on.
    similar: Vec<CpdClone>,
}

impl ScanIndex {
    /// Scan `run.paths` and search every pool. `run.similarity` below 1
    /// turns the search for similar functions on.
    pub fn build(pool: &rayon::ThreadPool, run: RunConfig) -> Self {
        let files = prepare_files_in(pool, &run)
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

    /// Every clone of the project: the token passes', then the similar
    /// functions'.
    pub fn clones(&self) -> impl Iterator<Item = &CpdClone> {
        let mut keys: Vec<&String> = self.clones.keys().collect();
        keys.sort();
        keys.into_iter()
            .flat_map(|key| self.clones[key].iter())
            .chain(self.similar.iter())
    }

    /// The display sources of every file, for summaries.
    pub fn sources(&self) -> Vec<SourceFile> {
        self.files
            .values()
            .flat_map(|f| f.sources.iter().cloned())
            .collect()
    }

    pub fn statistics(&self) -> Statistics {
        let clones: Vec<CpdClone> = self.clones().cloned().collect();
        statistics::compute(&self.sources(), &clones)
    }

    /// Tokenize the file `id` again, from `text` (an editor's buffer) or,
    /// without it, from the disk, and search its pools again. A file the
    /// index does not know joins it when the walk would have taken it, in
    /// the format `walk_format` says. Returns whether anything changed.
    pub fn update(&mut self, pool: &rayon::ThreadPool, id: &str, text: Option<&str>) -> bool {
        let (format, real_path) = match self.files.get(id) {
            Some(file) => (file.format.clone(), file.real_path.clone()),
            None => match self.walk_format(Path::new(id)) {
                Some(format) => (format, String::new()),
                None => return false,
            },
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
                    Err(_) => return self.remove(pool, id),
                }
            }
        };
        let (sources, prepared) = FilePreparer::new(&self.run)
            .prepare(id.to_string(), real_path.clone(), &format, text)
            .unwrap_or_default();
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
        for key in keys {
            self.search_pool(pool, &key);
        }
        self.search_similar();
        true
    }

    /// Drop the file `id`, deleted from the disk, and search its pools
    /// again. Returns whether the index had it.
    pub fn remove(&mut self, pool: &rayon::ThreadPool, id: &str) -> bool {
        let keys = self.pool_keys_of(id);
        if self.files.remove(id).is_none() {
            return false;
        }
        for key in keys {
            self.search_pool(pool, &key);
        }
        self.search_similar();
        true
    }

    /// The format a walk would give the file at `path`, if it would take it.
    pub fn walk_format(&self, path: &Path) -> Option<String> {
        let root = self.scan_roots.iter().find(|root| path.starts_with(root))?;
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
        let mut sources: Vec<PreparedSource> = self
            .files
            .values()
            .flat_map(|file| file.prepared.iter())
            .filter(|p| pool_key(&p.format, &self.run.cross_formats) == key)
            .cloned()
            .collect();
        if sources.is_empty() {
            self.clones.remove(key);
            return;
        }
        // The deterministic order of a scan's pools.
        sources.sort_unstable_by(|a, b| a.format.cmp(&b.format).then(a.id.cmp(&b.id)));
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
        let prepared: Vec<PreparedSource> = self
            .files
            .values()
            .flat_map(|file| file.prepared.iter())
            .filter(|p| !p.functions.is_empty())
            .cloned()
            .collect();
        let existing: Vec<CpdClone> = self.clones.values().flatten().cloned().collect();
        let mut similar = find_similar_functions(
            collect_function_sources(&prepared),
            threshold,
            self.run.min_tokens,
            self.run.min_lines,
            &existing,
        );
        if !self.run.kinds.is_empty() {
            similar.retain(|clone| self.run.kinds.iter().any(|kind| kind.matches(clone)));
        }
        self.similar = similar;
    }
}

/// The file a fragment with `source_id` lies in: an embedded block
/// (`<path>:<format>`, such as the script of a component) belongs to its
/// host file. The two fragments of a semantic pair can be in different
/// languages, so the suffix is any format's name, not the clone's.
pub fn host_file(source_id: &str) -> &str {
    static FORMATS: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    let formats =
        FORMATS.get_or_init(|| cpd_tokenizer::formats::list_formats().into_iter().collect());
    match source_id.rsplit_once(':') {
        Some((host, format)) if formats.contains(format) => host,
        _ => source_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "jscpd-lsp-index-{}-{}",
            std::process::id(),
            files.len()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, text) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        std::fs::canonicalize(dir).unwrap()
    }

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
        let mut index = ScanIndex::build(&pool, run);
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
    }
}
