// walker.rs

use std::{
    collections::{HashMap, HashSet},
    io::BufRead,
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Debug, Clone, Default)]
pub struct WalkConfig {
    pub paths: Vec<PathBuf>,
    pub extensions: Vec<String>, // empty = all supported formats
    pub ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub follow_symlinks: bool,
    pub no_gitignore: bool,
    pub formats_exts: HashMap<String, Vec<String>>,
    pub formats_names: HashMap<String, Vec<String>>,
    pub pattern: Option<String>,
}

#[derive(Debug)]
pub struct DiscoveredFile {
    /// The path the walker found the file at, anchored at the canonical scan
    /// root. This is the source id: what reports show and what `--ignore`
    /// matched. A file reached through a symlink keeps its walked name here.
    pub path: PathBuf,
    pub format: String,
    /// Canonical path of the file: where its bytes are read from, how files
    /// reachable through several paths are recognised as one, and the
    /// `--skip-isolated` fallback for symlinked group folders. Equal to `path`
    /// unless `--follow-symlinks` walked through a link (issue #1059).
    pub real_path: PathBuf,
    // File content is intentionally NOT stored here.  Each rayon worker
    // opens and memory-maps its file in the processing step, so at most
    // `num_threads` mmaps are live simultaneously — safe for any repo size
    // regardless of vm.max_map_count.
}

/// Build a GlobSet for the positive `--pattern` filter.
///
/// Relative patterns (those not starting with `/` or a Windows drive letter)
/// get an additional `**/` prefix variant so they match at any depth —
/// matching the behaviour of the ignore-pattern handling and of jscpd v4.
fn build_positive_glob_set(pattern: &str) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    if let Ok(g) = Glob::new(pattern) {
        builder.add(g);
    }
    // For relative patterns, add a `**/` variant so `src/**/*.ts` also
    // matches when the walked path is `subdir/src/foo.ts`, and bare
    // patterns like `*.ts` match at any depth.
    let anchored = pattern.starts_with('/') || (cfg!(windows) && is_windows_drive_path(pattern));
    if !anchored {
        let prefixed = format!("**/{}", pattern.trim_start_matches("./"));
        if let Ok(g) = Glob::new(&prefixed) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// True for paths anchored at a Windows drive root: `C:\src`, `c:/src`.
///
/// Pure string inspection so it can be unit-tested on every platform; the
/// caller decides whether the host treats such paths as absolute.
fn is_windows_drive_path(p: &str) -> bool {
    let mut chars = p.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(drive), Some(':'), Some('\\' | '/')) if drive.is_ascii_alphabetic()
    )
}

/// Build a pre-compiled GlobSet from ignore pattern strings.
///
/// Falls back gracefully — patterns that fail to parse as globs are skipped
/// with a debug log. This is a correctness improvement over the previous
/// substring-contains check: glob semantics are strictly more precise.
fn build_ignore_glob_set(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let p = pattern.trim_start_matches('/');
        // Try as-is first; also add a `**/prefix` variant so bare directory
        // names like "node_modules" match at any depth.
        if let Ok(g) = Glob::new(p) {
            builder.add(g);
        }
        let glob_any_depth = format!("**/{}", p);
        if let Ok(g) = Glob::new(&glob_any_depth) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

pub fn walk(config: &WalkConfig) -> Vec<DiscoveredFile> {
    walk_excluding(config, &[])
}

/// [`walk`], leaving out the folders `exclude_dirs` whole: `--lsp` gives each
/// nested project a scan of its own. The folders are absolute paths,
/// compared with the walked paths of an absolute root.
pub fn walk_excluding(config: &WalkConfig, exclude_dirs: &[PathBuf]) -> Vec<DiscoveredFile> {
    let mut results = Vec::new();
    for root in &config.paths {
        walk_one(root, config, exclude_dirs, &mut results);
    }
    if config.follow_symlinks || config.paths.len() > 1 {
        dedup_by_real_path(&mut results);
    }
    results
}

/// One file can be reached through several paths: from two scan roots when
/// one lies inside the other, and with `--follow-symlinks` through a file
/// symlink next to its target, a directory symlink into the scan root or two
/// scan roots linked to each other. Keep one entry per real file so it is
/// neither counted twice nor reported as a clone of itself (issue #1059).
/// The entry that *is* the real file wins over one reached through a link;
/// between links the lexicographically first walked path wins, so the result
/// does not depend on the parallel walk order.
fn dedup_by_real_path(results: &mut Vec<DiscoveredFile>) {
    results.sort_by(|a, b| (a.path != a.real_path, &a.path).cmp(&(b.path != b.real_path, &b.path)));
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(results.len());
    results.retain(|f| seen.insert(f.real_path.clone()));
}

/// The walked path re-anchored at the canonical scan root: `root_canon` plus
/// the components the walker appended below `root`. The root's own resolution
/// (a relative root, macOS `/var` → `/private/var`) is applied once, so
/// scan-root-relative display works; symlinks *below* the root are not
/// resolved, so a file keeps the name it was found by.
fn anchor_at_root(path: &Path, root: &Path, root_canon: &Path) -> PathBuf {
    match path.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => root_canon.to_path_buf(),
        Ok(rel) => root_canon.join(rel),
        Err(_) => path.to_path_buf(),
    }
}

