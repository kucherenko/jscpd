// summary.rs — opt-in codebase summary: per-file metrics, folder rollup, top-N lists.
//
// Everything in this module runs only when `--summary` is enabled, after
// detection has finished, over data already held in memory (SourceFile tokens
// and detected clones). Nothing in the detection hot path calls into it.

use crate::models::{CpdClone, SourceFile, Token, TokenKind};
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
///
/// Operators reach this function already joined — see [`joined_token`]. Only
/// the JavaScript tokenizer emits `&&` as one token; the generic one splits
/// every punctuation run into single characters, so without that joining the
/// short-circuit arms here would be unreachable for every other format.
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

/// What a bare `?` means in a language.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuestionMark {
    /// It only ever opens a ternary: C, Java, PHP, Go templates, and most else.
    Ternary,
    /// It also marks an optional, so `String?` and `x?.y` are types and
    /// accesses rather than branches. Neither TypeScript nor Swift has an
    /// Elvis operator, so `?:` there is an optional property, not a branch.
    Optional,
    /// As above, but `?:` *is* the Elvis operator and does branch: Kotlin,
    /// Groovy.
    OptionalWithElvis,
}

/// How one language spells its branches, beyond the shared keyword set.
///
/// The shared set is right for most of the formats jscpd knows. An entry here
/// exists only where a language branches on something the set has no word for
/// (`match` arms in Rust, `guard` in Swift, `select` in Go) or spells
/// something in the set so differently that counting it is simply wrong.
#[derive(Clone, Copy)]
struct DecisionRules {
    /// Branch tokens beyond the shared set, matched after joining.
    extra: &'static [&'static str],
    question: QuestionMark,
    /// Tokens that open a function body. Cyclomatic complexity is one path per
    /// function; where a language has no single reliable marker this stays
    /// empty and the estimate keeps its per-file baseline of one.
    declarations: &'static [&'static str],
    /// The C family declares a function as `head(args) {` with no keyword at
    /// all, so its functions are counted from that shape instead.
    braced_declarations: bool,
    /// Whether `"""` and `'''` delimit a string that can span lines; see
    /// [`has_triple_quoted_strings`].
    triple_quoted_strings: bool,
    /// Tokens that open a group of arms counted one by one through `extra`.
    /// Each cancels one arm, because N arms are N paths and so N - 1 branches,
    /// the same way a `switch` counts its `case` labels but not its `default`.
    arm_groups: &'static [&'static str],
}

impl Default for DecisionRules {
    fn default() -> Self {
        Self {
            extra: &[],
            question: QuestionMark::Ternary,
            declarations: &[],
            braced_declarations: false,
            triple_quoted_strings: false,
            arm_groups: &[],
        }
    }
}

/// Formats whose strings can span lines between `"""` or `'''` delimiters.
///
/// Where a doubled quote is how a quote is escaped — C# verbatim strings,
/// VB.NET, SQL, Pascal — three quotes in a row are ordinary string content: the
/// regex `@"""((?:\\.|[^""\\])*)"""` is one line of C#. Reading such a run as
/// a delimiter opens a string that never closes and swallows the rest of the
/// file, which is why this is a list rather than a default.
fn has_triple_quoted_strings(format: &str) -> bool {
    matches!(
        format,
        "python" | "kotlin" | "scala" | "groovy" | "swift" | "java" | "julia" | "elixir" | "dart"
    )
}

/// False for prose and data formats, whose "if" and `||` are words and
/// version ranges rather than branches.
fn has_control_flow(format: &str) -> bool {
    !matches!(
        format,
        "markdown"
            | "asciidoc"
            | "textile"
            | "wiki"
            | "txt"
            | "log"
            | "csv"
            | "json"
            | "json5"
            | "yaml"
            | "toml"
            | "ini"
            | "properties"
            | "editorconfig"
            | "ignore"
            | "diff"
            | "gettext"
    )
}

fn rules_for(format: &str) -> DecisionRules {
    DecisionRules {
        triple_quoted_strings: has_triple_quoted_strings(format),
        ..language_rules(format)
    }
}

