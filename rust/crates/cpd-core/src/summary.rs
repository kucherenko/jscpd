// summary.rs — opt-in codebase summary: per-file metrics, folder rollup, top-N lists.
//
// Everything in this module runs only when `--summary` is enabled, after
// detection has finished, over data already held in memory (SourceFile tokens
// and detected clones). Nothing in the detection hot path calls into it.

use crate::models::{CpdClone, SourceFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metric used to rank files and folders in the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SummaryMetric {
    #[default]
    Tokens,
    Lines,
    Size,
    Complexity,
}

impl std::str::FromStr for SummaryMetric {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tokens" => Ok(Self::Tokens),
            "lines" => Ok(Self::Lines),
            "size" => Ok(Self::Size),
            "complexity" => Ok(Self::Complexity),
            other => Err(format!(
                "invalid summary metric '{other}': must be one of: tokens, lines, size, complexity"
            )),
        }
    }
}

impl std::fmt::Display for SummaryMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Tokens => "tokens",
            Self::Lines => "lines",
            Self::Size => "size",
            Self::Complexity => "complexity",
        };
        f.write_str(s)
    }
}

/// Per-file summary row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub path: String,
    pub format: String,
    pub lines: u64,
    pub tokens: u64,
    pub bytes: u64,
    pub duplicated_lines: u64,
    pub duplicated_tokens: u64,
    /// Cyclomatic-complexity estimate: 1 + count of decision-point tokens
    /// (`if`, `for`, `while`, `case`, `catch`, `&&`, `||`, `?`, …).
    pub complexity: u64,
}

/// Per-folder rollup. Files are counted in their direct parent directory only
/// (no cumulative ancestor totals), so every file contributes to exactly one
/// folder row and rows are directly comparable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub path: String,
    pub files: u64,
    pub lines: u64,
    pub tokens: u64,
    pub bytes: u64,
    pub duplicated_lines: u64,
    /// Sum of per-file complexity estimates (divide by `files` for the mean).
    pub complexity: u64,
}

/// Codebase summary: top files and folder rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Primary sort metric.
    pub by: SummaryMetric,
    /// Top-N files by `by`, descending. Every row carries all metrics
    /// (tokens, lines, bytes, complexity, duplication) so one list serves
    /// every lens; re-run with a different `--summary-by` to re-rank.
    pub files: Vec<FileSummary>,
    /// Top-N folders by `by`, direct-parent aggregation.
    pub folders: Vec<FolderSummary>,
    /// Total number of files analyzed (before top-N truncation).
    pub total_files: u64,
    /// Total number of folders (before top-N truncation).
    pub total_folders: u64,
}

/// Decision-point tokens counted by the complexity estimate. Conservative,
/// language-agnostic list: branch/loop keywords and short-circuit operators
/// that appear as standalone tokens across supported languages.
///
/// Matching is ASCII-case-insensitive so case-insensitive and
/// uppercase-keyword languages (SQL, PL/SQL, Fortran, COBOL, BASIC, Pascal)
/// count too. The occasional identifier spelled like a keyword slightly
/// inflates an estimate that is only used for ranking.
fn is_decision_token(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 7 {
        return false;
    }
    let mut lower = [0u8; 7];
    for (dst, b) in lower.iter_mut().zip(bytes) {
        *dst = b.to_ascii_lowercase();
    }
    matches!(
        &lower[..bytes.len()],
        b"if"
            | b"elif"
            | b"elsif"
            | b"elseif"
            | b"unless"
            | b"for"
            | b"foreach"
            | b"while"
            | b"until"
            | b"case"
            | b"cond"
            | b"when"
            | b"catch"
            | b"rescue"
            | b"except"
            | b"andalso"
            | b"orelse"
            | b"&&"
            | b"||"
            | b"and"
            | b"or"
            | b"?"
            | b"??"
    )
}

/// A synthetic source is the per-sub-format shadow of a multi-format file
/// (markdown/vue/svelte embedded code); its id is `<parent-id>:<format>` and
/// its metrics are already covered by the parent entry.
fn is_synthetic(source: &SourceFile) -> bool {
    source
        .id
        .strip_suffix(source.format.as_str())
        .is_some_and(|prefix| prefix.ends_with(':'))
}

