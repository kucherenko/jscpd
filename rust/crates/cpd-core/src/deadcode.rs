//! What a dead-code run reports.
//!
//! These types live beside the clone models rather than in the analyzer that
//! produces them, so the reporters can render a dead-code run without
//! depending on the engine that performed it — the same split the clone side
//! already has between `models` and `cpd-reporter`.

use crate::models::Location;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A class of finding. Every category can be switched off independently
/// because they carry very different false-positive rates: unused imports are
/// nearly always safe to act on, unused class members in a dynamic codebase
/// are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// A file no entry point reaches through the import graph.
    UnusedFile,
    /// An exported name that no reachable module imports.
    UnusedExport,
    /// A module-private declaration with no references in its own module.
    UnusedSymbol,
    /// An import binding with no references.
    UnusedImport,
    /// A class member or enum member nothing appears to access.
    UnusedMember,
}

impl Category {
    pub const ALL: &'static [Category] = &[
        Category::UnusedFile,
        Category::UnusedExport,
        Category::UnusedSymbol,
        Category::UnusedImport,
        Category::UnusedMember,
    ];

    /// The default set: every category except members, whose accuracy depends
    /// most on type information basta does not have.
    pub const DEFAULT: &'static [Category] = &[
        Category::UnusedFile,
        Category::UnusedExport,
        Category::UnusedSymbol,
        Category::UnusedImport,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnusedFile => "unused-file",
            Self::UnusedExport => "unused-export",
            Self::UnusedSymbol => "unused-symbol",
            Self::UnusedImport => "unused-import",
            Self::UnusedMember => "unused-member",
        }
    }

    /// Heading used when grouping findings for a human reader.
    pub fn title(self) -> &'static str {
        match self {
            Self::UnusedFile => "Unused files",
            Self::UnusedExport => "Unused exports",
            Self::UnusedSymbol => "Unused symbols",
            Self::UnusedImport => "Unused imports",
            Self::UnusedMember => "Unused members",
        }
    }
}

impl FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Both spellings are accepted so `--categories unusedExports` from a
        // JSON config and `--categories unused-export` from a shell agree.
        match normalize(s).as_str() {
            "unused-file" | "unused-files" | "files" | "file" => Ok(Self::UnusedFile),
            "unused-export" | "unused-exports" | "exports" | "export" => Ok(Self::UnusedExport),
            "unused-symbol" | "unused-symbols" | "symbols" | "symbol" => Ok(Self::UnusedSymbol),
            "unused-import" | "unused-imports" | "imports" | "import" => Ok(Self::UnusedImport),
            "unused-member" | "unused-members" | "members" | "member" => Ok(Self::UnusedMember),
            other => Err(format!(
                "unknown category '{other}': expected one of unused-file, unused-export, \
                 unused-symbol, unused-import, unused-member"
            )),
        }
    }
}

/// Fold `unusedExports`, `unused_exports`, `UNUSED EXPORTS` and
/// `unused-exports` onto one spelling so config files and shells agree.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    let mut prev_lower = false;
    for ch in s.trim().chars() {
        match ch {
            '_' | ' ' | '-' => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
                prev_lower = false;
            }
            c if c.is_ascii_uppercase() => {
                if prev_lower {
                    out.push('-');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower = false;
            }
            c => {
                out.push(c);
                prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            }
        }
    }
    out
}

/// What a declaration is. The kind drives both the report wording and the
/// confidence penalties: an exported type alias that nothing imports is a
/// safer deletion than a class method that nothing appears to call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    /// A field or property on a class.
    Property,
    Interface,
    TypeAlias,
    Enum,
    EnumMember,
    Variable,
    /// A name bound by an `import` / `from x import y` statement.
    Import,
    /// A `export * from` / `export { x } from` binding that re-exports
    /// another module's symbol without declaring anything.
    ReExport,
}

impl SymbolKind {
    /// Lower-case word used in report messages.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Interface => "interface",
            Self::TypeAlias => "type",
            Self::Enum => "enum",
            Self::EnumMember => "enum member",
            Self::Variable => "variable",
            Self::Import => "import",
            Self::ReExport => "re-export",
        }
    }

    /// True for declarations that live inside a class body. Members are only
    /// ever reported when [`Category::UnusedMember`] is on,
    /// and they resolve against property accesses rather than bindings.
    pub fn is_member(self) -> bool {
        matches!(self, Self::Method | Self::Property | Self::EnumMember)
    }

    /// True for declarations that exist only in the type system. A type that
    /// is never imported is dead weight, but deleting one can never change
    /// runtime behavior, which the confidence model rewards.
    pub fn is_type_only(self) -> bool {
        matches!(self, Self::Interface | Self::TypeAlias)
    }
}

