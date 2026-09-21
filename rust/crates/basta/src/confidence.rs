//! How sure basta is that a finding is really dead.
//!
//! Static analysis of a dynamic language cannot be certain, and pretending
//! otherwise is how a tool gets switched off. Instead of hiding the doubt,
//! every finding carries a score and the reasons it is not 100.
//!
//! The score starts from a base that depends on what kind of finding it is —
//! an import binding with no references is a fact, a class member nothing
//! appears to call is an inference — and loses points for each piece of
//! contrary evidence. The reasons travel with the finding, so a reader can
//! disagree with the arithmetic instead of having to trust it.

use crate::finding::Reason;
use cpd_core::deadcode::Category;

/// Everything about a finding that bears on whether it is really dead. The
/// classifier fills in only the fields that can apply to the category it is
/// reporting: a string literal cannot keep an import *binding* alive, so
/// `name_in_string` is left false there.
#[derive(Debug, Default, Clone, Copy)]
pub struct Evidence {
    /// The file resolves names at runtime.
    pub dynamic_module: bool,
    /// The name appears in a string literal somewhere in the scan.
    pub name_in_string: bool,
    /// The declaration carries a decorator the analyzer does not recognise.
    pub decorated: bool,
    /// The declaration is abstract.
    pub abstract_declaration: bool,
    /// The declaration overrides an inherited member.
    pub overrides: bool,
    /// The declaration sits in a package index / `__init__.py`.
    pub package_surface: bool,
    /// A package manifest names the file as a published entry point.
    pub public_api: bool,
    /// The only importers are test files, which this run does not treat as
    /// entry points.
    pub used_only_by_tests: bool,
    /// The finding itself is inside a test, fixture or example file.
    pub in_test_file: bool,
    /// Some file in the scan failed to parse, so references are missing.
    pub unparsed_module: bool,
    /// More than one module declares this name.
    pub ambiguous_name: bool,
    /// The module's exports can be reached without being named.
    pub wildcarded: bool,
    /// Something reads this name as an attribute somewhere in the project.
    pub name_read_as_attribute: bool,
    /// An unused file whose path, without its extension, ends a string
    /// literal somewhere in the scan.
    pub path_in_string: bool,
}

/// The starting score for each category, before any evidence is subtracted.
///
/// The ordering is the ordering of how much inference each rule needs. An
/// unused import is read straight off the binding table. An unused member is
/// a guess made without types, matching `x.render()` against every `render`
/// in the project.
fn base_score(category: Category) -> u8 {
    match category {
        Category::UnusedImport => 100,
        Category::UnusedFile => 95,
        Category::UnusedSymbol => 90,
        Category::UnusedExport => 85,
        Category::UnusedMember => 70,
    }
}

/// Score a finding and list the reasons it is not certain, strongest first.
pub fn score(category: Category, evidence: &Evidence) -> (u8, Vec<Reason>) {
    let mut reasons = Vec::new();
    let mut add = |on: bool, reason: Reason| {
        if on {
            reasons.push(reason);
        }
    };
    add(evidence.public_api, Reason::PublicApi);
    add(evidence.abstract_declaration, Reason::Abstract);
    add(evidence.overrides, Reason::Overrides);
    add(evidence.decorated, Reason::Decorated);
    add(evidence.name_in_string, Reason::NameAppearsInString);
    add(evidence.path_in_string, Reason::PathAppearsInString);
    add(evidence.dynamic_module, Reason::DynamicAccess);
    add(evidence.wildcarded, Reason::WildcardReExport);
    add(evidence.name_read_as_attribute, Reason::NameReadAsAttribute);
    add(evidence.used_only_by_tests, Reason::UsedOnlyByTests);
    add(evidence.ambiguous_name, Reason::AmbiguousName);
    add(evidence.package_surface, Reason::PackageSurface);
    add(evidence.unparsed_module, Reason::UnparsedModule);
    add(evidence.in_test_file, Reason::InTestFile);

    reasons.sort_by(|a, b| {
        b.penalty()
            .cmp(&a.penalty())
            .then(a.explain().cmp(b.explain()))
    });
    let deduction: u32 = reasons.iter().map(|r| u32::from(r.penalty())).sum();
    let score = u32::from(base_score(category)).saturating_sub(deduction) as u8;
    (score, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finding_with_nothing_against_it_scores_its_base() {
        for category in Category::ALL {
            let (score, reasons) = score(*category, &Evidence::default());
            assert_eq!(score, base_score(*category), "{category:?}");
            assert!(reasons.is_empty(), "{category:?}");
        }
    }

    #[test]
    fn categories_are_ranked_by_how_much_inference_they_need() {
        let of = |c| score(c, &Evidence::default()).0;
        assert!(of(Category::UnusedImport) > of(Category::UnusedFile));
        assert!(of(Category::UnusedFile) > of(Category::UnusedSymbol));
        assert!(of(Category::UnusedSymbol) > of(Category::UnusedExport));
        assert!(of(Category::UnusedExport) > of(Category::UnusedMember));
    }

    #[test]
    fn each_piece_of_evidence_costs_its_penalty() {
        let (with, reasons) = score(
            Category::UnusedExport,
            &Evidence {
                dynamic_module: true,
                ..Evidence::default()
            },
        );
        assert_eq!(reasons, vec![Reason::DynamicAccess]);
        assert_eq!(
            with,
            base_score(Category::UnusedExport) - Reason::DynamicAccess.penalty()
        );
    }

    #[test]
    fn reasons_are_ordered_strongest_first() {
        let (_, reasons) = score(
            Category::UnusedMember,
            &Evidence {
                in_test_file: true,
                decorated: true,
                dynamic_module: true,
                ..Evidence::default()
            },
        );
        assert_eq!(
            reasons,
            vec![Reason::Decorated, Reason::DynamicAccess, Reason::InTestFile]
        );
    }

    #[test]
    fn overwhelming_evidence_floors_at_zero_rather_than_wrapping() {
        let (score, reasons) = score(
            Category::UnusedMember,
            &Evidence {
                dynamic_module: true,
                name_in_string: true,
                decorated: true,
                abstract_declaration: true,
                overrides: true,
                package_surface: true,
                public_api: true,
                used_only_by_tests: true,
                in_test_file: true,
                unparsed_module: true,
                ambiguous_name: true,
                wildcarded: true,
                name_read_as_attribute: true,
                path_in_string: true,
            },
        );
        assert_eq!(score, 0);
        assert_eq!(reasons.len(), 14);
    }

    #[test]
    fn a_file_whose_path_is_written_in_a_string_falls_below_the_default_threshold() {
        let (score, reasons) = score(
            Category::UnusedFile,
            &Evidence {
                path_in_string: true,
                ..Evidence::default()
            },
        );
        assert_eq!(score, 55, "reported on request, not by default");
        assert_eq!(reasons, vec![Reason::PathAppearsInString]);
    }

    #[test]
    fn a_published_api_export_falls_below_the_default_threshold() {
        let (score, _) = score(
            Category::UnusedExport,
            &Evidence {
                public_api: true,
                ..Evidence::default()
            },
        );
        assert!(
            score < crate::config::BastaConfig::default().min_confidence,
            "a package's published entry must not be reported by default, got {score}"
        );
    }

    #[test]
    fn an_unused_import_survives_a_single_weak_objection() {
        let (score, _) = score(
            Category::UnusedImport,
            &Evidence {
                in_test_file: true,
                ..Evidence::default()
            },
        );
        assert!(
            score >= crate::config::BastaConfig::default().min_confidence,
            "an unreferenced import binding is a fact, not an inference, got {score}"
        );
    }
}
