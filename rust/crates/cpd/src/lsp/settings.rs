//! Which analyses the server runs, and their options. Three places switch
//! an analysis on or off, each winning over the one before: `--lsp-analyses`
//! on the command line, the `lsp` section of a project's `.jscpd.json`, and
//! the same section in the editor's settings.

use serde::Deserialize;

/// One analysis of the server. Each has a rule of its own (see
/// `cpd_reporter::rules`) and a switch of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Analysis {
    /// Exact, renamed and near-miss copies (the token passes).
    Clones,
    /// Functions whose syntax trees have the same shape (`--similarity`).
    Ast,
    /// Functions that do the same job, by an embedding model (`--semantic`).
    Semantic,
    /// Unused files, exports, symbols, imports and members (`--dead-code`).
    DeadCode,
    /// Functions and files above a complexity limit.
    Complexity,
}

impl Analysis {
    pub const ALL: [Analysis; 5] = [
        Analysis::Clones,
        Analysis::Ast,
        Analysis::Semantic,
        Analysis::DeadCode,
        Analysis::Complexity,
    ];

    /// The name `--lsp-analyses` takes.
    pub fn name(self) -> &'static str {
        match self {
            Analysis::Clones => "clones",
            Analysis::Ast => "ast",
            Analysis::Semantic => "semantic",
            Analysis::DeadCode => "dead-code",
            Analysis::Complexity => "complexity",
        }
    }
}

/// The analyses `--lsp-analyses` names: a comma-separated list, or `all`.
/// Without the flag, only clones run.
pub fn parse_analyses(names: &[String]) -> Result<Vec<Analysis>, String> {
    if names.is_empty() {
        return Ok(vec![Analysis::Clones]);
    }
    let mut picked = Vec::new();
    for name in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        if name == "all" {
            return Ok(Analysis::ALL.to_vec());
        }
        let analysis = Analysis::ALL
            .into_iter()
            .find(|a| a.name() == name)
            .ok_or_else(|| {
                format!(
                    "unknown analysis '{name}': must be one of: clones, ast, semantic, dead-code, complexity, all"
                )
            })?;
        if !picked.contains(&analysis) {
            picked.push(analysis);
        }
    }
    Ok(picked)
}

/// The `lsp` section of a config file or of the editor's settings.
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LspSection {
    pub clones: Option<CloneSwitch>,
    pub ast: Option<AstSwitch>,
    pub semantic: Option<Switch>,
    pub dead_code: Option<Switch>,
    pub complexity: Option<ComplexitySwitch>,
    /// Publish the findings of every file, not only the open ones, for an
    /// editor that lists the whole project in a problems panel.
    pub all_files: Option<bool>,
}