/// A single piece of dead code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Which rule produced this.
    #[serde(with = "category_serde")]
    pub category: Category,
    /// Scan-root-relative path of the file the finding is in.
    pub path: String,
    /// Declared name. Empty for [`Category::UnusedFile`].
    pub name: String,
    /// The name other modules would import it by, when it differs from `name`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_as: Option<String>,
    /// What kind of declaration this is. `None` for a whole-file finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKind>,
    /// Enclosing class or enum name, for members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The analyzer's language id (`js`, `python`, ...). A string rather than
    /// an enum so that a new language is a new analyzer, not a new variant in
    /// this crate.
    pub language: String,
    pub start: Location,
    pub end: Location,
    /// Lines the declaration spans — the size of the deletion.
    pub lines: u32,
    /// 0-100. See [`basta::confidence`](https://docs.rs/basta) for how it is derived.
    pub confidence: u8,
    /// Why the confidence is not 100, most significant first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<Reason>,
    /// One-line human-readable statement of the finding.
    pub message: String,
}

impl Finding {
    /// Confidence as a coarse bucket, for reporters that cannot show a number.
    pub fn level(&self) -> ConfidenceLevel {
        ConfidenceLevel::of(self.confidence)
    }

    /// Stable identity of a finding across runs: the same declaration in the
    /// same file keeps its fingerprint when unrelated lines move, because the
    /// line number is deliberately not part of it.
    pub fn fingerprint(&self) -> String {
        let parent = self.parent.as_deref().unwrap_or("");
        format!(
            "{}:{}:{}:{}",
            self.category.as_str(),
            self.path,
            parent,
            if self.name.is_empty() {
                "-"
            } else {
                &self.name
            }
        )
    }
}

/// Coarse confidence bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
    Certain,
}

impl ConfidenceLevel {
    pub fn of(score: u8) -> Self {
        match score {
            90..=u8::MAX => Self::Certain,
            75..=89 => Self::High,
            50..=74 => Self::Medium,
            _ => Self::Low,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Certain => "certain",
        }
    }
}

/// Something about the code that makes a finding less certain. Each reason
/// subtracts a fixed number of points; the reasons travel with the finding so
/// a reader can judge the score rather than trust it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// The module calls `eval`, `getattr`, `globals()`, `require(expr)` or
    /// accesses properties by computed key.
    DynamicAccess,
    /// The name appears inside a string literal somewhere in the scan, so it
    /// may be looked up by name at runtime.
    NameAppearsInString,
    /// The declaration carries a decorator the analyzer does not recognise.
    Decorated,
    /// The declaration is re-exported from a package index / `__init__.py`,
    /// which is usually a deliberate public surface.
    PackageSurface,
    /// The file exports a public API from a published package manifest.
    PublicApi,
    /// A member that overrides or implements an inherited declaration.
    Overrides,
    /// An abstract declaration whose implementations live in subclasses.
    Abstract,
    /// The only references come from test files, which this run does not treat
    /// as entry points.
    UsedOnlyByTests,
    /// The finding is inside a test, fixture or example file.
    InTestFile,
    /// A module in the scan failed to parse, so some references are unknown.
    UnparsedModule,
    /// The name is also declared elsewhere in the scan, so the reference
    /// matching may have attributed uses to the wrong declaration.
    AmbiguousName,
    /// A wildcard re-export (`export *`, `from m import *`) hides which names
    /// actually cross the module boundary.
    WildcardReExport,
    /// Something in the project reads this name as an attribute. In Python a
    /// module's contents are reachable as attributes of the module object, so
    /// `mod.name` may well be this declaration.
    NameReadAsAttribute,
    /// A string literal somewhere in the scan ends with this file's path,
    /// extension left off: `resolve(distDir, 'runtime/handlers/island')`.
    /// Not an import, so not an edge — but a framework that loads files by
    /// path writes exactly this, and the file is then very much alive.
    PathAppearsInString,
}

impl Reason {
    /// Points subtracted from the base score.
    pub fn penalty(self) -> u8 {
        match self {
            Self::DynamicAccess => 30,
            Self::NameAppearsInString => 35,
            Self::Decorated => 40,
            Self::PackageSurface => 20,
            Self::PublicApi => 60,
            Self::Overrides => 45,
            Self::Abstract => 50,
            Self::UsedOnlyByTests => 25,
            Self::InTestFile => 15,
            Self::UnparsedModule => 20,
            Self::AmbiguousName => 25,
            Self::WildcardReExport => 30,
            Self::NameReadAsAttribute => 35,
            // Enough to take an unused file (95) under the default floor of
            // 60: the string is no proof, and the finding is no longer one a
            // reader should act on without looking.
            Self::PathAppearsInString => 40,
        }
    }

