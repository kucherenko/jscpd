//! The dead-code section of a jscpd config file.
//!
//! A project that already keeps a `.jscpd.json` should not need a second
//! file, and should not have to repeat on every command line what it said
//! once. The section is one object under `deadCode` — or `dead-code`, or
//! `basta`; they are the same key — and both front ends read it the same way:
//!
//! ```json
//! {
//!   "threshold": 5,
//!   "deadCode": {
//!     "minConfidence": 80,
//!     "entry": ["src/handlers/**"],
//!     "framework": ["next"],
//!     "frameworks": [{ "name": "kiosk-router", "detect": { "packageJsonKeys": ["kioskRouter"] },
//!                      "entry": ["screens/**/*.screen.js"] }]
//!   }
//! }
//! ```
//!
//! Everything in it has a flag of the same name, and the flag wins. The
//! struct lives here rather than in jscpd so that the two readers cannot
//! drift: jscpd embeds it in its own config type, the `basta` binary finds
//! the file by the same cascade jscpd uses and reads this one key out of it.

use crate::framework::Framework;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The names the section goes by in a config file. `deadCode` is the
/// spelling jscpd's other keys use; `dead-code` is the flag's; `basta` is the
/// tool's.
pub const KEYS: &[&str] = &["deadCode", "dead-code", "basta"];

/// Where a jscpd config is looked for, in order, under the working
/// directory. The last one holds it under a `jscpd` key.
const CONFIG_FILES: &[&str] = &[".jscpd.json", ".config/jscpd.json", ".config/.jscpd.json"];
const MANIFEST: &str = "package.json";
const MANIFEST_KEY: &str = "jscpd";

/// Dead-code settings a project keeps in its jscpd config.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Section {
    /// Run dead-code detection when jscpd is started with no mode flag. The
    /// `basta` binary has no other mode, so it does not look at this.
    pub enabled: Option<bool>,
    /// Findings to report, by the names `--categories` takes, or `all`.
    pub categories: Option<Vec<String>>,
    #[serde(alias = "min-confidence")]
    pub min_confidence: Option<u8>,
    /// The smallest declaration worth reporting. Inside this section the key
    /// is unambiguous; at the top level of a jscpd config `minLines` is about
    /// clones.
    #[serde(alias = "min-lines")]
    pub min_lines: Option<u32>,
    pub entry: Option<Vec<String>>,
    /// Globs to skip in a dead-code run, on top of any the run already has.
    pub ignore: Option<Vec<String>>,
    #[serde(alias = "include-tests")]
    pub include_tests: Option<bool>,
    #[serde(alias = "include-entry-exports")]
    pub include_entry_exports: Option<bool>,
    /// Fail when dead code exceeds this share of the codebase. Separate from
    /// jscpd's own `threshold`, which is a budget for duplication: the two
    /// percentages measure different things and rarely want the same number.
    pub threshold: Option<f64>,
    /// Framework definitions, in the shape of `frameworks.yaml`, added to the
    /// built-in ones or replacing them by name.
    pub frameworks: Option<Vec<Framework>>,
    /// A file of such definitions, instead of `basta.frameworks.*`.
    #[serde(alias = "frameworks-config")]
    pub frameworks_config: Option<PathBuf>,
    /// Frameworks to take as present at the scan roots.
    pub framework: Option<Vec<String>>,
    #[serde(alias = "no-frameworks")]
    pub no_frameworks: Option<bool>,
}

