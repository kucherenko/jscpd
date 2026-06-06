// walker.rs

use std::collections::HashMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct WalkConfig {
    pub paths: Vec<PathBuf>,
    pub extensions: Vec<String>, // empty = all supported formats
    pub ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub min_lines: Option<usize>,
    pub max_lines: Option<usize>,
    pub follow_symlinks: bool,
    pub no_gitignore: bool,
    pub formats_exts: HashMap<String, Vec<String>>,
    pub formats_names: HashMap<String, Vec<String>>,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            paths: vec![],
            extensions: vec![],
            ignore_patterns: vec![],
            max_size: None,
            min_lines: None,
            max_lines: None,
            follow_symlinks: false,
            no_gitignore: false,
            formats_exts: HashMap::new(),
            formats_names: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub format: String,
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

    // Use mpsc::channel for collection — cheaper than Arc<Mutex<Vec>> under parallelism.
    let (tx, rx) = mpsc::channel::<DiscoveredFile>();

    let follow_symlinks = config.follow_symlinks;
    let max_size = config.max_size;
    let min_lines = config.min_lines;
    let max_lines = config.max_lines;
    let extensions = config.extensions.clone();
    let formats_exts = config.formats_exts.clone();
    let formats_names = config.formats_names.clone();

    builder.build_parallel().run(move || {
        let tx = tx.clone();
        let extensions = extensions.clone();
        let ignore_set = ignore_set.clone();
        let formats_exts = formats_exts.clone();
        let formats_names = formats_names.clone();

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

            // Format detection.
            let format = match detect_format(&path, &extensions, &formats_exts, &formats_names) {
                Some(f) => f,
                None => return WalkState::Continue,
            };

            // Ignore patterns — pre-compiled GlobSet (correctness + speed vs substring).
            if !ignore_set.is_empty() && ignore_set.is_match(&path) {
                return WalkState::Continue;
            }

            // Line count check — read the file ONCE here if needed.
            // Previous implementation read the file twice when min_lines/max_lines was set.
            if min_lines.is_some() || max_lines.is_some() {
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        // Count newlines in already-read bytes — no second syscall.
                        let lc = count_lines(&bytes);
                        if min_lines.is_some_and(|m| lc < m) {
                            return WalkState::Continue;
                        }
                        if max_lines.is_some_and(|m| lc > m) {
                            return WalkState::Continue;
                        }
                    }
                    Err(_) => return WalkState::Continue,
                }
            }

            let _ = tx.send(DiscoveredFile { path, format });
            WalkState::Continue
        })
    });

    // Drain the channel.
    results.extend(rx);
}

/// Count lines by counting newline bytes — O(n) in bytes, no UTF-8 decode.
fn count_lines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == b'\n').count()
}

fn detect_format(
    path: &Path,
    filter: &[String],
    formats_exts: &HashMap<String, Vec<String>>,
    formats_names: &HashMap<String, Vec<String>>,
) -> Option<String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Priority 1: check formats_names (filename-based matching)
    if !formats_names.is_empty() {
        for (format, names) in formats_names {
            if names.iter().any(|n| n == file_name) {
                let fmt = format.clone();
                if filter.is_empty() || filter.iter().any(|e| e == &fmt) {
                    return Some(fmt);
                }
            }
        }
    }

    // Priority 2: check formats_exts (extension-based matching)
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !formats_exts.is_empty() && !ext.is_empty() {
        for (format, exts) in formats_exts {
            if exts.iter().any(|e| e == ext) {
                let fmt = format.clone();
                if filter.is_empty() || filter.iter().any(|e| e == &fmt) {
                    return Some(fmt);
                }
            }
        }
    }

    // Priority 3: built-in format detection
    let fmt = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(|e| cpd_tokenizer::formats::get_format_by_extension(e))
        .map(|s| s.to_string())
        .or_else(|| {
            std::fs::read_to_string(path).ok().and_then(|c| {
                c.lines()
                    .next()
                    .filter(|l| l.starts_with("#!"))
                    .and_then(|l| cpd_tokenizer::formats::get_format_by_shebang(l))
                    .map(|s| s.to_string())
            })
        })?;

    if !filter.is_empty() && !filter.iter().any(|e| e == &fmt) {
        return None;
    }
    Some(fmt)
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
    fn count_lines_counts_newlines() {
        assert_eq!(count_lines(b"a\nb\nc\n"), 3);
        assert_eq!(count_lines(b"a\nb\nc"), 2);
        assert_eq!(count_lines(b""), 0);
    }
}
