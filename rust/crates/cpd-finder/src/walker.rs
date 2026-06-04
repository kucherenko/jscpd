// walker.rs
// Attribution: file discovery with gitignore support; inspired by jscpd-rs approach; rewritten independently.

use std::{fs, path::{Path, PathBuf}};
use ignore::WalkBuilder;

#[derive(Debug, Clone)]
pub struct WalkConfig {
    pub paths: Vec<PathBuf>,
    pub extensions: Vec<String>,        // empty = all supported formats
    pub ignore_patterns: Vec<String>,
    pub max_size: Option<u64>,
    pub min_lines: Option<usize>,
    pub max_lines: Option<usize>,
    pub follow_symlinks: bool,
    pub no_gitignore: bool,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            paths: vec![],
            extensions: vec![],
            ignore_patterns: vec![],
            max_size: Some(512 * 1024),
            min_lines: None,
            max_lines: None,
            follow_symlinks: false,
            no_gitignore: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub format: String,
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

    for entry in builder.build().flatten() {
        let path = entry.path().to_path_buf();
        if !path.is_file() { continue; }

        // Skip symlinks if not following
        if !config.follow_symlinks {
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if meta.file_type().is_symlink() { continue; }
            }
        }

        // Size limit
        if let Some(max) = config.max_size {
            if let Ok(meta) = fs::metadata(&path) {
                if meta.len() > max { continue; }
            }
        }

        // Format detection
        let format = match detect_format(&path, &config.extensions) {
            Some(f) => f,
            None => continue,
        };

        // Ignore patterns (manual glob-style: path contains or ends with pattern)
        if config.ignore_patterns.iter().any(|p| {
            let p = p.trim_start_matches('/');
            path.to_string_lossy().contains(p) || path.ends_with(p)
        }) {
            continue;
        }

        // Line count checks
        if config.min_lines.is_some() || config.max_lines.is_some() {
            if let Ok(content) = fs::read_to_string(&path) {
                let lc = content.lines().count();
                if config.min_lines.map_or(false, |m| lc < m) { continue; }
                if config.max_lines.map_or(false, |m| lc > m) { continue; }
            }
        }

        results.push(DiscoveredFile { path, format });
    }
}

fn detect_format(path: &Path, filter: &[String]) -> Option<String> {
    let fmt = path.extension()
        .and_then(|e| e.to_str())
        .and_then(|ext| cpd_tokenizer::formats::get_format_by_extension(ext))
        .map(|s| s.to_string())
        .or_else(|| {
            fs::read_to_string(path).ok().and_then(|c| {
                c.lines().next()
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
        if !dir.exists() { return; }
        let config = WalkConfig { paths: vec![dir], ..Default::default() };
        let files = walk(&config);
        assert!(files.iter().any(|f| f.format == "javascript"), "must find JS");
        assert!(files.iter().any(|f| f.format == "typescript"), "must find TS");
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
        if !dir.exists() { return; }
        let config = WalkConfig { paths: vec![dir], max_size: Some(0), ..Default::default() };
        assert!(walk(&config).is_empty(), "max_size=0 must exclude all");
    }

    #[test]
    fn extension_filter_limits_to_js_only() {
        let dir = fixtures();
        if !dir.exists() { return; }
        let config = WalkConfig {
            paths: vec![dir],
            extensions: vec!["javascript".to_string()],
            ..Default::default()
        };
        let files = walk(&config);
        assert!(files.iter().all(|f| f.format == "javascript"),
            "extension filter must return only JS files");
    }
}
