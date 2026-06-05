// Attribution: git blame enrichment using gix pure-Rust implementation; rewritten independently.

use cpd_core::models::CpdClone;
use std::path::Path;

/// Enrich clone fragments with git blame data.
/// Runs serially. Safe to call on non-git directories (returns early).
pub fn enrich(clones: &mut Vec<CpdClone>, repo_root: &Path) {
    // Attempt to open repo. If not a git repo, return immediately (no error).
    let _repo = match gix::open(repo_root) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Blame enrichment: in V1, leave blame as None for all fragments.
    // The gix blame API is complex and version-sensitive.
    // Full blame will be wired in a follow-up.
    // This stub proves the gix dependency works and the function is callable.
    //
    // fragment.blame remains None for all fragments (V1 limitation).
    let _ = clones;
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{CpdClone, Fragment, Location};

    fn make_clone(source_id: &str) -> CpdClone {
        let loc = Location {
            line: 5,
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
        let mut clones = vec![make_clone("/tmp/a.rs")];
        // /tmp is not a git repo — must return without panicking
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
    fn git_repo_opens_without_panic() {
        // The jscpd repo itself is a git repo
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // cpd-finder/
            .parent()
            .unwrap() // crates/
            .parent()
            .unwrap(); // rust/
        let parent = repo_root.parent().unwrap(); // jscpd/

        let mut clones = vec![make_clone("rust/crates/cpd-finder/src/blame.rs")];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            enrich(&mut clones, parent);
        }));
        assert!(result.is_ok(), "enrich on git repo must not panic");
    }
}