    /// Short explanation shown next to a finding.
    pub fn explain(self) -> &'static str {
        match self {
            Self::DynamicAccess => "file resolves names at runtime",
            Self::NameAppearsInString => "name appears in a string literal",
            Self::Decorated => "carries an unrecognised decorator",
            Self::PackageSurface => "re-exported from a package index",
            Self::PublicApi => "part of the package's published API",
            Self::Overrides => "overrides an inherited member",
            Self::Abstract => "abstract declaration",
            Self::UsedOnlyByTests => "only referenced by tests",
            Self::InTestFile => "declared in a test file",
            Self::UnparsedModule => "a file in the scan did not parse",
            Self::AmbiguousName => "the name is declared more than once",
            Self::WildcardReExport => "reached through a wildcard re-export",
            Self::NameReadAsAttribute => "the name is read as an attribute elsewhere",
            Self::PathAppearsInString => "its path appears in a string literal",
        }
    }
}

/// Everything a run produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub findings: Vec<Finding>,
    pub statistics: Stats,
}

/// Run-level counters, for reporters and for the exit-code gate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Files walked and analyzed.
    pub files: u32,
    /// Files that failed to parse.
    pub unparsed: u32,
    /// Their paths, so a reader can tell a broken fixture from a real gap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unparsed_files: Vec<String>,
    /// Files an entry point reaches.
    pub reachable_files: u32,
    /// Declarations found across every analyzed file.
    pub symbols: u32,
    /// Entry-point files, however they were detected.
    pub entry_points: u32,
    /// Findings surviving the confidence threshold, per category.
    #[serde(default)]
    pub by_category: Vec<CategoryCount>,
    /// Lines of code the surviving findings cover.
    pub dead_lines: u32,
    /// Total lines across analyzed files.
    pub total_lines: u32,
    /// `dead_lines` as a percentage of `total_lines`.
    pub percentage: f64,
    /// ISO-8601 timestamp of the run.
    pub detection_date: String,
}

impl Stats {
    /// Findings reported, across every category.
    pub fn total_findings(&self) -> u32 {
        self.by_category.iter().map(|c| c.count).sum()
    }

    pub fn count_of(&self, category: Category) -> u32 {
        self.by_category
            .iter()
            .find(|c| c.category == category)
            .map_or(0, |c| c.count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    #[serde(with = "category_serde")]
    pub category: Category,
    pub count: u32,
    pub lines: u32,
}

/// `Category` serializes as its kebab-case name in both directions. It is
/// written by hand because the enum's `Deserialize` would otherwise have to
/// duplicate the alias list that [`std::str::FromStr`] already owns.
mod category_serde {
    use super::Category;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &Category, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(c.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Category, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(category: Category, confidence: u8) -> Finding {
        Finding {
            category,
            path: "src/a.ts".into(),
            name: "foo".into(),
            exported_as: None,
            symbol_kind: Some(SymbolKind::Function),
            parent: None,
            language: "js".into(),
            start: Location {
                line: 3,
                column: 0,
                offset: 20,
            },
            end: Location {
                line: 6,
                column: 1,
                offset: 60,
            },
            lines: 4,
            confidence,
            reasons: Vec::new(),
            message: "message".into(),
        }
    }

    #[test]
    fn confidence_buckets_have_no_gaps() {
        assert_eq!(ConfidenceLevel::of(100), ConfidenceLevel::Certain);
        assert_eq!(ConfidenceLevel::of(90), ConfidenceLevel::Certain);
        assert_eq!(ConfidenceLevel::of(89), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::of(75), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::of(74), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::of(50), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::of(49), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::of(0), ConfidenceLevel::Low);
    }

    #[test]
    fn fingerprint_ignores_line_numbers() {
        let mut a = finding(Category::UnusedExport, 90);
        let mut b = a.clone();
        b.start.line = 400;
        b.end.line = 410;
        assert_eq!(a.fingerprint(), b.fingerprint());
        a.name = "bar".into();
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn category_round_trips_through_json() {
        let f = finding(Category::UnusedMember, 80);
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"unused-member\""), "{json}");
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(back.category, Category::UnusedMember);
    }

    #[test]
    fn stats_sum_findings_across_categories() {
        let stats = Stats {
            by_category: vec![
                CategoryCount {
                    category: Category::UnusedExport,
                    count: 3,
                    lines: 30,
                },
                CategoryCount {
                    category: Category::UnusedImport,
                    count: 5,
                    lines: 5,
                },
            ],
            ..Stats::default()
        };
        assert_eq!(stats.total_findings(), 8);
        assert_eq!(stats.count_of(Category::UnusedExport), 3);
        assert_eq!(stats.count_of(Category::UnusedFile), 0);
    }
}