fn language_rules(format: &str) -> DecisionRules {
    match format {
        // A `match` has no keyword per arm, only the `=>` each one is written
        // with. Every arm is counted and the `match` itself takes one back: a
        // three-arm match is three paths, which is two branches.
        "rust" => DecisionRules {
            extra: &["=>"],
            declarations: &["fn"],
            arm_groups: &["match"],
            ..Default::default()
        },
        "swift" => DecisionRules {
            extra: &["guard"],
            question: QuestionMark::Optional,
            declarations: &["func"],
            ..Default::default()
        },
        // `=>` opens an arrow function here rather than a branch, so it counts
        // toward the function tally instead.
        "typescript" | "tsx" | "flow" | "javascript" | "jsx" => DecisionRules {
            question: QuestionMark::Optional,
            declarations: &["function", "=>"],
            ..Default::default()
        },
        "kotlin" => DecisionRules {
            question: QuestionMark::OptionalWithElvis,
            declarations: &["fun"],
            ..Default::default()
        },
        "groovy" => DecisionRules {
            question: QuestionMark::OptionalWithElvis,
            declarations: &["def"],
            ..Default::default()
        },
        "csharp" => DecisionRules {
            question: QuestionMark::Optional,
            braced_declarations: true,
            ..Default::default()
        },
        "c" | "c-header" | "cpp" | "cpp-header" | "java" | "objectivec" | "clike" => {
            DecisionRules {
                braced_declarations: true,
                ..Default::default()
            }
        }
        "go" => DecisionRules {
            extra: &["select"],
            declarations: &["func"],
            ..Default::default()
        },
        "scala" | "python" | "ruby" | "crystal" => DecisionRules {
            declarations: &["def"],
            ..Default::default()
        },
        "php" | "lua" => DecisionRules {
            declarations: &["function"],
            ..Default::default()
        },
        "erlang" => DecisionRules {
            extra: &["receive"],
            ..Default::default()
        },
        _ => DecisionRules::default(),
    }
}

/// Two-character operators the generic tokenizer hands over as two tokens.
const JOINED_OPERATORS: &[&str] = &["&&", "||", "??", "?.", "?:", "=>"];

/// One token's text, or the two-character operator that two adjacent tokens
/// spell between them.
///
/// The pair cannot borrow: each token owns its own `String`, so two adjacent
/// characters of the source are not adjacent in memory. Two bytes on the stack
/// avoid an allocation per operator.
enum Scanned<'a> {
    Single(&'a str),
    Pair([u8; 2]),
}

impl Scanned<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Single(text) => text,
            Self::Pair(bytes) => std::str::from_utf8(bytes).unwrap_or(""),
        }
    }
}

/// The token at `at`, joined with the next one when the two are adjacent
/// single-character punctuation spelling a two-character operator.
///
/// The generic tokenizer splits `&&` into two `&` tokens, so a scan that looks
/// at one token at a time can never see a short-circuit operator at all.
/// Returns what to classify and the index to continue from.
fn joined_token(tokens: &[Token], at: usize) -> (Scanned<'_>, usize) {
    let current = &tokens[at];
    let single = (Scanned::Single(current.value.as_str()), at + 1);
    let Some(next) = tokens.get(at + 1) else {
        return single;
    };
    let (Some(&left), Some(&right)) = (
        current.value.as_bytes().first(),
        next.value.as_bytes().first(),
    ) else {
        return single;
    };
    // Adjacent in the source, and both exactly one punctuation character.
    if current.end.offset != next.start.offset
        || current.value.len() != 1
        || next.value.len() != 1
        || !left.is_ascii_punctuation()
        || !right.is_ascii_punctuation()
    {
        return single;
    }
    let pair = [left, right];
    match std::str::from_utf8(&pair).is_ok_and(|text| JOINED_OPERATORS.contains(&text)) {
        true => (Scanned::Pair(pair), at + 2),
        false => single,
    }
}