fn walk_one(
    root: &Path,
    config: &WalkConfig,
    exclude_dirs: &[PathBuf],
    results: &mut Vec<DiscoveredFile>,
) {
    let mut builder = WalkBuilder::new(root);
    builder.follow_links(config.follow_symlinks);
    builder.git_ignore(!config.no_gitignore);
    builder.hidden(false);
    if !exclude_dirs.is_empty() {
        let excluded = exclude_dirs.to_vec();
        builder.filter_entry(move |entry| !excluded.iter().any(|dir| entry.path() == dir));
    }

    // Pre-compile ignore glob set once — shared across all walker threads.
    let ignore_set = build_ignore_glob_set(&config.ignore_patterns);

    // Build a positive pattern glob set: if set, only files matching the
    // pattern are included. In v4, `pattern` (e.g. `**/*.ts`) was appended
    // to each scan path to form the glob passed to fast-glob. Here we
    // filter post-walk instead.
    //
    // We match against the path relative to the walk root so that relative
    // patterns like `src/**/*.ts` work regardless of whether the root is
    // absolute or relative. We also add a `**/` prefix variant for
    // relative patterns so that `*.ts` matches at any depth (consistent
    // with how ignore patterns are handled).
    let pattern_set = config.pattern.as_deref().map(build_positive_glob_set);

    // Canonicalize root once for relative path computation.
    let root_canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    // Use mpsc::channel for collection — cheaper than Arc<Mutex<Vec>> under parallelism.
    let (tx, rx) = mpsc::channel::<DiscoveredFile>();

    let follow_symlinks = config.follow_symlinks;
    let root_given = root.to_path_buf();
    let max_size = config.max_size;
    let extensions = config.extensions.clone();
    let formats_exts = config.formats_exts.clone();
    let formats_names = config.formats_names.clone();
    let pattern_set = pattern_set.clone();

    builder.build_parallel().run(move || {
        let tx = tx.clone();
        let extensions = extensions.clone();
        let ignore_set = ignore_set.clone();
        let formats_exts = formats_exts.clone();
        let formats_names = formats_names.clone();
        let pattern_set = pattern_set.clone();
        let root_canon = root_canon.clone();
        let root_given = root_given.clone();

        Box::new(move |entry_result| {
            use ignore::WalkState;

            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            let path = entry.path().to_path_buf();
            if !path.is_file() {
                return WalkState::Continue;
            }

            // Skip symlinks if not following.
            if !follow_symlinks
                && let Ok(meta) = std::fs::symlink_metadata(&path)
                && meta.file_type().is_symlink()
            {
                return WalkState::Continue;
            }

            // Size limit check (metadata only — no file read yet).
            if let Some(max) = max_size
                && let Ok(meta) = std::fs::metadata(&path)
                && meta.len() > max
            {
                return WalkState::Continue;
            }

            // Pattern filter: if set, only include files matching the positive glob.
            // Try matching against both the relative path (stripped of root prefix) and
            // the full path so that both relative patterns (e.g. `src/**/*.ts`) and
            // absolute patterns (e.g. `/project/src/**/*.ts`) work correctly.
            if let Some(ref ps) = pattern_set
                && !ps.is_empty()
            {
                let rel = path.strip_prefix(&root_canon).unwrap_or(&path);
                if !ps.is_match(rel) && !ps.is_match(&path) {
                    return WalkState::Continue;
                }
            }

            // Format detection.
            let format = match detect_format(&path, &extensions, &formats_exts, &formats_names) {
                Some(f) => f,
                None => return WalkState::Continue,
            };

            // Ignore patterns — pre-compiled GlobSet (correctness + speed vs substring).
            if !ignore_set.is_empty() && ignore_set.is_match(&path) {
                return WalkState::Continue;
            }

            // Only a walk that follows links can reach a file by a name that
            // is not its own; keep the default walk free of the extra syscall.
            let real_path = if follow_symlinks {
                std::fs::canonicalize(&path).ok()
            } else {
                None
            };
            let path = anchor_at_root(&path, &root_given, &root_canon);
            let real_path = real_path.unwrap_or_else(|| path.clone());
            let _ = tx.send(DiscoveredFile {
                path,
                format,
                real_path,
            });
            WalkState::Continue
        })
    });

    // Drain the channel.
    results.extend(rx);
}

