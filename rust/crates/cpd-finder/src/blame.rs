use cpd_core::models::{BlameEntry, CpdClone};
use std::collections::HashMap;
use std::path::Path;

pub type BlameMap = HashMap<String, HashMap<u32, (String, String, i64)>>;

/// Strip the `:format` suffix from multi-format source IDs.
/// e.g. "file1.md:markdown" -> "file1.md"
fn clean_source_id(source_id: &str) -> &str {
    match source_id.rfind(':') {
        Some(pos) => &source_id[..pos],
        None => source_id,
    }
}

/// Run `git blame --porcelain` on a file and collect per-line (commit_sha, author, timestamp) data.
/// Returns a map from 1-based line number to blame info.
fn blame_file(file_path: &str, repo_root: &Path) -> Option<HashMap<u32, (String, String, i64)>> {
    let clean_path = clean_source_id(file_path);
    let absolute = std::path::Path::new(clean_path).canonicalize().ok()?;
    let relative = absolute.strip_prefix(repo_root.canonicalize().ok()?).ok()?;
    let output = std::process::Command::new("git")
        .args(["blame", "--porcelain", "--", &relative.to_string_lossy()])
        .current_dir(repo_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut result: HashMap<u32, (String, String, i64)> = HashMap::new();

    let mut sha = String::new();
    let mut author = String::new();
    let mut timestamp: i64 = 0;
    let mut result_line: u32 = 0;

    for line in stdout.lines() {
        if line.starts_with('\t') {
            if !sha.is_empty() && result_line > 0 {
                result.insert(result_line, (sha.clone(), author.clone(), timestamp));
            }
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            timestamp = rest.parse().unwrap_or(0);
        } else if line.starts_with("summary ")
            || line.starts_with("author-mail")
            || line.starts_with("author-tz")
            || line.starts_with("committer")
            || line.starts_with("previous")
            || line.starts_with("filename")
        {
            // skip metadata lines
        } else {
            // Header line format: sha orig_line result_line [num_lines]
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() >= 3 {
                sha = parts[0].to_string();
                if sha.len() > 40 {
                    sha.truncate(40);
                }
                result_line = parts[2].parse().unwrap_or(0);
            }
        }
    }

    Some(result)
}

/// Enrich clone fragments with git blame data.
/// Uses `git blame --porcelain` to get per-line author info.
/// Safe to call on non-git directories (returns early).
/// Returns a BlameMap with per-file per-line blame data for use by reporters.
pub fn enrich(clones: &mut [CpdClone], repo_root: &Path) -> BlameMap {
    if std::process::Command::new("git")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        return HashMap::new();
    }

    let mut files_to_blame: BlameMap = HashMap::new();

    for clone in clones.iter() {
        for frag in [&clone.fragment_a, &clone.fragment_b] {
            let clean = clean_source_id(&frag.source_id).to_string();
            if files_to_blame.contains_key(&clean) {
                continue;
            }
            if let Some(blame_data) = blame_file(&clean, repo_root) {
                files_to_blame.insert(clean.clone(), blame_data);
            } else {
                files_to_blame.insert(clean.clone(), HashMap::new());
            }
        }
    }

    for clone in clones.iter_mut() {
        for frag in [&mut clone.fragment_a, &mut clone.fragment_b] {
            let clean = clean_source_id(&frag.source_id).to_string();
            if let Some(blame_data) = files_to_blame.get(&clean) {
                let line = frag.start.line;
                if let Some((sha, author, timestamp)) = blame_data.get(&line) {
                    frag.blame = Some(BlameEntry {
                        commit_sha: sha.clone(),
                        author: author.clone(),
                        timestamp: *timestamp,
                    });
                }
            }
        }
    }

    files_to_blame
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{CpdClone, Fragment, Location};

    fn make_clone(source_id: &str, start_line: u32) -> CpdClone {
        let loc = Location {
            line: start_line,
            column: 0,
            offset: 0,
        };
        let frag = Fragment {
            source_id: source_id.to_string(),
            start: loc.clone(),
            end: loc,
            range: [0, 10],
            blame: None,
        };
        CpdClone {
            format: "rust".to_string(),
            fragment_a: frag.clone(),
            fragment_b: frag,
            token_count: 20,
        }
    }

    #[test]
    fn non_git_directory_does_not_panic() {
        let mut clones = vec![make_clone("/tmp/a.rs", 1)];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            enrich(&mut clones, Path::new("/tmp"));
        }));
        assert!(result.is_ok(), "enrich on non-git dir must not panic");
    }

    #[test]
    fn empty_clones_does_not_panic() {
        let mut clones: Vec<CpdClone> = vec![];
        enrich(&mut clones, Path::new("/tmp"));
        assert!(clones.is_empty());
    }

    #[test]
    fn git_repo_does_not_panic() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut clones = vec![make_clone("rust/crates/cpd-finder/src/blame.rs", 1)];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            enrich(&mut clones, repo_root);
        }));
        assert!(result.is_ok(), "enrich on git repo must not panic");
    }

    #[test]
    fn clean_source_id_strips_format_suffix() {
        assert_eq!(clean_source_id("file.md:markdown"), "file.md");
        assert_eq!(clean_source_id("file.rs"), "file.rs");
        assert_eq!(
            clean_source_id("path/to/file.tsx:typescript"),
            "path/to/file.tsx"
        );
    }
}