/// Which tokens lie inside a triple-quoted string.
///
/// The tokenizer is line-based: it closes every string at the end of the line
/// it started on, so the body of a `"""` string arrives as ordinary words, and
/// a docstring saying "if the value is big" lands three branches on the file.
///
/// The quotes themselves survive as literal tokens, and a run of adjacent
/// literals spells exactly the quote characters its line holds: a PEP 257
/// opener `"""Summary.` arrives as `""` and `"Summary.`, a closer on its own
/// line as `""` and `"`, and a one-line docstring as `""`, `"One."`, `""`. So
/// an odd number of triple quotes in a run opens or closes the string, and an
/// even number leaves it as it was.
///
/// Reading a pair of tokens alone is not enough, and was the first version of
/// this: with the summary written on the opening line the opener is not a bare
/// quote, so the *closing* `"""` looked like an opener and swallowed every
/// branch that followed it.
fn triple_quoted(tokens: &[Token]) -> Vec<bool> {
    const DELIMITERS: [&str; 2] = ["\"\"\"", "'''"];
    let mut inside = vec![false; tokens.len()];
    let mut open: Option<&str> = None;
    let mut at = 0usize;
    while at < tokens.len() {
        if tokens[at].kind != TokenKind::Literal {
            inside[at] = open.is_some();
            at += 1;
            continue;
        }
        let start = at;
        let mut text = tokens[at].value.clone();
        at += 1;
        while at < tokens.len()
            && tokens[at].kind == TokenKind::Literal
            && tokens[at - 1].end.offset == tokens[at].start.offset
        {
            text.push_str(&tokens[at].value);
            at += 1;
        }
        let was_open = open.is_some();
        for delimiter in DELIMITERS {
            let toggles = text.matches(delimiter).count() % 2 == 1;
            match open {
                Some(current) if current == delimiter && toggles => open = None,
                None if toggles => open = Some(delimiter),
                _ => {}
            }
        }
        // The run is string text whichever way it turned the state.
        inside[start..at].fill(was_open || open.is_some());
    }
    inside
}

/// Words a file binds as names of its own.
///
/// `case`, `when` and `cond` are branch keywords in some languages and
/// perfectly ordinary variable names in others — the shared list cannot tell
/// them apart, and the tokenizer is no help: its `classify_word` returns
/// `Identifier` for every word, so `if` in Python is tagged exactly like a
/// variable called `case`.
///
/// What a file *does* reveal is which words it assigns to, reads off an
/// object, or lists as a parameter or argument. A word this file writes
/// `case = 3`, `x.case` or `f(case, when)` for is that file's own name,
/// whatever the language reserves, and counting it as a branch is what
/// inflated the estimate elevenfold on a file of five such assignments.
fn locally_bound(tokens: &[Token]) -> Vec<&str> {
    let mut bound = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        if !is_decision_token(&token.value) {
            continue;
        }
        // `obj.case` — a member, never the keyword.
        let after_dot = at
            .checked_sub(1)
            .is_some_and(|previous| tokens[previous].value == ".");
        // `case = 3`, but not `case == 3` or `case => 3`.
        let assigned = tokens.get(at + 1).is_some_and(|next| next.value == "=")
            && tokens
                .get(at + 2)
                .is_none_or(|after| !matches!(after.value.as_str(), "=" | ">"));
        // `def headline(case, when)` or `headline(case, when)`: a keyword is
        // never written directly before a comma or a closing paren, a name in
        // a parameter or argument list always is. Words only — `Ok(parse()?)`
        // is Rust's `?` doing its job, not a name.
        let listed = token.value.starts_with(|c: char| c.is_ascii_alphabetic())
            && tokens
                .get(at + 1)
                .is_some_and(|next| matches!(next.value.as_str(), "," | ")"));
        if after_dot || assigned || listed {
            bound.push(token.value.as_str());
        }
    }
    bound.sort_unstable();
    bound.dedup();
    bound
}

/// Keywords that take a parenthesised head and a block, so that `) {` after
/// one of them opens a branch rather than a function body.
const PARENTHESISED_STATEMENTS: &[&str] = &[
    "switch",
    "using",
    "lock",
    "synchronized",
    "with",
    "do",
    "try",
    "return",
    "sizeof",
    "typeof",
    "new",
    "throw",
    "await",
    "yield",
    "fixed",
    "unsafe",
];

