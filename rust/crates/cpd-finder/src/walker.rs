// walker.rs

use std::{collections::HashMap, io::BufRead};

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
    pub path: PathBuf,
    pub format: String,
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
    if !pattern.starts_with('/') && !is_windows_absolute(pattern) {
        let prefixed = format!("**/{}", pattern.trim_start_matches("./"));
        if let Ok(g) = Glob::new(&prefixed) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

#[cfg(not(target_os = "windows"))]
fn is_windows_absolute(_: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn is_windows_absolute(p: &str) -> bool {
    p.chars().next().map_or(false, |c| c.is_ascii_alphabetic())
        && p.starts_with(|c: char| c == ':' || c == '\\')
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
    let mut results = Vec::new();
    for root in &config.paths {
        walk_one(root, config, &mut results);
    }
    results
}

fn walk_one(root: &Path, config: &WalkConfig, results: &mut Vec<DiscoveredFile>) {
    let mut builder = WalkBuilder::new(root);
    builder.follow_links(config.follow_symlinks);
    builder.git_ignore(!config.no_gitignore);
    builder.hidden(false);

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
            if !follow_symlinks {
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink() {
                        return WalkState::Continue;
                    }
                }
            }

            // Size limit check (metadata only — no file read yet).
            if let Some(max) = max_size {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.len() > max {
                        return WalkState::Continue;
                    }
                }
            }

            // Pattern filter: if set, only include files matching the positive glob.
            // Try matching against both the relative path (stripped of root prefix) and
            // the full path so that both relative patterns (e.g. `src/**/*.ts`) and
            // absolute patterns (e.g. `/project/src/**/*.ts`) work correctly.
            if let Some(ref ps) = pattern_set {
                if !ps.is_empty() {
                    let rel = path.strip_prefix(&root_canon).unwrap_or(&path);
                    if !ps.is_match(rel) && !ps.is_match(&path) {
                        return WalkState::Continue;
                    }
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

            let _ = tx.send(DiscoveredFile { path, format });
            WalkState::Continue
        })
    });

    // Drain the channel.
    results.extend(rx);
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

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walker")
    }

    #[test]
    fn walks_js_and_ts_files() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        let config = WalkConfig {
            paths: vec![dir],
            ..Default::default()
        };
        let files = walk(&config);
        assert!(
            files.iter().any(|f| f.format == "javascript"),
            "must find JS"
        );
        assert!(
            files.iter().any(|f| f.format == "typescript"),
            "must find TS"
        );
    }

    #[test]
    fn nonexistent_path_returns_empty() {
        let config = WalkConfig {
            paths: vec![PathBuf::from("/tmp/cpd-nonexistent-xyz")],
            ..Default::default()
        };
        let files = walk(&config);
        assert!(files.is_empty());
    }

    #[test]
    fn max_size_zero_excludes_all_files() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        let config = WalkConfig {
            paths: vec![dir],
            max_size: Some(0),
            ..Default::default()
        };
        assert!(walk(&config).is_empty(), "max_size=0 must exclude all");
    }

    #[test]
    fn extension_filter_limits_to_js_only() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        let config = WalkConfig {
            paths: vec![dir],
            extensions: vec!["javascript".to_string()],
            ..Default::default()
        };
        let files = walk(&config);
        assert!(
            files.iter().all(|f| f.format == "javascript"),
            "extension filter must return only JS files"
        );
    }

    #[test]
    fn ignore_glob_pattern_excludes_matching_paths() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        // Exclude everything with "*.js" — should leave only TS or nothing.
        let config = WalkConfig {
            paths: vec![dir],
            ignore_patterns: vec!["*.js".to_string()],
            ..Default::default()
        };
        let files = walk(&config);
        assert!(
            files.iter().all(|f| f.format != "javascript"),
            "*.js glob pattern must exclude all JS files"
        );
    }

    #[test]
    fn pattern_with_absolute_path_matches_relative_subdirs() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        // Use absolute path for root — the bug scenario from issue #811.
        // The pattern "subdir_a/**/*.js" must match even though the walker
        // returns absolute paths when given an absolute root.
        let config = WalkConfig {
            paths: vec![dir.clone()],
            pattern: Some("subdir_a/**/*.js".to_string()),
            ..Default::default()
        };
        let files = walk(&config);
        assert!(
            !files.is_empty(),
            "relative pattern must match files under absolute root"
        );
        assert!(
            files.iter().all(|f| f.format == "javascript"),
            "pattern must only include JS files in subdir_a"
        );
    }

    #[test]
    fn pattern_star_dot_ts_matches_at_any_depth() {
        let dir = fixtures();
        if !dir.exists() {
            return;
        }
        let config = WalkConfig {
            paths: vec![dir],
            pattern: Some("*.ts".to_string()),
            ..Default::default()
        };
        let files = walk(&config);
        assert!(
            !files.is_empty(),
            "*.ts pattern must match TS files at any depth"
        );
        assert!(
            files.iter().all(|f| f.format == "typescript"),
            "*.ts pattern must only match TS files"
        );
    }
}
