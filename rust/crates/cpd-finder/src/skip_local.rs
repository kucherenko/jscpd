// skip_local.rs
// Filters out clone pairs where both fragments share the same parent directory.

use std::path::Path;
use cpd_core::models::CpdClone;

pub fn apply_skip_local(clones: Vec<CpdClone>) -> Vec<CpdClone> {
    clones.into_iter().filter(|c| {
        let dir_a = Path::new(&c.fragment_a.source_id).parent();
        let dir_b = Path::new(&c.fragment_b.source_id).parent();
        // Keep the clone only if fragments are in DIFFERENT directories
        dir_a != dir_b
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{CpdClone, Fragment, Location};

    fn make_clone(a: &str, b: &str) -> CpdClone {
        let loc = Location { line: 1, column: 0, offset: 0 };
        let frag = |id: &str| Fragment {
            source_id: id.to_string(),
            start: loc.clone(),
            end: loc.clone(),
            range: [0, 5],
            blame: None,
        };
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: frag(a),
            fragment_b: frag(b),
            token_count: 50,
        }
    }

    #[test]
    fn same_dir_pair_removed() {
        let clones = vec![make_clone("src/a.js", "src/b.js")];
        let filtered = apply_skip_local(clones);
        assert!(filtered.is_empty(), "same-directory pair must be removed");
    }

    #[test]
    fn different_dir_pair_kept() {
        let clones = vec![make_clone("src/a.js", "lib/b.js")];
        let filtered = apply_skip_local(clones);
        assert_eq!(filtered.len(), 1, "different-directory pair must be kept");
    }

    #[test]
    fn root_level_files_same_dir_removed() {
        let clones = vec![make_clone("a.js", "b.js")];
        let filtered = apply_skip_local(clones);
        assert!(filtered.is_empty(), "root-level same-dir pair must be removed");
    }

    #[test]
    fn empty_input_returns_empty() {
        let filtered = apply_skip_local(vec![]);
        assert!(filtered.is_empty());
    }
}