/// The switch of an analysis without options of its own.
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Switch {
    pub enabled: Option<bool>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloneSwitch {
    pub enabled: Option<bool>,
    /// Clones of at least this many tokens are warnings and smaller ones
    /// information. Without it, every clone is a warning.
    pub warning_tokens: Option<u32>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AstSwitch {
    pub enabled: Option<bool>,
    /// The ratio two functions' syntax trees must share; the `similarity`
    /// key of the config when it is below 1, and 0.85 otherwise.
    pub similarity: Option<f32>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComplexitySwitch {
    pub enabled: Option<bool>,
    /// A function above this complexity gets a diagnostic.
    pub function_limit: Option<u32>,
}

/// The ratio of the ast analysis when nothing sets one: near-identical
/// structure, the value the docs give for `--similarity`.
pub const DEFAULT_AST_SIMILARITY: f32 = 0.85;
/// The complexity above which a function gets a diagnostic.
pub const DEFAULT_FUNCTION_LIMIT: u32 = 15;

/// What one project runs: the analyses that are on, and their options.
#[derive(Debug, Clone, PartialEq)]
pub struct Analyses {
    pub on: Vec<Analysis>,
    pub warning_tokens: Option<u32>,
    pub ast_similarity: f32,
    pub function_limit: u32,
    pub all_files: bool,
}

impl Analyses {
    /// The analyses of a project: `defaults` from the command line, with the
    /// switches of `section` (the config file with the editor's settings
    /// merged on top) winning over them. `similarity` is the config's
    /// `--similarity` ratio.
    pub fn resolve(defaults: &[Analysis], section: Option<&LspSection>, similarity: f32) -> Self {
        let section = section.cloned().unwrap_or_default();
        let switched = |analysis: Analysis| -> Option<bool> {
            match analysis {
                Analysis::Clones => section.clones.as_ref().and_then(|s| s.enabled),
                Analysis::Ast => section.ast.as_ref().and_then(|s| s.enabled),
                Analysis::Semantic => section.semantic.as_ref().and_then(|s| s.enabled),
                Analysis::DeadCode => section.dead_code.as_ref().and_then(|s| s.enabled),
                Analysis::Complexity => section.complexity.as_ref().and_then(|s| s.enabled),
            }
        };
        let on = Analysis::ALL
            .into_iter()
            .filter(|&a| switched(a).unwrap_or(defaults.contains(&a)))
            .collect();
        let ast_similarity = section
            .ast
            .as_ref()
            .and_then(|s| s.similarity)
            .filter(|s| *s > 0.0 && *s <= 1.0)
            .unwrap_or(match similarity > 0.0 && similarity < 1.0 {
                true => similarity,
                false => DEFAULT_AST_SIMILARITY,
            });
        Self {
            on,
            warning_tokens: section.clones.as_ref().and_then(|s| s.warning_tokens),
            ast_similarity,
            function_limit: section
                .complexity
                .as_ref()
                .and_then(|s| s.function_limit)
                .unwrap_or(DEFAULT_FUNCTION_LIMIT),
            all_files: section.all_files.unwrap_or(false),
        }
    }

    pub fn has(&self, analysis: Analysis) -> bool {
        self.on.contains(&analysis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_clones_run_without_the_flag() {
        assert_eq!(parse_analyses(&[]).unwrap(), [Analysis::Clones]);
        assert_eq!(parse_analyses(&["all".into()]).unwrap(), Analysis::ALL);
        assert_eq!(
            parse_analyses(&["dead-code".into(), "ast".into(), "ast".into()]).unwrap(),
            [Analysis::DeadCode, Analysis::Ast]
        );
        assert!(parse_analyses(&["dead_code".into()]).is_err());
    }

    #[test]
    fn a_config_switch_wins_over_the_command_line() {
        let section: LspSection = serde_json::from_str(
            r#"{"clones": {"enabled": false}, "complexity": {"enabled": true, "functionLimit": 20}}"#,
        )
        .unwrap();
        let analyses = Analyses::resolve(&[Analysis::Clones, Analysis::Ast], Some(&section), 1.0);
        assert_eq!(analyses.on, [Analysis::Ast, Analysis::Complexity]);
        assert_eq!(analyses.function_limit, 20);
        assert_eq!(analyses.ast_similarity, DEFAULT_AST_SIMILARITY);
    }

    #[test]
    fn the_ast_ratio_comes_from_the_section_then_the_similarity_key() {
        let none = Analyses::resolve(&[], None, 0.7);
        assert_eq!(none.ast_similarity, 0.7);
        let section: LspSection = serde_json::from_str(r#"{"ast": {"similarity": 0.9}}"#).unwrap();
        assert_eq!(
            Analyses::resolve(&[], Some(&section), 0.7).ast_similarity,
            0.9
        );
    }

    #[test]
    fn an_unknown_key_in_the_section_is_an_error() {
        assert!(serde_json::from_str::<LspSection>(r#"{"clone": {"enabled": true}}"#).is_err());
        assert!(serde_json::from_str::<LspSection>(r#"{"complexity": {"limit": 3}}"#).is_err());
    }
}