/// Decision points and function count for one file.
///
/// Cyclomatic complexity is one path per function plus one per branch. Where a
/// language has no reliable function marker the tally falls back to the
/// per-file baseline of one, which is what this estimate has always used.
fn scan_complexity(tokens: &[Token], rules: &DecisionRules) -> (u64, u64) {
    let (mut decisions, mut functions, mut groups) = (0u64, 0u64, 0u64);
    let mut at = 0usize;
    let in_string = match rules.triple_quoted_strings {
        true => triple_quoted(tokens),
        false => vec![false; tokens.len()],
    };
    let bound = locally_bound(tokens);
    // What the token before each open paren was, so a `) {` can be told from
    // the head it closes: a function signature, or `if (…) {`.
    let mut heads: Vec<&str> = Vec::new();
    let mut last_head: Option<&str> = None;
    while at < tokens.len() {
        // Skip the body of a string that spans lines; see `triple_quoted`.
        if in_string[at] {
            at += 1;
            continue;
        }
        let (scanned, next) = joined_token(tokens, at);
        let text = scanned.text();
        // `String?` writes the optional against the type it belongs to;
        // `cond ? a : b` puts the ternary in the open. Nothing else separates
        // the two without parsing, and the convention is near-universal.
        let attached = at
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous))
            .is_some_and(|previous| previous.end.offset == tokens[at].start.offset);
        at = next;
        // C, C++, Java and C# open a function with `) {` and no keyword of
        // their own. Tracking what preceded each `(` is enough to tell that
        // from the `) {` of an `if` or a `switch`, and it is the only reason
        // those languages kept a per-file baseline.
        if rules.braced_declarations {
            match text {
                "(" => {
                    let head = at
                        .checked_sub(2)
                        .and_then(|before| tokens.get(before))
                        .map(|token| token.value.as_str())
                        .unwrap_or("");
                    heads.push(head);
                }
                ")" => last_head = heads.pop(),
                "{" => {
                    if let Some(head) = last_head.take()
                        && !head.is_empty()
                        && head.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && !is_decision_token(head)
                        && !PARENTHESISED_STATEMENTS
                            .iter()
                            .any(|s| s.eq_ignore_ascii_case(head))
                    {
                        functions += 1;
                    }
                }
                _ => last_head = None,
            }
        }
        if rules
            .declarations
            .iter()
            .any(|d| d.eq_ignore_ascii_case(text))
        {
            functions += 1;
            continue;
        }
        if rules
            .arm_groups
            .iter()
            .any(|g| g.eq_ignore_ascii_case(text))
        {
            groups += 1;
            continue;
        }
        if rules.extra.iter().any(|e| e.eq_ignore_ascii_case(text)) {
            decisions += 1;
            continue;
        }
        // A word this file binds as a name is that file's name, not a keyword.
        if bound.binary_search(&text).is_ok() {
            continue;
        }
        let counts = match text {
            // `?.` never branches, and `?:` branches only where it is the
            // Elvis operator rather than an optional property.
            "?." => false,
            "?:" => rules.question != QuestionMark::Optional,
            "?" => rules.question == QuestionMark::Ternary || !attached,
            other => is_decision_token(other),
        };
        if counts {
            decisions += 1;
        }
    }
    (decisions.saturating_sub(groups), functions)
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
        for (fragment, unmatched) in [
            (&clone.fragment_a, clone.unmatched_lines[0]),
            (&clone.fragment_b, clone.unmatched_lines[1]),
        ] {
            // Sub-format fragments carry a `<path>:<format>` id; fold them
            // into the parent file.
            let path = fragment
                .source_id
                .strip_suffix(&format!(":{}", clone.format))
                .unwrap_or(&fragment.source_id);
            let entry = dup.entry(path.to_string()).or_default();
            // Gap lines of a merged clone are not duplicated code.
            entry.0 += fragment
                .end
                .line
                .saturating_sub(fragment.start.line)
                .saturating_sub(unmatched) as u64;
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
            let (decisions, functions) =
                scan_complexity(&source.tokens, &rules_for(&source.format));
            let (duplicated_lines, duplicated_tokens) = dup.get(&path).copied().unwrap_or_default();
            FileSummary {
                lines,
                tokens: source.tokens.len() as u64,
                bytes: source.bytes,
                duplicated_lines,
                duplicated_tokens,
                // One path per function, or the per-file baseline where the
                // language has no marker the scan can trust. Prose and data
                // have no paths: an "if" in a README is a word, and a lock
                // file full of `||` version ranges is not code.
                complexity: match has_control_flow(&source.format) {
                    true => functions.max(1) + decisions,
                    false => 0,
                },
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
            similarity: None,
            similarity_method: None,
            unmatched_lines: [0, 0],
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
    fn prose_and_data_have_no_complexity() {
        let words = [
            "If", "you", "need", "it", "or", "while", "waiting", "for", "a", "case",
        ];
        let sources = vec![
            source("README.md", "markdown", &words, 10),
            source(
                "pnpm-lock.yaml",
                "yaml",
                &["version", ":", "^1", "||", "^2"],
                10,
            ),
            source("notes.py", "python", &words, 10),
        ];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity);
        let cx = |path: &str| {
            summary
                .files
                .iter()
                .find(|f| f.path == path)
                .unwrap()
                .complexity
        };
        assert_eq!(cx("README.md"), 0, "a word is not a branch");
        assert_eq!(cx("pnpm-lock.yaml"), 0, "a version range is not a branch");
        assert!(cx("notes.py") > 1, "the same words in code still count");
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

    /// Tokens laid out over a real source string the way the generic
    /// tokenizer hands them over: one per word, one per punctuation
    /// character, with the offsets that make adjacency visible.
    fn lay_out(id: &str, format: &str, code: &str) -> SourceFile {
        let mut tokens = Vec::new();
        let bytes = code.as_bytes();
        let mut at = 0usize;
        while at < bytes.len() {
            let byte = bytes[at];
            if byte.is_ascii_whitespace() {
                at += 1;
                continue;
            }
            let start = at;
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
            } else {
                at += 1;
            }
            let at32 = |offset: usize| Location {
                line: 1,
                column: offset as u32,
                offset: offset as u32,
            };
            tokens.push(Token {
                kind: TokenKind::Identifier,
                value: code[start..at].to_string(),
                start: at32(start),
                end: at32(at),
            });
        }
        SourceFile {
            id: id.to_string(),
            format: format.to_string(),
            tokens,
            bytes: code.len() as u64,
        }
    }

    fn cx(format: &str, code: &str) -> u64 {
        let sources = vec![lay_out("a", format, code)];
        compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity).files[0].complexity
    }

    #[test]
    fn short_circuit_operators_count_when_split_into_characters() {
        // The generic tokenizer hands `&&` over as two `&` tokens, so without
        // joining them the short-circuit arms of the shared list never match
        // for any format but JavaScript.
        assert_eq!(cx("c", "int f() { return a && b; }"), 2);
        assert_eq!(cx("c", "int f() { return a || b; }"), 2);
        assert_eq!(cx("c", "int f() { return a && b || c; }"), 3);
        // A single `&` is a bitwise and, not a branch.
        assert_eq!(cx("c", "int f() { return a & b; }"), 1);
    }

    #[test]
    fn an_optional_is_not_a_ternary() {
        // `String?` writes the question mark against its type; a ternary puts
        // it in the open. Nothing else tells them apart without parsing.
        assert_eq!(cx("swift", "func f(x: String?) -> Int { return 1 }"), 1);
        assert_eq!(cx("swift", "func f(x: Foo) { let y = x?.bar }"), 1);
        assert_eq!(
            cx("swift", "func f(x: Int) -> Int { return x > 0 ? 1 : 2 }"),
            2
        );
        // Nil-coalescing is a branch in any spelling.
        assert_eq!(
            cx("swift", "func f(a: Int?, b: Int) -> Int { return a ?? b }"),
            2
        );
        // A language where `?` only ever opens a ternary is unaffected.
        assert_eq!(cx("c", "int f(int x) { return x ? 1 : 2; }"), 2);
    }

    #[test]
    fn a_match_arm_is_the_branch_not_the_match() {
        // Three arms are three paths: one for the function, two branches.
        let three_arms = "fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, _ => 3 } }";
        assert_eq!(cx("rust", three_arms), 3);
        // A guard is a branch of its own on top of the arm it guards.
        let guarded = "fn f(x: i32, ok: bool) -> i32 { match x { 0 if ok => 1, 0 => 2, _ => 3 } }";
        assert_eq!(cx("rust", guarded), 4);
        // A match with a single arm does not branch at all.
        assert_eq!(cx("rust", "fn f(x: i32) -> i32 { match x { _ => 0 } }"), 1);
        // `=>` opens an arrow function in JavaScript, so it is a declaration
        // there rather than a branch.
        assert_eq!(cx("javascript", "const f = (x) => x + 1;"), 1);
    }

    #[test]
    fn complexity_counts_one_path_per_function() {
        // Four functions, three of them with a single branch.
        let code = "def a(n):\n if n: pass\ndef b(n):\n if n: pass\ndef c(n):\n if n: pass\ndef d(n):\n pass\n";
        assert_eq!(cx("python", code), 7);
        // A language with no marker the scan can trust keeps the per-file
        // baseline of one rather than guessing.
        assert_eq!(cx("cobol", "IF x THEN y"), 2);
    }

    #[test]
    fn guard_and_select_are_branches_where_they_exist() {
        assert_eq!(
            cx("swift", "func f(x: Int) { guard x > 0 else { return } }"),
            2
        );
        assert_eq!(cx("go", "func f() { select { } }"), 2);
        // `guard` is an ordinary word elsewhere.
        assert_eq!(cx("c", "int f() { int guard = 1; return guard; }"), 1);
    }

    #[test]
    fn elvis_branches_only_where_the_language_has_one() {
        // Kotlin's `?:` is the elvis operator.
        assert_eq!(
            cx("kotlin", "fun f(a: Int?, b: Int): Int { return a ?: b }"),
            2
        );
        // TypeScript has no elvis; `a?: T` marks an optional property.
        assert_eq!(cx("typescript", "function f(a?: string) { return a; }"), 1);
    }

    #[test]
    fn a_docstring_is_not_a_pile_of_branches() {
        // The real tokenizer closes every string at the end of its line, so a
        // docstring leaves a literal holding just its opening quote and its
        // body arrives as ordinary words.
        let quote = "\u{22}";
        let tokens = vec![
            lit_token("def", 0, 3, TokenKind::Identifier),
            lit_token("f", 4, 5, TokenKind::Identifier),
            lit_token(&quote.repeat(2), 6, 8, TokenKind::Literal),
            lit_token(quote, 8, 9, TokenKind::Literal),
            lit_token("if", 10, 12, TokenKind::Identifier),
            lit_token("for", 13, 16, TokenKind::Identifier),
            lit_token("while", 17, 22, TokenKind::Identifier),
            lit_token(&quote.repeat(2), 23, 25, TokenKind::Literal),
            lit_token(quote, 25, 26, TokenKind::Literal),
            lit_token("if", 27, 29, TokenKind::Identifier),
        ];
        let sources = vec![SourceFile {
            id: "a".into(),
            format: "python".into(),
            tokens,
            bytes: 30,
        }];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity);
        // One `def`, and only the `if` outside the docstring.
        assert_eq!(summary.files[0].complexity, 2);
    }

    #[test]
    fn a_docstring_with_its_summary_on_the_opening_line_closes_where_it_ends() {
        // PEP 257 style, as the tokenizer really hands it over: the opener is
        // `""` + `"Summary.` rather than a bare quote, so a reader that looked
        // for `""` + `"` alone took the *closing* quotes for an opener and
        // swallowed every branch after them.
        let q = "\u{22}";
        let tokens = vec![
            lit_token("def", 0, 3, TokenKind::Identifier),
            lit_token("f", 4, 5, TokenKind::Identifier),
            lit_token(&q.repeat(2), 10, 12, TokenKind::Literal),
            lit_token(&format!("{q}Summary."), 12, 21, TokenKind::Literal),
            lit_token("if", 30, 32, TokenKind::Identifier),
            lit_token("and", 33, 36, TokenKind::Identifier),
            lit_token(&q.repeat(2), 40, 42, TokenKind::Literal),
            lit_token(q, 42, 43, TokenKind::Literal),
            lit_token("if", 50, 52, TokenKind::Identifier),
            lit_token("x", 53, 54, TokenKind::Identifier),
            // A one-line docstring is balanced on its own line.
            lit_token(&q.repeat(2), 60, 62, TokenKind::Literal),
            lit_token(&format!("{q}One.{q}"), 62, 68, TokenKind::Literal),
            lit_token(&q.repeat(2), 68, 70, TokenKind::Literal),
            lit_token("for", 75, 78, TokenKind::Identifier),
        ];
        let sources = vec![SourceFile {
            id: "a".into(),
            format: "python".into(),
            tokens,
            bytes: 80,
        }];
        let summary = compute_summary(&sources, &[], 10, SummaryMetric::Complexity, identity);
        // One `def`, plus the `if` and the `for` written as code.
        assert_eq!(summary.files[0].complexity, 3);
    }

    #[test]
    fn a_doubled_quote_escape_is_not_a_triple_quoted_string() {
        // C# verbatim strings escape a quote by doubling it, so a run spelling
        // `"""x` is a quote followed by `x`, not a string that runs on. Read
        // as one, it swallowed the rest of a 413-branch file.
        let q = "\u{22}";
        let file = |format: &str| SourceFile {
            id: "a".into(),
            format: format.into(),
            tokens: vec![
                lit_token("x", 0, 1, TokenKind::Identifier),
                lit_token(&q.repeat(2), 4, 6, TokenKind::Literal),
                lit_token(&format!("{q}x"), 6, 8, TokenKind::Literal),
                lit_token("if", 10, 12, TokenKind::Identifier),
                lit_token("y", 13, 14, TokenKind::Identifier),
            ],
            bytes: 20,
        };
        let cx_of = |format: &str| {
            compute_summary(
                &[file(format)],
                &[],
                10,
                SummaryMetric::Complexity,
                identity,
            )
            .files[0]
                .complexity
        };
        assert_eq!(cx_of("csharp"), 2, "the `if` after the escape is code");
        // The same run in Python really does open a string.
        assert_eq!(cx_of("python"), 1);
    }

    fn lit_token(value: &str, start: u32, end: u32, kind: TokenKind) -> Token {
        Token {
            kind,
            value: value.to_string(),
            start: Location {
                line: 1,
                column: start,
                offset: start,
            },
            end: Location {
                line: 1,
                column: end,
                offset: end,
            },
        }
    }

    #[test]
    fn a_name_the_file_binds_is_not_a_keyword() {
        // `case` and `when` are branch keywords somewhere and ordinary
        // variables elsewhere; the tokenizer tags both `Identifier`. A file
        // that assigns to the word has settled the question for itself.
        let code = "def run(c):\n case = 3\n when = 4\n return case + when\n";
        assert_eq!(cx("python", code), 1);
        // Reading it off an object is the same evidence.
        assert_eq!(cx("python", "def f(o):\n return o.case\n"), 1);
        // So is listing it as a parameter or an argument.
        let listed = "def headline(case, when):\n return fmt(case, when)\n";
        assert_eq!(cx("python", listed), 1);
        // Rust's `?` before a closing paren is still a branch: only words are
        // taken as names.
        let question = "fn f(s: &str) -> Result<u8, E> { Ok(parse(s)?) }";
        assert_eq!(cx("rust", question), 2);
        // Without that evidence the keyword still counts: one `def`, plus
        // `case` and `when`.
        let ruby = "def f(x)\n case x\n when 1 then 2\n end\nend";
        assert_eq!(cx("ruby", ruby), 3);
    }

    #[test]
    fn the_c_family_declares_functions_without_a_keyword() {
        // `head(args) {` is the only marker C, C++, Java and C# give, and it
        // has to be told apart from the `) {` of a control statement.
        let code =
            "int add(int a, int b) { if (a > b) { return a; } return b; }\nvoid noop(void) { }\n";
        assert_eq!(cx("c", code), 3, "two functions plus one if");
        let java = "class T { int f(int a) { if (a > 0 && a < 10) { return a; } return 0; } }";
        assert_eq!(cx("java", java), 3, "one function, one if, one &&");
    }

    #[test]
    fn a_parenthesised_statement_is_not_a_function() {
        // `switch (x) {` and `for (…) {` end in `) {` just like a signature.
        // One function plus the single `case`; the `switch` head adds neither.
        assert_eq!(
            cx(
                "c",
                "int f(int x) { switch (x) { case 1: return 1; } return 0; }"
            ),
            2
        );
        assert_eq!(
            cx("c", "void f(void) { for (int i = 0; i < 3; i++) { } }"),
            2
        );
        assert_eq!(cx("c", "void f(void) { while (x) { } }"), 2);
        // A language that declares functions by keyword is unaffected by this.
        assert_eq!(cx("go", "func f() { if x { } }"), 2);
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
    fn gap_lines_of_a_merged_clone_stay_out_of_file_duplication() {
        let sources = vec![
            source("a.js", "javascript", &["x"; 20], 10),
            source("b.js", "javascript", &["x"; 20], 10),
        ];
        let mut merged = clone_between("javascript", "a.js", "b.js", 10, 60);
        merged.unmatched_lines = [0, 3];
        let summary = compute_summary(&sources, &[merged], 10, SummaryMetric::Tokens, identity);
        let dup = |path: &str| {
            summary
                .files
                .iter()
                .find(|f| f.path == path)
                .unwrap()
                .duplicated_lines
        };
        assert_eq!(dup("a.js"), 10);
        assert_eq!(dup("b.js"), 7, "three gap lines in b are not duplicated");
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
