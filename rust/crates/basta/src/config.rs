//! Run configuration: what to scan, what to report, how strict to be.

use std::collections::HashMap;
use std::path::PathBuf;

use cpd_core::deadcode::Category;

/// Configuration for one dead-code run.
#[derive(Debug, Clone)]
pub struct BastaConfig {
    /// Roots to scan.
    pub paths: Vec<PathBuf>,
    /// Categories to report. Empty means [`Category::DEFAULT`].
    pub categories: Vec<Category>,
    /// Findings below this confidence are dropped. 0 reports everything.
    pub min_confidence: u8,
    /// Extra entry-point globs beyond the ones detected from the project
    /// manifests and conventional layout.
    pub entry: Vec<String>,
    /// Glob patterns of files to skip entirely.
    pub ignore: Vec<String>,
    /// Report dead code inside test, fixture and example files.
    ///
    /// Tests are entry points either way — a test runner starts them, so a
    /// test file is never "unused". This decides whether the dead code
    /// *inside* one is worth saying out loud, and it is off by default
    /// because it buries the findings in shipped code.
    pub include_tests: bool,
    /// Report exports of entry-point files.
    ///
    /// An entry point's exports are the project's public surface: nothing
    /// inside the scan is supposed to import them, so reporting them by
    /// default would flag every published API as dead.
    pub include_entry_exports: bool,
    /// Report symbols shorter than this many lines. Long-dead functions are
    /// worth more than one-line constants; 0 reports everything.
    pub min_lines: u32,
    /// Do not consult `.gitignore` while walking.
    pub no_gitignore: bool,
    /// Follow symbolic links while walking.
    pub follow_symlinks: bool,
    /// Skip files larger than this many bytes.
    pub max_size: Option<u64>,
    /// Rayon worker count; `None` uses the default.
    pub workers: Option<usize>,
    /// Restrict the scan to these jscpd format names. Empty means every
    /// format basta supports.
    pub formats: Vec<String>,
    /// Extra extension → format mappings, mirroring jscpd's `--formats-exts`.
    pub formats_exts: HashMap<String, Vec<String>>,
}

impl Default for BastaConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            categories: Category::DEFAULT.to_vec(),
            min_confidence: 60,
            entry: Vec::new(),
            ignore: Vec::new(),
            include_tests: false,
            include_entry_exports: false,
            min_lines: 0,
            no_gitignore: false,
            follow_symlinks: false,
            max_size: None,
            workers: None,
            formats: Vec::new(),
            formats_exts: HashMap::new(),
        }
    }
}

impl BastaConfig {
    /// The categories this run reports, resolving the empty default.
    pub fn active_categories(&self) -> &[Category] {
        if self.categories.is_empty() {
            Category::DEFAULT
        } else {
            &self.categories
        }
    }

    pub fn reports(&self, category: Category) -> bool {
        self.active_categories().contains(&category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_parses_every_spelling() {
        for (input, want) in [
            ("unused-export", Category::UnusedExport),
            ("unusedExports", Category::UnusedExport),
            ("unused_exports", Category::UnusedExport),
            ("exports", Category::UnusedExport),
            ("  FILES  ", Category::UnusedFile),
            ("unused-member", Category::UnusedMember),
        ] {
            assert_eq!(input.parse::<Category>().unwrap(), want, "input {input}");
        }
    }

    #[test]
    fn category_rejects_unknown() {
        let err = "unused-everything".parse::<Category>().unwrap_err();
        assert!(err.contains("unknown category"), "{err}");
    }

    #[test]
    fn default_config_omits_members_but_keeps_the_rest() {
        let cfg = BastaConfig::default();
        assert!(cfg.reports(Category::UnusedExport));
        assert!(cfg.reports(Category::UnusedImport));
        assert!(!cfg.reports(Category::UnusedMember));
    }

    #[test]
    fn empty_categories_fall_back_to_default() {
        let cfg = BastaConfig {
            categories: Vec::new(),
            ..BastaConfig::default()
        };
        assert_eq!(cfg.active_categories(), Category::DEFAULT);
    }
}
