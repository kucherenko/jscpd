//! What the boundary rules of a config file found (`boundaries`): copies of
//! protected code, a module, a folder, a file or a method, in places a rule
//! keeps it out of, such as money calculated in a frontend when only the
//! backend may calculate it.

use crate::models::{CloneKind, Fragment, SimilarityMethod};
use serde::{Deserialize, Serialize};

/// The violations of every rule, in the order the config file lists the
/// rules.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoundaryReport {
    pub rules: Vec<RuleReport>,
}

impl BoundaryReport {
    /// How many copies the rules found, all rules together.
    pub fn violations(&self) -> usize {
        self.rules.iter().map(|rule| rule.violations.len()).sum()
    }

    /// Every fragment of every violation, for the path rewrites a report
    /// needs: scan-root-relative display, or absolute paths.
    pub fn fragments_mut(&mut self) -> impl Iterator<Item = &mut Fragment> {
        self.rules
            .iter_mut()
            .flat_map(|rule| rule.violations.iter_mut())
            .flat_map(|v| [&mut v.copy.fragment, &mut v.protected.fragment])
    }
}

/// One rule and the copies of its code found outside its boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleReport {
    pub name: String,
    pub violations: Vec<Violation>,
}

/// A copy of protected code in a place its rule keeps the code out of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Violation {
    /// How the copy matches the protected code: `exact` or `renamed` for a
    /// token clone, `similar` for a merged clone or a pair of functions of
    /// the same structure, `semantic` for a function that does the same
    /// thing written another way, often in another language.
    pub kind: CloneKind,
    /// For a `similar` copy, the mechanism that found it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<SimilarityMethod>,
    /// For `similar` and `semantic` copies, the score of the match: matched
    /// tokens over the span (`gap`), the structural overlap of two functions
    /// (`ast`), or the cosine similarity of their embeddings (`semantic`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    /// Detection tokens the copy holds: the matched tokens of a token clone,
    /// the tokens of the smaller function of a function pair.
    pub tokens: u32,
    /// The copy, in a place the rule keeps the code out of.
    pub copy: Place,
    /// The protected code it copies.
    pub protected: Place,
}

impl Violation {
    /// `similarity` rounded to three decimals, the form reports print.
    pub fn similarity_rounded(&self) -> Option<f64> {
        self.similarity
            .map(|s| (f64::from(s) * 1000.0).round() / 1000.0)
    }

    /// The lines of the copy, the first and the last included.
    pub fn copy_lines(&self) -> u32 {
        let fragment = &self.copy.fragment;
        fragment.end.line.saturating_sub(fragment.start.line) + 1
    }
}

/// One side of a violation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub fragment: Fragment,
    /// The function the fragment lies in, when jscpd finds functions in its
    /// language and the function has a name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Location;

    fn place(file: &str, from: u32, to: u32) -> Place {
        Place {
            fragment: Fragment::new(
                file,
                Location::new(from, 0, 0),
                Location::new(to, 0, 0),
                [0, 0],
            ),
            function: None,
        }
    }

    fn violation() -> Violation {
        Violation {
            kind: CloneKind::Semantic,
            method: None,
            similarity: Some(0.712_345),
            tokens: 40,
            copy: place("web/total.ts", 12, 40),
            protected: place("backend/Fees.kt", 20, 51),
        }
    }

    #[test]
    fn a_report_counts_the_violations_of_every_rule() {
        let report = BoundaryReport {
            rules: vec![
                RuleReport {
                    name: "money".into(),
                    violations: vec![violation(), violation()],
                },
                RuleReport {
                    name: "auth".into(),
                    violations: vec![],
                },
            ],
        };
        assert_eq!(report.violations(), 2);
    }

    #[test]
    fn the_copy_counts_its_first_and_last_line() {
        assert_eq!(violation().copy_lines(), 29);
        assert_eq!(violation().similarity_rounded(), Some(0.712));
    }

    #[test]
    fn every_fragment_can_be_rewritten() {
        let mut report = BoundaryReport {
            rules: vec![RuleReport {
                name: "money".into(),
                violations: vec![violation()],
            }],
        };
        for fragment in report.fragments_mut() {
            fragment.source_id = format!("/root/{}", fragment.source_id);
        }
        let v = &report.rules[0].violations[0];
        assert_eq!(v.copy.fragment.source_id, "/root/web/total.ts");
        assert_eq!(v.protected.fragment.source_id, "/root/backend/Fees.kt");
    }
}
