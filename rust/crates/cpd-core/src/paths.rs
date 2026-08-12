// paths.rs — shared source-id path helpers.
//
// Source ids may carry a `:format` suffix (multi-format files, e.g.
// `README.md:javascript`) and, since scan-root relativization, fragments may
// store their scan root separately in `Fragment.source_root`. These helpers
// are the single source of truth for turning a fragment back into a real
// filesystem path; cpd-finder (blame) and cpd-reporter both rely on them.

use crate::models::Fragment;

/// Strip a `:format` suffix from a source id so it can be used as a real path.
///
/// Format-qualified IDs look like `README.md:javascript`. A bare colon inside
/// a Windows drive letter (`C:\…`) or after a path separator is NOT a format
/// suffix — only a colon preceded by a non-separator, non-colon char with a
/// valid format name after it qualifies.
pub fn clean_source_id(source_id: &str) -> &str {
    match source_id.rfind(':') {
        Some(pos) if pos > 0 => {
            let before = source_id.as_bytes()[pos - 1];
            // A colon right after a single drive letter (e.g. `C:`) or after
            // a path separator (`/`, `\`) is structural, not a format suffix.
            if pos == 1 && before.is_ascii_alphabetic() {
                return source_id;
            }
            if before == b'/' || before == b'\\' {
                return source_id;
            }
            &source_id[..pos]
        }
        _ => source_id,
    }
}

/// Resolve a fragment's filesystem path by joining `source_root` (if set) with
/// the cleaned `source_id`. Falls back to the bare `source_id` when no root is
/// stored (absolute paths, `--absolute` mode, or legacy data).
pub fn resolve_fragment_path(fragment: &Fragment) -> String {
    let clean = clean_source_id(&fragment.source_id);
    match &fragment.source_root {
        Some(root) => {
            let joined = std::path::Path::new(root).join(clean);
            joined.to_string_lossy().into_owned()
        }
        None => clean.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Location;

    fn frag(source_id: &str, source_root: Option<&str>) -> Fragment {
        let loc = Location {
            line: 1,
            column: 0,
            offset: 0,
        };
        Fragment {
            source_id: source_id.to_string(),
            source_root: source_root.map(str::to_string),
            start: loc.clone(),
            end: loc,
            range: [0, 0],
            blame: None,
        }
    }

    #[test]
    fn clean_source_id_strips_format_suffix() {
        assert_eq!(clean_source_id("README.md:javascript"), "README.md");
    }

    #[test]
    fn clean_source_id_preserves_bare_path() {
        assert_eq!(clean_source_id("src/a.js"), "src/a.js");
    }

    #[test]
    fn clean_source_id_preserves_windows_drive() {
        assert_eq!(clean_source_id(r"C:\scan\a.rs"), r"C:\scan\a.rs");
    }

    #[test]
    fn clean_source_id_strips_format_from_windows_path() {
        assert_eq!(clean_source_id(r"C:\scan\a.md:javascript"), r"C:\scan\a.md");
    }

    #[test]
    fn resolve_fragment_path_joins_root() {
        assert_eq!(
            resolve_fragment_path(&frag("src/a.js", Some("/project"))),
            "/project/src/a.js"
        );
    }

    #[test]
    fn resolve_fragment_path_falls_back_to_source_id() {
        assert_eq!(
            resolve_fragment_path(&frag("/absolute/path/a.js", None)),
            "/absolute/path/a.js"
        );
    }

    #[test]
    fn resolve_fragment_path_cleans_format_suffix() {
        assert_eq!(
            resolve_fragment_path(&frag("doc.md:javascript", Some("/repo"))),
            "/repo/doc.md"
        );
    }
}