impl Section {
    /// The section as it is written: the object under one of [`KEYS`].
    pub fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())
    }

    /// The section of a whole jscpd config, when it has one.
    ///
    /// `"deadCode": true` is the older, shorter meaning of the key — turn the
    /// mode on — and carries no settings, so it reads as no section at all.
    pub fn of_config(config: &serde_json::Value) -> Result<Option<Self>, String> {
        let Some((key, value)) = KEYS.iter().find_map(|key| Some((key, config.get(key)?))) else {
            return Ok(None);
        };
        match value {
            serde_json::Value::Bool(_) | serde_json::Value::Null => Ok(None),
            value => Self::from_value(value)
                .map(Some)
                .map_err(|error| format!("\"{key}\": {error}")),
        }
    }

    /// The section of the config file at `path`, with the path in any error.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let config: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::of_config(&config).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The section of whichever jscpd config `directory` holds, with the file
    /// it came from. The same cascade jscpd walks, so the two tools started
    /// in one directory read one file.
    pub fn discover(directory: &Path) -> Result<Option<(PathBuf, Self)>, String> {
        for name in CONFIG_FILES {
            let path = directory.join(name);
            if path.is_file() {
                return Ok(Self::load(&path)?.map(|section| (path, section)));
            }
        }
        let path = directory.join(MANIFEST);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        // A manifest that does not parse is somebody else's problem to
        // report; it says nothing about dead code either way.
        let Some(config) = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|manifest| manifest.get(MANIFEST_KEY).cloned())
        else {
            return Ok(None);
        };
        Self::of_config(&config)
            .map(|section| section.map(|section| (path.clone(), section)))
            .map_err(|error| format!("{}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scan::TempTree;

    #[test]
    fn the_section_goes_by_any_of_its_three_names() {
        for key in KEYS {
            let config = serde_json::json!({ *key: { "minConfidence": 80 } });
            let section = Section::of_config(&config).unwrap().expect(key);
            assert_eq!(section.min_confidence, Some(80), "{key}");
        }
        assert_eq!(
            Section::of_config(&serde_json::json!({ "threshold": 5 })).unwrap(),
            None
        );
    }

    #[test]
    fn the_boolean_form_of_the_key_is_a_switch_not_a_section() {
        for config in [
            serde_json::json!({ "deadCode": true }),
            serde_json::json!({ "basta": false }),
        ] {
            assert_eq!(Section::of_config(&config).unwrap(), None);
        }
    }

    #[test]
    fn every_setting_has_a_key_and_kebab_case_is_accepted() {
        let section = Section::from_value(&serde_json::json!({
            "enabled": true,
            "categories": ["unused-file", "exports"],
            "min-confidence": 70,
            "minLines": 3,
            "entry": ["src/handlers/**"],
            "ignore": ["**/generated/**"],
            "include-tests": true,
            "includeEntryExports": true,
            "threshold": 2.5,
            "frameworks": [{ "name": "house", "detect": { "dependencies": ["house"] }, "globals": ["onBoot"] }],
            "frameworksConfig": "tools/frameworks.yaml",
            "framework": ["next"],
            "no-frameworks": false
        }))
        .unwrap();
        assert_eq!(section.enabled, Some(true));
        assert_eq!(section.min_confidence, Some(70));
        assert_eq!(section.min_lines, Some(3));
        assert_eq!(section.include_tests, Some(true));
        assert_eq!(section.threshold, Some(2.5));
        assert_eq!(section.frameworks.as_ref().unwrap()[0].name, "house");
        assert_eq!(
            section.frameworks_config.as_deref(),
            Some(Path::new("tools/frameworks.yaml"))
        );
        assert_eq!(section.no_frameworks, Some(false));
    }

    #[test]
    fn a_misspelled_key_is_refused_by_name() {
        let error = Section::of_config(&serde_json::json!({ "deadCode": { "minConfidense": 80 } }))
            .unwrap_err();
        assert!(error.contains("\"deadCode\""), "{error}");
        assert!(error.contains("minConfidense"), "{error}");
    }

    #[test]
    fn the_config_is_found_where_jscpd_finds_it() {
        let tree = TempTree::new("section-discover");
        assert_eq!(Section::discover(tree.path()).unwrap(), None);

        // Last in the cascade: the manifest's `jscpd` key.
        tree.write(
            "package.json",
            r#"{ "name": "app", "jscpd": { "dead-code": { "minConfidence": 90 } } }"#,
        );
        let (path, section) = Section::discover(tree.path()).unwrap().unwrap();
        assert_eq!(path, tree.path().join("package.json"));
        assert_eq!(section.min_confidence, Some(90));

        // `.config/jscpd.json` is ahead of it, and `.jscpd.json` ahead of both.
        tree.write(
            ".config/jscpd.json",
            r#"{ "basta": { "minConfidence": 70 } }"#,
        );
        let (_, section) = Section::discover(tree.path()).unwrap().unwrap();
        assert_eq!(section.min_confidence, Some(70));
        tree.write(".jscpd.json", r#"{ "threshold": 5 }"#);
        assert_eq!(
            Section::discover(tree.path()).unwrap(),
            None,
            "the first file found is the config, section or no section"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_says_which_one() {
        let tree = TempTree::new("section-broken");
        tree.write(".jscpd.json", "{ not json");
        let error = Section::discover(tree.path()).unwrap_err();
        assert!(error.contains(".jscpd.json"), "{error}");
        let missing = Section::load(&tree.path().join("nope.json")).unwrap_err();
        assert!(missing.contains("nope.json"), "{missing}");
    }
}