/// The files of `files`, canonical paths, that a walk with `config` would
/// take: those under a scan path that [`accepts`] lets through. Cheaper than
/// a walk for a few files of a large tree.
pub fn take_files<'a>(
    files: impl IntoIterator<Item = &'a PathBuf>,
    config: &WalkConfig,
) -> Vec<DiscoveredFile> {
    let roots: Vec<PathBuf> = config
        .paths
        .iter()
        .map(|root| std::fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect();
    let mut taken: Vec<DiscoveredFile> = files
        .into_iter()
        .filter_map(|file| {
            let root = roots.iter().find(|root| file.starts_with(root))?;
            let format = accepts(file, root, config)?;
            Some(DiscoveredFile {
                path: file.clone(),
                format,
                real_path: file.clone(),
            })
        })
        .collect();
    taken.sort_by(|a, b| a.path.cmp(&b.path));
    taken
}

/// Whether a walk with `config` would take the file at `path`, under the scan
/// root `root`, and in which format: the format filters, `--pattern`,
/// `--ignore`, the ignore files (see [`ignored_by_files`]) and, for a file on
/// disk, the size limit. A language server asks this about a file an editor
/// has open, which may not be on disk yet, and about files that appear
/// while it runs.
pub fn accepts(path: &Path, root: &Path, config: &WalkConfig) -> Option<String> {
    if let Some(pattern) = config.pattern.as_deref() {
        let set = build_positive_glob_set(pattern);
        let relative = path.strip_prefix(root).unwrap_or(path);
        if !set.is_empty() && !set.is_match(relative) && !set.is_match(path) {
            return None;
        }
    }
    let format = detect_format(
        path,
        &config.extensions,
        &config.formats_exts,
        &config.formats_names,
    )?;
    let ignore = build_ignore_glob_set(&config.ignore_patterns);
    if !ignore.is_empty() && ignore.is_match(path) {
        return None;
    }
    if let Some(max) = config.max_size
        && std::fs::metadata(path).is_ok_and(|meta| meta.len() > max)
    {
        return None;
    }
    if ignored_by_files(path, root, false, config.no_gitignore) {
        return None;
    }
    Some(format)
}

/// Whether the ignore files leave `path` out of a walk from `root`, as they
/// do in the walk itself: `.ignore` files, and inside a git repository its
/// `.gitignore` files and `.git/info/exclude` (unless `no_gitignore`). A
/// walk checks every entry below its root and skips an ignored folder whole,
/// so the folders between `root` and `path` count as well as `path`; `root`
/// itself and the folders above it do not. The nearest ignore file decides,
/// and `.ignore` beats `.gitignore` in one folder.
pub fn ignored_by_files(path: &Path, root: &Path, is_dir: bool, no_gitignore: bool) -> bool {
    use ignore::gitignore::{Gitignore, GitignoreBuilder};
    let Ok(below) = path.strip_prefix(root) else {
        return false;
    };
    let repo = root
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .filter(|_| !no_gitignore);
    let mut loaded: HashMap<PathBuf, Option<Gitignore>> = HashMap::new();
    let mut load = |file: PathBuf, dir: &Path| -> Option<Gitignore> {
        loaded
            .entry(file.clone())
            .or_insert_with(|| {
                if !file.is_file() {
                    return None;
                }
                let mut builder = GitignoreBuilder::new(dir);
                builder.add(&file);
                builder.build().ok().filter(|gi| !gi.is_empty())
            })
            .clone()
    };
    let mut entry = root.to_path_buf();
    let names: Vec<_> = below.components().collect();
    for (i, name) in names.iter().enumerate() {
        entry.push(name);
        let entry_is_dir = is_dir || i + 1 < names.len();
        let mut decided = None;
        // The folders above the entry, nearest first.
        for dir in entry.ancestors().skip(1) {
            let mut files = vec![dir.join(".ignore")];
            if repo.is_some_and(|repo| dir.starts_with(repo)) {
                files.push(dir.join(".gitignore"));
            }
            for file in files {
                if let Some(gi) = load(file, dir) {
                    let matched = gi.matched(&entry, entry_is_dir);
                    if matched.is_ignore() || matched.is_whitelist() {
                        decided = Some(matched.is_ignore());
                        break;
                    }
                }
            }
            if decided.is_some() {
                break;
            }
        }
        if decided.is_none()
            && let Some(repo) = repo
            && let Some(gi) = load(repo.join(".git/info/exclude"), repo)
        {
            let matched = gi.matched(&entry, entry_is_dir);
            if matched.is_ignore() || matched.is_whitelist() {
                decided = Some(matched.is_ignore());
            }
        }
        if decided == Some(true) {
            return true;
        }
    }
    false
}

fn detect_format(
    path: &Path,
    filter: &[String],
    formats_exts: &HashMap<String, Vec<String>>,
    formats_names: &HashMap<String, Vec<String>>,
) -> Option<String> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Priority 1: check formats_names (filename-based matching)
    if !formats_names.is_empty() {
        for (format, names) in formats_names {
            if names.iter().any(|n| n == file_name)
                && (filter.is_empty() || filter.iter().any(|e| e == format))
            {
                return Some(format.clone());
            }
        }
    }

    // Priority 2: check formats_exts (extension-based matching)
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !formats_exts.is_empty() && !ext.is_empty() {
        for (format, exts) in formats_exts {
            if exts.iter().any(|e| e == ext)
                && (filter.is_empty() || filter.iter().any(|e| e == format))
            {
                return Some(format.clone());
            }
        }
    }

    // Priority 3: built-in format detection
    let fmt = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(cpd_tokenizer::formats::get_format_by_extension)
        .or_else(|| {
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::new(file);
            let line = reader.lines().next()?.ok()?;

            if line.starts_with("#!") {
                cpd_tokenizer::formats::get_format_by_shebang(&line)
            } else {
                None
            }
        })?;

    if !filter.is_empty() && !filter.iter().any(|e| e == fmt) {
        return None;
    }
    Some(fmt.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty temp directory for one test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cpd-walker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    /// `(file name, format)` of every walked file, sorted.
    fn names_and_formats(config: &WalkConfig) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = walk(config)
            .into_iter()
            .map(|f| {
                let name = f.path.file_name().unwrap().to_string_lossy().into_owned();
                (name, f.format)
            })
            .collect();
        found.sort();
        found
    }

    fn names(config: &WalkConfig) -> Vec<String> {
        names_and_formats(config)
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(n, f)| (n.to_string(), f.to_string()))
            .collect()
    }

    #[test]
    fn taken_files_are_the_ones_the_walk_takes() {
        let dir = temp_dir("take-files");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        write(&dir, ".gitignore", "*.gen.js\n");
        let code = "export const a = 1;\n";
        let files: Vec<PathBuf> = [
            "src/a.js",
            "src/b.py",
            "src/c.txt-unknown",
            "src/d.gen.js",
            "other/e.js",
        ]
        .iter()
        .map(|rel| std::fs::canonicalize(write(&dir, rel, code)).unwrap())
        .collect();
        let root = std::fs::canonicalize(&dir).unwrap();
        let config = WalkConfig {
            paths: vec![root.join("src")],
            extensions: vec!["javascript".to_string(), "python".to_string()],
            ignore_patterns: vec!["**/*.py".to_string()],
            ..Default::default()
        };
        let taken: Vec<String> = take_files(&files, &config)
            .into_iter()
            .map(|f| {
                assert_eq!(f.path, f.real_path);
                f.path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        // b.py by --ignore, c by its format, d.gen.js by .gitignore, e.js
        // by the scan paths.
        assert_eq!(taken, ["src/a.js"]);
        let walked: Vec<String> = names(&config);
        assert_eq!(walked, ["a.js"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignore_files_leave_a_path_out_as_the_walk_does() {
        let dir = temp_dir("ignored-by-files");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("src/keep")).unwrap();
        std::fs::write(dir.join(".gitignore"), "dist/\n*.gen.js\n").unwrap();
        std::fs::write(dir.join("src/.gitignore"), "!keep.gen.js\n").unwrap();
        let root = std::fs::canonicalize(&dir).unwrap();
        let ignored = |path: &str| ignored_by_files(&root.join(path), &root, false, false);
        assert!(ignored("dist/a.js"), "a file in an ignored folder");
        assert!(ignored("dist/deep/a.js"), "and deeper");
        assert!(ignored("src/x.gen.js"));
        assert!(!ignored("src/keep.gen.js"), "a nearer file wins");
        assert!(!ignored("src/a.js"));
        assert!(
            !ignored_by_files(&root.join("dist/a.js"), &root, false, true),
            "no_gitignore"
        );
        // The root itself is never ignored, as a walk never skips its root.
        let dist = root.join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        assert!(!ignored_by_files(&dist.join("a.js"), &dist, false, false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignored_by_files_agrees_with_the_walk() {
        // Whatever the walk leaves out, ignored_by_files says is ignored, and
        // the other way round: the language server relies on the two agreeing.
        let dir = temp_dir("ignored-agrees");
        std::fs::create_dir_all(dir.join(".git/info")).unwrap();
        write(&dir, ".git/info/exclude", "local/\n");
        write(&dir, ".ignore", "vendor/\n");
        write(&dir, ".gitignore", "build/\n");
        for f in ["src/a.js", "local/b.js", "vendor/c.js", "build/d.js"] {
            write(&dir, f, "let x = 1;\n");
        }
        let root = std::fs::canonicalize(&dir).unwrap();
        let walked = names(&WalkConfig {
            paths: vec![root.clone()],
            ..Default::default()
        });
        assert_eq!(walked, ["a.js"]);
        for (rel, ignored) in [
            ("src/a.js", false),
            ("local/b.js", true),
            ("vendor/c.js", true),
            ("build/d.js", true),
        ] {
            assert_eq!(
                ignored_by_files(&root.join(rel), &root, false, false),
                ignored,
                "{rel}"
            );
        }
        // A path outside the root is not the root's business.
        assert!(!ignored_by_files(
            Path::new("/elsewhere/x.js"),
            &root,
            false,
            false
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn fixtures() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walker");
        assert!(dir.is_dir(), "missing fixture folder {}", dir.display());
        dir
    }

    #[test]
    fn windows_drive_path_detection() {
        for anchored in ["C:\\src\\**\\*.ts", "c:/src/**/*.ts", "D:\\"] {
            assert!(is_windows_drive_path(anchored), "{anchored}");
        }
        for relative in [
            "src/**/*.ts",
            "C:",
            "C:src",
            ":\\src",
            "/abs/path",
            "",
            "1:\\x",
        ] {
            assert!(!is_windows_drive_path(relative), "{relative}");
        }
    }

    #[test]
    fn walks_every_supported_file_with_its_format() {
        let config = WalkConfig {
            paths: vec![fixtures()],
            ..Default::default()
        };
        assert_eq!(
            names_and_formats(&config),
            pairs(&[
                ("file1.js", "javascript"),
                ("file2.ts", "typescript"),
                ("file3.py", "python"),
            ])
        );
    }

    #[test]
    fn walked_paths_are_anchored_at_the_canonical_root() {
        let root = std::fs::canonicalize(fixtures()).unwrap();
        let config = WalkConfig {
            paths: vec![fixtures()],
            ..Default::default()
        };
        for file in walk(&config) {
            assert!(file.path.starts_with(&root), "{}", file.path.display());
            assert!(file.path.is_file(), "{}", file.path.display());
            assert_eq!(file.real_path, file.path, "no links were followed");
        }
    }

    #[test]
    fn a_single_file_root_is_walked_as_itself() {
        let file = fixtures().join("subdir_a/file1.js");
        let found = walk(&WalkConfig {
            paths: vec![file.clone()],
            ..Default::default()
        });
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, std::fs::canonicalize(&file).unwrap());
        assert_eq!(found[0].format, "javascript");
    }

    #[test]
    fn a_scan_root_inside_another_adds_no_second_copy() {
        let dir = fixtures();
        let config = WalkConfig {
            paths: vec![dir.clone(), dir.join("subdir_a")],
            ..Default::default()
        };
        assert_eq!(names(&config), ["file1.js", "file2.ts", "file3.py"]);
    }

    #[test]
    fn excluded_folders_are_left_out_whole() {
        let root = std::fs::canonicalize(fixtures()).unwrap();
        let config = WalkConfig {
            paths: vec![root.clone()],
            ..Default::default()
        };
        let mut found: Vec<String> = walk_excluding(&config, &[root.join("subdir_a")])
            .into_iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, ["file3.py"]);
    }

    #[test]
    fn nonexistent_path_returns_empty() {
        let config = WalkConfig {
            paths: vec![PathBuf::from("/tmp/cpd-nonexistent-xyz")],
            ..Default::default()
        };
        assert!(walk(&config).is_empty());
    }

    #[test]
    fn max_size_keeps_files_up_to_the_limit() {
        let dir = temp_dir("max-size");
        write(&dir, "small.js", "a;\n");
        write(&dir, "large.js", &"let x = 1;\n".repeat(100));
        let config = |max_size| WalkConfig {
            paths: vec![dir.clone()],
            max_size,
            ..Default::default()
        };
        assert_eq!(names(&config(Some(0))), Vec::<String>::new());
        assert_eq!(
            names(&config(Some(3))),
            ["small.js"],
            "exactly at the limit"
        );
        assert_eq!(names(&config(None)), ["large.js", "small.js"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extension_filter_limits_to_the_listed_formats() {
        let config = WalkConfig {
            paths: vec![fixtures()],
            extensions: vec!["javascript".to_string()],
            ..Default::default()
        };
        assert_eq!(names(&config), ["file1.js"]);
        let config = WalkConfig {
            extensions: vec!["javascript".to_string(), "python".to_string()],
            ..config
        };
        assert_eq!(names(&config), ["file1.js", "file3.py"]);
    }

    #[test]
    fn ignore_glob_pattern_excludes_matching_paths() {
        let config = WalkConfig {
            paths: vec![fixtures()],
            ignore_patterns: vec!["*.js".to_string()],
            ..Default::default()
        };
        assert_eq!(names(&config), ["file2.ts", "file3.py"]);
        // A bare folder name leaves the folder out at any depth.
        let config = WalkConfig {
            ignore_patterns: vec!["**/subdir_a/**".to_string()],
            ..config
        };
        assert_eq!(names(&config), ["file3.py"]);
    }

    #[test]
    fn pattern_with_absolute_path_matches_relative_subdirs() {
        // Issue #811: a relative pattern must match under an absolute root,
        // whose walked paths are absolute.
        let config = WalkConfig {
            paths: vec![std::fs::canonicalize(fixtures()).unwrap()],
            pattern: Some("subdir_a/**/*.js".to_string()),
            ..Default::default()
        };
        assert_eq!(names(&config), ["file1.js"]);
    }

    #[test]
    fn an_absolute_pattern_matches_the_full_path() {
        let root = std::fs::canonicalize(fixtures()).unwrap();
        let pattern = format!("{}/subdir_b/*.py", root.display());
        let config = WalkConfig {
            paths: vec![root],
            pattern: Some(pattern),
            ..Default::default()
        };
        assert_eq!(names(&config), ["file3.py"]);
    }

    #[test]
    fn pattern_star_dot_ts_matches_at_any_depth() {
        let config = WalkConfig {
            paths: vec![fixtures()],
            pattern: Some("*.ts".to_string()),
            ..Default::default()
        };
        assert_eq!(names(&config), ["file2.ts"]);
    }

    #[test]
    fn custom_extensions_and_file_names_map_to_formats() {
        let dir = temp_dir("custom-formats");
        write(&dir, "page.vuex", "let x = 1;\n");
        write(&dir, "Jenkinsfile", "pipeline { }\n");
        write(&dir, "plain.js", "let y = 2;\n");
        let config = WalkConfig {
            paths: vec![dir.clone()],
            formats_exts: HashMap::from([("javascript".to_string(), vec!["vuex".to_string()])]),
            formats_names: HashMap::from([("groovy".to_string(), vec!["Jenkinsfile".to_string()])]),
            ..Default::default()
        };
        assert_eq!(
            names_and_formats(&config),
            pairs(&[
                ("Jenkinsfile", "groovy"),
                ("page.vuex", "javascript"),
                ("plain.js", "javascript"),
            ])
        );
        // A format filter applies to the custom mappings too.
        let config = WalkConfig {
            extensions: vec!["groovy".to_string()],
            ..config
        };
        assert_eq!(names(&config), ["Jenkinsfile"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_without_extension_is_recognised_by_its_shebang() {
        let dir = temp_dir("shebang");
        write(&dir, "deploy", "#!/usr/bin/env python3\nprint('hi')\n");
        write(&dir, "notes", "just some text\n");
        let found = names_and_formats(&WalkConfig {
            paths: vec![dir.clone()],
            ..Default::default()
        });
        assert_eq!(found, pairs(&[("deploy", "python")]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_answers_as_the_walk_does() {
        let dir = temp_dir("accepts");
        write(&dir, ".ignore", "generated/\n");
        let app = write(&dir, "src/app.ts", "let x = 1;\n");
        let gen_file = write(&dir, "generated/out.ts", "let x = 1;\n");
        let big = write(&dir, "src/big.ts", &"let x = 1;\n".repeat(50));
        let root = std::fs::canonicalize(&dir).unwrap();
        let at = |p: &Path| root.join(p.strip_prefix(&dir).unwrap());
        let config = WalkConfig {
            max_size: Some(100),
            ..Default::default()
        };

        assert_eq!(
            accepts(&at(&app), &root, &config).as_deref(),
            Some("typescript")
        );
        assert_eq!(
            accepts(&at(&gen_file), &root, &config),
            None,
            "an .ignore file"
        );
        assert_eq!(accepts(&at(&big), &root, &config), None, "over max_size");
        assert_eq!(
            accepts(&root.join("README"), &root, &config),
            None,
            "no format"
        );
        // A file the editor has open but which is not on disk yet.
        assert_eq!(
            accepts(&root.join("src/new.ts"), &root, &config).as_deref(),
            Some("typescript")
        );

        let only_js = WalkConfig {
            pattern: Some("*.js".to_string()),
            ..Default::default()
        };
        assert_eq!(accepts(&at(&app), &root, &only_js), None, "pattern");
        let ignore_src = WalkConfig {
            ignore_patterns: vec!["src/**".to_string()],
            ..Default::default()
        };
        assert_eq!(accepts(&at(&app), &root, &ignore_src), None, "--ignore");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