fn metric_of(file: &FileSummary, by: SummaryMetric) -> u64 {
    match by {
        SummaryMetric::Tokens => file.tokens,
        SummaryMetric::Lines => file.lines,
        SummaryMetric::Size => file.bytes,
        SummaryMetric::Complexity => file.complexity,
    }
}

fn folder_metric_of(folder: &FolderSummary, by: SummaryMetric) -> u64 {
    match by {
        SummaryMetric::Tokens => folder.tokens,
        SummaryMetric::Lines => folder.lines,
        SummaryMetric::Size => folder.bytes,
        SummaryMetric::Complexity => folder.complexity,
    }
}

/// Parent directory of a path, with separators normalized to `/`.
/// Files at the scan root map to `"."`.
fn parent_dir(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    match normalized.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => normalized[..idx].to_string(),
        None => ".".to_string(),
    }
}

/// Compute the summary from detection results.
///
/// `display_path` maps a source id (canonical absolute path) to the path shown
/// in reports — the same relativization applied to clone fragments, so
/// per-file duplication matching works on identical strings.
pub fn compute_summary(
    sources: &[SourceFile],
    clones: &[CpdClone],
    top: usize,
    by: SummaryMetric,
    display_path: impl Fn(&str) -> String,
) -> Summary {
    // Per-file duplication, keyed by display path. Both fragments of a clone
    // count toward their file: the question here is "where does duplicated
    // code live", not the de-duplicated total that Statistics reports.
    let mut dup: HashMap<String, (u64, u64)> = HashMap::new();
    for clone in clones {
        for fragment in [&clone.fragment_a, &clone.fragment_b] {
            // Sub-format fragments carry a `<path>:<format>` id; fold them
            // into the parent file.
            let path = fragment
                .source_id
                .strip_suffix(&format!(":{}", clone.format))
                .unwrap_or(&fragment.source_id);
            let entry = dup.entry(path.to_string()).or_default();
            entry.0 += fragment.end.line.saturating_sub(fragment.start.line) as u64;
            entry.1 += clone.token_count as u64;
        }
    }

    let mut files: Vec<FileSummary> = sources
        .iter()
        .filter(|s| !is_synthetic(s))
        .map(|source| {
            let path = display_path(&source.id);
            // Same line metric as Statistics: max token start line.
            let lines = source
                .tokens
                .iter()
                .map(|t| t.start.line)
                .max()
                .unwrap_or(0) as u64;
            let decisions = source
                .tokens
                .iter()
                .filter(|t| is_decision_token(&t.value))
                .count() as u64;
            let (duplicated_lines, duplicated_tokens) = dup.get(&path).copied().unwrap_or_default();
            FileSummary {
                lines,
                tokens: source.tokens.len() as u64,
                bytes: source.bytes,
                duplicated_lines,
                duplicated_tokens,
                complexity: 1 + decisions,
                format: source.format.clone(),
                path,
            }
        })
        .collect();

    let total_files = files.len() as u64;

    // Folder rollup over ALL files (before top-N truncation).
    let mut folder_map: HashMap<String, FolderSummary> = HashMap::new();
    for file in &files {
        let dir = parent_dir(&file.path);
        let entry = folder_map
            .entry(dir.clone())
            .or_insert_with(|| FolderSummary {
                path: dir,
                files: 0,
                lines: 0,
                tokens: 0,
                bytes: 0,
                duplicated_lines: 0,
                complexity: 0,
            });
        entry.files += 1;
        entry.lines += file.lines;
        entry.tokens += file.tokens;
        entry.bytes += file.bytes;
        entry.duplicated_lines += file.duplicated_lines;
        entry.complexity += file.complexity;
    }
    let total_folders = folder_map.len() as u64;

    // Top-N files by the primary metric: `--summary-top N` always yields at
    // most N rows (least surprise). Other lenses are one `--summary-by` away;
    // every row still carries all metrics.
    files.sort_by(|a, b| {
        metric_of(b, by)
            .cmp(&metric_of(a, by))
            .then_with(|| a.path.cmp(&b.path))
    });
    files.truncate(top);

    let mut folders: Vec<FolderSummary> = folder_map.into_values().collect();
    folders.sort_by(|a, b| {
        folder_metric_of(b, by)
            .cmp(&folder_metric_of(a, by))
            .then_with(|| a.path.cmp(&b.path))
    });
    folders.truncate(top);

    Summary {
        by,
        files,
        folders,
        total_files,
        total_folders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CpdClone, Fragment, Location, Token, TokenKind};

    fn loc(line: u32) -> Location {
        Location {
            line,
            column: 0,
            offset: 0,
        }
    }

    fn token(value: &str, line: u32) -> Token {
        Token {
            kind: TokenKind::Keyword,
            value: value.to_string(),
            start: loc(line),
            end: loc(line),
        }
    }

    fn source(id: &str, format: &str, values: &[&str], bytes: u64) -> SourceFile {
        SourceFile {
            id: id.to_string(),
            format: format.to_string(),
            tokens: values
                .iter()
                .enumerate()
                .map(|(i, v)| token(v, i as u32 + 1))
                .collect(),
            bytes,
        }
    }

    fn clone_between(format: &str, a: &str, b: &str, lines: u32, tokens: u32) -> CpdClone {
        let fragment = |id: &str| Fragment {
            source_id: id.to_string(),
            source_root: None,
            start: loc(1),
            end: loc(1 + lines),
            range: [0, tokens],
            blame: None,
        };
        CpdClone {
            format: format.to_string(),
            fragment_a: fragment(a),
            fragment_b: fragment(b),
            token_count: tokens,
            is_new: false,
            kind: Default::default(),
        }
    }

    fn identity(path: &str) -> String {
        path.to_string()
    }

    #[test]
    fn empty_input_produces_empty_summary() {
        let summary = compute_summary(&[], &[], 10, SummaryMetric::Tokens, identity);
        assert!(summary.files.is_empty());
        assert!(summary.folders.is_empty());
        assert_eq!(summary.total_files, 0);
        assert_eq!(summary.total_folders, 0);
    }

    #[test]
    fn files_sorted_by_primary_metric() {
        let sources = vec![
            source("src/small.js", "javascript", &["a", "b"], 10),
            source("src/big.js", "javascript", &["a", "b", "c", "d"], 20),
        ];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Tokens, identity);
        assert_eq!(summary.files[0].path, "src/big.js");
        assert_eq!(summary.files[0].tokens, 4);
        assert_eq!(summary.total_files, 2);
    }

    #[test]
    fn top_n_is_exact_row_count_by_primary_metric() {
        // huge.js wins on tokens, fat.js wins on size — top=1 by tokens must
        // yield exactly one row: huge.js. `--summary-top N` never surprises
        // with more than N rows; other metrics are served by --summary-by.
        let sources = vec![
            source("huge.js", "javascript", &["a", "b", "c", "d", "e"], 1),
            source("fat.js", "javascript", &["a"], 9999),
        ];
        let summary = compute_summary(&sources, &[], 1, SummaryMetric::Tokens, identity);
        assert_eq!(summary.files.len(), 1);
        assert_eq!(summary.files[0].path, "huge.js");
        assert_eq!(summary.total_files, 2, "truncation stays visible");

        let by_size = compute_summary(&sources, &[], 1, SummaryMetric::Size, identity);
        assert_eq!(by_size.files[0].path, "fat.js");
    }

    #[test]
    fn complexity_counts_decision_tokens() {
        let sources = vec![source(
            "a.js",
            "javascript",
            &["if", "x", "&&", "y", "for", "z", "else"],
            10,
        )];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity);
        // 1 + (if, &&, for) = 4; "else" is not a decision point.
        assert_eq!(summary.files[0].complexity, 4);
    }

    #[test]
    fn complexity_is_case_insensitive() {
        // SQL / PL/SQL / Fortran style uppercase keywords.
        let sources = vec![source(
            "a.sql",
            "sql",
            &["IF", "x", "OR", "y", "WHEN", "THEN", "If"],
            10,
        )];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity);
        // 1 + (IF, OR, WHEN, If) = 5; THEN is not a decision point.
        assert_eq!(summary.files[0].complexity, 5);
    }

    #[test]
    fn decision_token_edge_cases() {
        assert!(is_decision_token("unless"));
        assert!(is_decision_token("ELSEIF"));
        assert!(is_decision_token("andalso"));
        assert!(!is_decision_token(""));
        assert!(!is_decision_token("iffy"));
        assert!(!is_decision_token("conditionally"), "length-capped");
        assert!(!is_decision_token("форматирование"), "non-ASCII ignored");
    }

    #[test]
    fn folder_rollup_uses_direct_parent() {
        let sources = vec![
            source("src/app/a.js", "javascript", &["x"], 5),
            source("src/app/b.js", "javascript", &["x", "y"], 5),
            source("src/c.js", "javascript", &["x"], 5),
            source("root.js", "javascript", &["x"], 5),
        ];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Tokens, identity);
        assert_eq!(summary.total_folders, 3);
        let app = summary
            .folders
            .iter()
            .find(|f| f.path == "src/app")
            .expect("src/app folder");
        assert_eq!(app.files, 2);
        assert_eq!(app.tokens, 3);
        let root = summary.folders.iter().find(|f| f.path == ".");
        assert!(root.is_some(), "root files grouped under '.'");
    }

    #[test]
    fn duplication_attributed_to_both_fragments() {
        let sources = vec![
            source("a.js", "javascript", &["x", "y", "z"], 5),
            source("b.js", "javascript", &["x", "y", "z"], 5),
        ];
        let clones = vec![clone_between("javascript", "a.js", "b.js", 9, 30)];
        let summary = compute_summary(&sources, &clones, 10, SummaryMetric::Tokens, identity);
        for path in ["a.js", "b.js"] {
            let file = summary.files.iter().find(|f| f.path == path).unwrap();
            assert_eq!(file.duplicated_lines, 9, "{path} duplicated lines");
            assert_eq!(file.duplicated_tokens, 30, "{path} duplicated tokens");
        }
    }

    #[test]
    fn synthetic_sub_format_sources_are_skipped() {
        let sources = vec![
            source("doc.md", "markdown", &["x", "y"], 100),
            source("doc.md:javascript", "javascript", &["x"], 0),
        ];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Tokens, identity);
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.files[0].path, "doc.md");
    }

    #[test]
    fn sub_format_clone_folds_into_parent_file() {
        let sources = vec![source("doc.md", "markdown", &["x", "y"], 100)];
        let clones = vec![clone_between(
            "javascript",
            "doc.md:javascript",
            "doc.md:javascript",
            4,
            20,
        )];
        let summary = compute_summary(&sources, &clones, 10, SummaryMetric::Tokens, identity);
        assert_eq!(
            summary.files[0].duplicated_lines, 8,
            "both fragments fold in"
        );
    }

    #[test]
    fn display_path_applied_before_dup_matching() {
        let sources = vec![source("/abs/root/a.js", "javascript", &["x"], 5)];
        let clones = vec![clone_between("javascript", "a.js", "a.js", 2, 10)];
        let summary = compute_summary(&sources, &clones, 10, SummaryMetric::Tokens, |p| {
            p.strip_prefix("/abs/root/").unwrap_or(p).to_string()
        });
        assert_eq!(summary.files[0].path, "a.js");
        assert_eq!(summary.files[0].duplicated_lines, 4);
    }

    #[test]
    fn folders_truncated_to_top_n_but_total_reported() {
        let sources: Vec<SourceFile> = (0..5)
            .map(|i| source(&format!("dir{i}/f.js"), "javascript", &["x"], 1))
            .collect();
        let summary = compute_summary(&sources, &[], 2, SummaryMetric::Tokens, identity);
        assert_eq!(summary.folders.len(), 2);
        assert_eq!(summary.total_folders, 5);
    }

    #[test]
    fn metric_parses_from_str() {
        assert_eq!(
            "complexity".parse::<SummaryMetric>().unwrap(),
            SummaryMetric::Complexity
        );
        assert!("bogus".parse::<SummaryMetric>().is_err());
    }

    #[test]
    fn summary_serializes_camel_case() {
        let sources = vec![source("a.js", "javascript", &["x"], 5)];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Size, identity);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"totalFiles\""));
        assert!(json.contains("\"duplicatedLines\""));
        assert!(json.contains("\"by\":\"size\""));
        assert!(!json.contains("total_files"));
    }
}
