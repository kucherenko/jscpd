// health.rs — one 0–100 score for a project, from how much of its code is
// duplicated, dead, or concentrated in complex files, plus whatever other
// tools (coverage, tests, security) are fed in.
//
// Every dimension is a share of code lines, so project size cancels out; size
// comes back in only to keep a small project from swinging on one finding.
// Each share becomes a 0–100 sub-score on a half-life curve, and the
// sub-scores are combined with a weighted geometric mean, which one bad
// dimension cannot hide behind the good ones.

use crate::deadcode::Stats as DeadCodeStats;
use crate::models::CpdClone;
use crate::summary::Summary;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A file is "complex" from this complexity up: about the top tenth of the
/// code files in the calibration corpus.
pub const COMPLEX_FILE: u64 = 50;

/// Lines of prior evidence mixed into each built-in dimension. One clone in a
/// 300-line project is 5%; at 2000 lines of prior it moves the share by a
/// fraction of that, and at 50K lines the prior no longer matters.
const PRIOR_LINES: f64 = 2000.0;

/// A dimension that reads less than this share of the code is left out.
const MIN_COVERAGE: f64 = 0.05;

/// What a built-in dimension looks like in a typical project — the median of
/// the calibration corpus — and the share at which its sub-score halves,
/// chosen so that the median project scores 75.
struct Calibration {
    median: f64,
    half_life: f64,
}

// Calibrated on 42 open-source projects (GitHub trending, 1.3K to 878K lines
// of code; 35 of them with JavaScript, TypeScript or Python for dead code).
const DUPLICATION: Calibration = Calibration {
    median: 3.5,
    half_life: 8.5,
};

/// Markup, stylesheets, declarative schemas and the templating languages
/// built on top of markup: a duplicated template or style rule repeating is
/// not the maintenance problem duplicated programming logic is, so it does
/// not count toward the duplication share at all — the same treatment
/// prose and data files get in [`compute`], just decided per clone rather
/// than per file, since a `.svelte` or `.vue` file's markup and style
/// blocks are tokenized separately from its script block. Public so a
/// format-level duplication breakdown can leave these rows out too.
///
/// These are the tokenizer's own format *names*
/// (`cpd-tokenizer/src/formats.rs`), not file extensions: html/htm/xml/svg
/// all tokenize as `markup`, `.puml`/`.plantuml` as `plant-uml`, `.tpl` as
/// `smarty`, `.jade` as `pug`, and `.vtl` as `velocity` — matching on the
/// extension instead of the name a clone's `format` field actually carries
/// would silently never exclude anything.
pub fn is_markup(format: &str) -> bool {
    matches!(
        format,
        "markup"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "stylus"
            | "razor"
            | "haml"
            | "pug"
            | "handlebars"
            | "erb"
            | "liquid"
            | "twig"
            | "velocity"
            | "ftl"
            | "soy"
            | "smarty"
            | "tt2"
            | "protobuf"
            | "plant-uml"
            | "mermaid"
            | "django"
            | "aspnet"
    )
}

/// Prose: half of [`crate::summary::has_control_flow`]'s denylist, split
/// out so the duplication line can name which category of "not code" a
/// project actually has, instead of a fixed disclaimer.
fn is_text(format: &str) -> bool {
    matches!(
        format,
        "markdown" | "asciidoc" | "rest" | "textile" | "wiki" | "txt" | "log" | "diff" | "gettext"
    )
}

/// Data: the other half of the same denylist.
fn is_data(format: &str) -> bool {
    matches!(
        format,
        "csv"
            | "json"
            | "json5"
            | "yaml"
            | "toml"
            | "ini"
            | "properties"
            | "editorconfig"
            | "ignore"
    )
}
const DEAD_CODE: Calibration = Calibration {
    median: 3.1,
    half_life: 7.5,
};
const COMPLEXITY: Calibration = Calibration {
    median: 20.9,
    half_life: 50.0,
};

/// Overrides for one built-in dimension (config key `health.<dimension>`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tuning {
    /// The share, in percent, at which the sub-score is 50.
    pub half_life: Option<f64>,
    /// Weight in the mean; `0` leaves the dimension out.
    pub weight: Option<f64>,
}

/// Which way an external metric improves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Less is healthier: failing tests, vulnerabilities per KLOC.
    #[default]
    Lower,
    /// More is healthier: test coverage. Scored on the distance to `max`.
    Higher,
}

/// A measurement from another tool. Either a ready `score` (0–100), or a
/// `value` with the `halfLife` that turns it into one.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalMetric {
    pub id: String,
    pub score: Option<f64>,
    pub value: Option<f64>,
    #[serde(default)]
    pub direction: Direction,
    /// The best possible `value` of a `higher` metric (default 100).
    pub max: Option<f64>,
    pub half_life: Option<f64>,
    pub weight: Option<f64>,
}

/// The `health` config object, also the shape of a `--health-input` file
/// (which normally carries only `metrics`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HealthConfig {
    #[serde(default)]
    pub duplication: Tuning,
    #[serde(default)]
    pub dead_code: Tuning,
    #[serde(default)]
    pub complexity: Tuning,
    /// Complexity from which a file counts as complex (default 50).
    pub complex_file: Option<u64>,
    #[serde(default)]
    pub metrics: Vec<ExternalMetric>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    /// Lines of code files; prose and data are not counted.
    pub lines: u64,
    pub files: u64,
    /// XS under 1K lines, S under 10K, M under 100K, L under 1M, then XL.
    pub class: &'static str,
}

/// One scored dimension.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimension {
    pub id: String,
    /// `jscpd` for the built-in dimensions, `external` for the rest.
    pub source: &'static str,
    /// What was measured: a share of lines in percent for the built-in
    /// dimensions, the tool's own value for an external one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// `value` after the small-project prior was mixed in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted: Option<f64>,
    /// The lines behind `value`: duplicated, dead, or in complex files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub half_life: Option<f64>,
    /// Percent of the code lines the dimension could analyze, when that is
    /// not all of them: dead code reads JavaScript, TypeScript and Python
    /// only. `weight` is already scaled by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// The code formats `value` was measured over, most-lines first; only
    /// set for `duplication`, where markup and prose/data formats are left
    /// out and a reader may want to know what is left.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<String>,
    /// Which category labels apply to what was left out of `value`, of
    /// `markup`, `text` and `data`; only the categories this project
    /// actually has files in are listed. Only set for `duplication`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<&'static str>,
    pub weight: f64,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skipped {
    pub id: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// `None` when nothing could be scored (no code and no external metric).
    pub score: Option<f64>,
    /// `A` from 85, `B` from 70, `C` from 55, `D` from 40, else `E`.
    pub grade: Option<char>,
    pub size: Size,
    pub dimensions: Vec<Dimension>,
    /// Built-in dimensions that could not be measured, and why. The score is
    /// the mean of the rest, so two scores are comparable only when they are
    /// built from the same dimensions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skipped>,
}

impl HealthConfig {
    /// Lay `other` (a `--health-input` file) over `self` (the config file):
    /// its metrics are added, and any tuning it sets wins.
    pub fn merge(mut self, other: HealthConfig) -> Self {
        for (mine, theirs) in [
            (&mut self.duplication, other.duplication),
            (&mut self.dead_code, other.dead_code),
            (&mut self.complexity, other.complexity),
        ] {
            mine.half_life = theirs.half_life.or(mine.half_life);
            mine.weight = theirs.weight.or(mine.weight);
        }
        self.complex_file = other.complex_file.or(self.complex_file);
        self.metrics.extend(other.metrics);
        self
    }
}

pub fn grade(score: f64) -> char {
    match score {
        s if s >= 85.0 => 'A',
        s if s >= 70.0 => 'B',
        s if s >= 55.0 => 'C',
        s if s >= 40.0 => 'D',
        _ => 'E',
    }
}

fn size_class(lines: u64) -> &'static str {
    match lines {
        0..1_000 => "XS",
        1_000..10_000 => "S",
        10_000..100_000 => "M",
        100_000..1_000_000 => "L",
        _ => "XL",
    }
}

/// 100 at zero, 50 at one half-life, 25 at two: no cliff and no dead zone.
fn half_life_score(value: f64, half_life: f64) -> f64 {
    100.0 * 2f64.powf(-value.max(0.0) / half_life)
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// A generous ceiling on any one weight: comfortably above any reasonable
/// weighting scheme (typical weights are 0.1-10), and small enough that
/// summing a handful of them — as the geometric mean does — cannot overflow
/// to `f64::INFINITY` and turn the score into `inf / inf = NaN`.
const MAX_WEIGHT: f64 = 1e6;

/// Check what serde cannot: a metric must be scorable and every number sane.
pub fn validate(config: &HealthConfig) -> Result<(), String> {
    for (name, tuning) in [
        ("duplication", &config.duplication),
        ("deadCode", &config.dead_code),
        ("complexity", &config.complexity),
    ] {
        if tuning.half_life.is_some_and(|h| h.is_nan() || h <= 0.0) {
            return Err(format!("{name}.halfLife must be greater than 0"));
        }
        if tuning
            .weight
            .is_some_and(|w| !w.is_finite() || !(0.0..=MAX_WEIGHT).contains(&w))
        {
            return Err(format!("{name}.weight must be between 0 and {MAX_WEIGHT}"));
        }
    }
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for metric in &config.metrics {
        let id = &metric.id;
        if id.trim().is_empty() {
            return Err("every metric needs an id".to_string());
        }
        if matches!(id.as_str(), "duplication" | "dead-code" | "complexity") {
            return Err(format!("metric '{id}': that id is a built-in dimension"));
        }
        if !seen_ids.insert(id.as_str()) {
            return Err(format!("metric '{id}': id given more than once"));
        }
        if metric
            .weight
            .is_some_and(|w| !w.is_finite() || !(0.0..=MAX_WEIGHT).contains(&w))
        {
            return Err(format!(
                "metric '{id}': weight must be between 0 and {MAX_WEIGHT}"
            ));
        }
        match (metric.score, metric.value, metric.half_life) {
            (Some(score), _, _) if !(0.0..=100.0).contains(&score) => {
                return Err(format!("metric '{id}': score must be between 0 and 100"));
            }
            (Some(_), _, _) => {}
            (None, Some(value), Some(half_life)) if value.is_finite() && half_life > 0.0 => {}
            (None, Some(_), Some(_)) => {
                return Err(format!(
                    "metric '{id}': value must be a number and halfLife greater than 0"
                ));
            }
            _ => {
                return Err(format!(
                    "metric '{id}': give either a score (0-100) or a value with a halfLife"
                ));
            }
        }
    }
    Ok(())
}

/// Duplicated lines in code files, counted the way jscpd's own percentage
/// counts them — the matched lines of each clone's primary fragment — so the
/// health score and `--threshold` speak about the same number. A clone
/// whose primary fragment sits in a prose or data file is not counted, and
/// neither is one whose own format [`is_markup`]: a duplicated template or
/// style rule is not duplicated code, even inside an otherwise full-weight
/// file such as a `.svelte` or `.vue` component.
fn duplicated_code_lines(clones: &[CpdClone], code: &[&crate::summary::FileSummary]) -> u64 {
    let code_paths: HashSet<&str> = code.iter().map(|f| f.path.as_str()).collect();
    clones
        .iter()
        .filter(|clone| {
            if is_markup(&clone.format) {
                return false;
            }
            // A sub-format fragment (`App.vue:typescript`) belongs to its file.
            let id = &clone.fragment_a.source_id;
            let path = id.strip_suffix(&format!(":{}", clone.format)).unwrap_or(id);
            code_paths.contains(path)
        })
        .map(CpdClone::matched_lines)
        .sum()
}

/// A built-in dimension: `problem` lines out of `total`, pulled toward the
/// corpus median by the small-project prior.
fn built_in(
    id: &str,
    problem: u64,
    total: u64,
    calibration: &Calibration,
    tuning: &Tuning,
) -> Dimension {
    let total = total as f64;
    let value = problem as f64 / total * 100.0;
    let adjusted = (value * total + calibration.median * PRIOR_LINES) / (total + PRIOR_LINES);
    let half_life = tuning.half_life.unwrap_or(calibration.half_life);
    Dimension {
        id: id.to_string(),
        source: "jscpd",
        value: Some(round1(value)),
        adjusted: Some(round1(adjusted)),
        lines: Some(problem),
        half_life: Some(half_life),
        coverage: None,
        formats: Vec::new(),
        excluded: Vec::new(),
        weight: tuning.weight.unwrap_or(1.0),
        score: round1(half_life_score(adjusted, half_life)),
    }
}

fn external(metric: &ExternalMetric) -> Dimension {
    let score = metric.score.unwrap_or_else(|| {
        let value = metric.value.unwrap_or_default();
        let distance = match metric.direction {
            Direction::Lower => value,
            Direction::Higher => metric.max.unwrap_or(100.0) - value,
        };
        half_life_score(distance, metric.half_life.unwrap_or(1.0))
    });
    Dimension {
        id: metric.id.clone(),
        source: "external",
        value: metric.value,
        adjusted: None,
        lines: None,
        half_life: metric.half_life.filter(|_| metric.score.is_none()),
        coverage: None,
        formats: Vec::new(),
        excluded: Vec::new(),
        weight: metric.weight.unwrap_or(1.0),
        score: round1(score),
    }
}

/// Score a project.
///
/// `summary` must list every file (no top-N cut) with the report paths the
/// `clones` carry; `dead_code` is `None` when no file is in a language the
/// dead-code analysis reads. `config` is expected to have passed [`validate`].
pub fn compute(
    summary: &Summary,
    clones: &[CpdClone],
    dead_code: Option<&DeadCodeStats>,
    config: &HealthConfig,
) -> Health {
    // Prose and data files have complexity 0 and are not the project's code:
    // a copied JSON snapshot is not a maintenance problem.
    let code: Vec<_> = summary.files.iter().filter(|f| f.complexity > 0).collect();
    let code_lines: u64 = code.iter().map(|f| f.lines).sum();
    // Markup is code (it has complexity), but its duplication does not count
    // ([`is_markup`]); the share duplication is measured over has to exclude
    // it on both sides, or unrelated markup dilutes the share for free —
    // adding one unique, un-duplicated HTML file would lower a project's
    // duplication percentage without a single duplicated line changing.
    let non_markup_lines: u64 = code
        .iter()
        .filter(|f| !is_markup(&f.format))
        .map(|f| f.lines)
        .sum();
    let mut dimensions = Vec::new();
    let mut skipped = Vec::new();

    if non_markup_lines == 0 {
        skipped.push(Skipped {
            id: "duplication",
            reason: match code_lines {
                0 => "no code files",
                _ => "every code file is markup",
            },
        });
    } else {
        // N-way copies are reported as pairs, which can count a line twice.
        let duplicated = duplicated_code_lines(clones, &code).min(non_markup_lines);
        let mut duplication = built_in(
            "duplication",
            duplicated,
            non_markup_lines,
            &DUPLICATION,
            &config.duplication,
        );
        // What `value` was actually measured over: every code format that
        // is not markup, most-lines first, so a reader can tell "5.4%" apart
        // from "5.4% of a mostly-Python project" without reading the docs.
        let mut format_lines: HashMap<&str, u64> = HashMap::new();
        for f in code.iter().filter(|f| !is_markup(&f.format)) {
            *format_lines.entry(f.format.as_str()).or_insert(0) += f.lines;
        }
        let mut counted: Vec<(&str, u64)> = format_lines.into_iter().collect();
        counted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        duplication.formats = counted.into_iter().map(|(f, _)| f.to_string()).collect();
        // Only name a category this project actually has files in: a pure
        // JavaScript project should not be told "text and data" are excluded
        // when it has neither.
        if summary.files.iter().any(|f| is_markup(&f.format)) {
            duplication.excluded.push("markup");
        }
        if summary.files.iter().any(|f| is_text(&f.format)) {
            duplication.excluded.push("text");
        }
        if summary.files.iter().any(|f| is_data(&f.format)) {
            duplication.excluded.push("data");
        }
        dimensions.push(duplication);
    }

    if code_lines == 0 {
        skipped.push(Skipped {
            id: "complexity",
            reason: "no code files",
        });
    } else {
        let complex_file = config.complex_file.unwrap_or(COMPLEX_FILE);
        let complex: u64 = code
            .iter()
            .filter(|f| f.complexity >= complex_file)
            .map(|f| f.lines)
            .sum();
        dimensions.push(built_in(
            "complexity",
            complex,
            code_lines,
            &COMPLEXITY,
            &config.complexity,
        ));
    }
    // Share of the code the dead-code analysis could read.
    let readable = |stats: &DeadCodeStats| f64::from(stats.total_lines) / code_lines.max(1) as f64;
    match dead_code.filter(|stats| stats.total_lines > 0) {
        // Ten TypeScript fixtures in a Rust project say nothing about it.
        Some(stats) if readable(stats) < MIN_COVERAGE => skipped.push(Skipped {
            id: "dead-code",
            reason: "JavaScript, TypeScript and Python are under 5% of the code",
        }),
        Some(stats) => {
            let mut dimension = built_in(
                "dead-code",
                u64::from(stats.dead_lines),
                u64::from(stats.total_lines),
                &DEAD_CODE,
                &config.dead_code,
            );
            // A dimension weighs as much as the code it could read.
            let share = readable(stats);
            if share < 1.0 {
                dimension.coverage = Some(round1(share * 100.0));
                dimension.weight = (dimension.weight * share * 1000.0).round() / 1000.0;
            }
            dimensions.insert(dimensions.len().min(1), dimension);
        }
        None => skipped.push(Skipped {
            id: "dead-code",
            reason: "no JavaScript, TypeScript or Python files",
        }),
    }
    dimensions.extend(config.metrics.iter().map(external));
    dimensions.retain(|d| d.weight > 0.0);

    // Weighted geometric mean. A sub-score is floored at 1 so that one zero
    // does not erase every other dimension.
    let weight: f64 = dimensions.iter().map(|d| d.weight).sum();
    let score = (weight > 0.0).then(|| {
        let log_sum: f64 = dimensions
            .iter()
            .map(|d| d.weight * d.score.max(1.0).ln())
            .sum();
        round1((log_sum / weight).exp())
    });

    Health {
        score,
        grade: score.map(grade),
        size: Size {
            lines: code_lines,
            files: code.len() as u64,
            class: size_class(code_lines),
        },
        dimensions,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CloneKind, Fragment, Location};
    use crate::summary::{FileSummary, SummaryMetric};

    fn file(path: &str, lines: u64, complexity: u64) -> FileSummary {
        FileSummary {
            path: path.to_string(),
            format: "javascript".to_string(),
            lines,
            tokens: lines * 8,
            bytes: lines * 30,
            duplicated_lines: 0,
            duplicated_tokens: 0,
            complexity,
        }
    }

    fn summary(files: Vec<FileSummary>) -> Summary {
        Summary {
            by: SummaryMetric::Complexity,
            total_files: files.len() as u64,
            total_folders: 1,
            files,
            folders: vec![],
        }
    }

    fn fragment(path: &str, start: u32, end: u32) -> Fragment {
        Fragment {
            source_id: path.to_string(),
            source_root: None,
            start: Location::new(start, 0, 0),
            end: Location::new(end, 0, 0),
            range: [0, 0],
            blame: None,
        }
    }

    fn clone(a: Fragment, b: Fragment) -> CpdClone {
        CpdClone {
            format: "javascript".to_string(),
            fragment_a: a,
            fragment_b: b,
            token_count: 60,
            is_new: false,
            kind: CloneKind::Exact,
            similarity: None,
            similarity_method: None,
            unmatched_lines: [0, 0],
        }
    }

    fn dimension<'a>(health: &'a Health, id: &str) -> &'a Dimension {
        health.dimensions.iter().find(|d| d.id == id).unwrap()
    }

    #[test]
    fn the_curve_halves_every_half_life() {
        assert_eq!(half_life_score(0.0, 7.0), 100.0);
        assert!((half_life_score(7.0, 7.0) - 50.0).abs() < 1e-9);
        assert!((half_life_score(14.0, 7.0) - 25.0).abs() < 1e-9);
    }

    #[test]
    fn grades_follow_the_thresholds() {
        let grades: String = [92.0, 85.0, 84.9, 70.0, 55.0, 40.0, 39.9]
            .map(grade)
            .iter()
            .collect();
        assert_eq!(grades, "AABBCDE");
    }

    #[test]
    fn a_clean_large_project_scores_high() {
        let files = (0..100)
            .map(|i| file(&format!("src/{i}.js"), 500, 12))
            .collect();
        let health = compute(&summary(files), &[], None, &HealthConfig::default());
        assert_eq!(health.size.lines, 50_000);
        assert_eq!(health.size.class, "M");
        assert_eq!(health.grade, Some('A'));
        assert_eq!(dimension(&health, "duplication").value, Some(0.0));
        assert_eq!(health.skipped[0].id, "dead-code");
    }

    #[test]
    fn duplication_lists_the_formats_it_was_measured_over() {
        let mut py_file = file("src/a.py", 300, 5);
        py_file.format = "python".to_string();
        let mut css_file = file("src/a.css", 50, 5);
        css_file.format = "css".to_string();
        let files = vec![py_file, file("src/b.js", 100, 5), css_file];
        let health = compute(&summary(files), &[], None, &HealthConfig::default());
        let duplication = dimension(&health, "duplication");
        assert_eq!(
            duplication.formats,
            vec!["python".to_string(), "javascript".to_string()],
            "most lines first, css left out entirely"
        );
    }

    #[test]
    fn duplication_is_counted_the_way_the_statistics_count_it() {
        let files = vec![file("src/a.js", 100, 5), file("src/App.vue", 100, 5)];
        let mut vue = clone(
            fragment("src/App.vue:javascript", 3, 13),
            fragment("src/a.js", 40, 50),
        );
        vue.format = "javascript".to_string();
        let clones = vec![
            clone(fragment("src/a.js", 1, 11), fragment("src/b.js", 1, 11)),
            vue,
        ];
        let health = compute(&summary(files), &clones, None, &HealthConfig::default());
        let duplication = dimension(&health, "duplication");
        assert_eq!(
            duplication.lines,
            Some(20),
            "10 + 10: one fragment per clone"
        );
        assert_eq!(duplication.value, Some(10.0));
    }

    #[test]
    fn markup_duplication_does_not_count_at_all() {
        let mut css_file = file("src/a.css", 100, 5);
        css_file.format = "css".to_string();
        let files = vec![file("src/b.js", 100, 5), css_file];

        let mut css_clone = clone(fragment("src/a.css", 1, 51), fragment("src/c.css", 1, 51));
        css_clone.format = "css".to_string();
        let health = compute(
            &summary(files),
            &[css_clone],
            None,
            &HealthConfig::default(),
        );
        let duplication = dimension(&health, "duplication");

        // The css file's 100 lines still count toward the total (it is
        // still code), but its duplication contributes nothing.
        assert_eq!(duplication.lines, Some(0));
        assert_eq!(duplication.value, Some(0.0));
    }

    #[test]
    fn markup_exclusion_follows_the_clone_format_not_the_containing_file() {
        // A `.svelte` file is a full-weight format on its own, but its style
        // block is tokenized as its own `css` sub-format and excluded like
        // any other css clone — the exclusion follows what was duplicated,
        // not what the file is declared as.
        let mut svelte_file = file("src/Card.svelte", 100, 5);
        svelte_file.format = "svelte".to_string();
        let files = vec![svelte_file];
        let mut css_clone = clone(
            fragment("src/Card.svelte:css", 1, 51),
            fragment("other/Card.svelte:css", 1, 51),
        );
        css_clone.format = "css".to_string();
        css_clone.fragment_a.source_id = "src/Card.svelte:css".to_string();

        let health = compute(
            &summary(files),
            &[css_clone],
            None,
            &HealthConfig::default(),
        );
        let duplication = dimension(&health, "duplication");
        assert_eq!(duplication.lines, Some(0));
        assert_eq!(duplication.value, Some(0.0));
    }

    #[test]
    fn a_javascript_clone_beside_an_excluded_css_clone_still_counts() {
        let mut svelte_file = file("src/Card.svelte", 100, 5);
        svelte_file.format = "svelte".to_string();
        let files = vec![file("src/a.js", 100, 5), svelte_file];

        let js_clone = clone(fragment("src/a.js", 1, 41), fragment("other/a.js", 1, 41));
        let mut css_clone = clone(
            fragment("src/Card.svelte:css", 1, 51),
            fragment("other/Card.svelte:css", 1, 51),
        );
        css_clone.format = "css".to_string();
        css_clone.fragment_a.source_id = "src/Card.svelte:css".to_string();

        let health = compute(
            &summary(files),
            &[js_clone, css_clone],
            None,
            &HealthConfig::default(),
        );
        let duplication = dimension(&health, "duplication");
        // 40 js lines out of 200 unweighted total; the 50 css lines are not
        // in the numerator at all.
        assert_eq!(duplication.lines, Some(40));
        assert_eq!(duplication.value, Some(20.0));
    }

    #[test]
    fn unrelated_markup_does_not_dilute_the_duplication_share() {
        // Two fully-duplicated 100-line JS files: 50%, the matched lines of
        // one side out of both files' lines (the usual jscpd convention).
        let js_files = vec![file("a.js", 100, 5), file("b.js", 100, 5)];
        let js_clone = clone(fragment("a.js", 1, 101), fragment("b.js", 1, 101));
        let before = compute(
            &summary(js_files.clone()),
            std::slice::from_ref(&js_clone),
            None,
            &HealthConfig::default(),
        );

        // Add one large, entirely unique (never duplicated) HTML file. The
        // duplication share must not move: markup lines were never in the
        // numerator, so they must not be in the denominator either, or
        // adding unrelated markup would look like it lowered duplication.
        let mut html = file("index.html", 2000, 5);
        html.format = "markup".to_string();
        let mut files = js_files;
        files.push(html);
        let after = compute(&summary(files), &[js_clone], None, &HealthConfig::default());

        let before_dup = dimension(&before, "duplication");
        let after_dup = dimension(&after, "duplication");
        assert_eq!(before_dup.value, Some(50.0));
        assert_eq!(
            after_dup.value, before_dup.value,
            "unrelated markup changed the duplication share: {after_dup:?}"
        );
    }

    #[test]
    fn an_all_markup_project_skips_duplication_rather_than_scoring_from_the_prior_alone() {
        let mut html = file("index.html", 500, 5);
        html.format = "markup".to_string();
        let health = compute(&summary(vec![html]), &[], None, &HealthConfig::default());
        assert!(
            health.dimensions.iter().all(|d| d.id != "duplication"),
            "duplication should be skipped, not scored from the prior alone: {:?}",
            health.dimensions
        );
        assert_eq!(
            health
                .skipped
                .iter()
                .find(|s| s.id == "duplication")
                .map(|s| s.reason),
            Some("every code file is markup")
        );
        // Complexity is still measurable: markup files are code, just not
        // counted toward duplication.
        assert!(health.dimensions.iter().any(|d| d.id == "complexity"));
    }

    #[test]
    fn prose_and_data_are_not_the_projects_code() {
        let files = vec![
            file("src/a.js", 100, 5),
            file("data/snapshot.json", 9000, 0),
        ];
        let clones = vec![clone(
            fragment("data/snapshot.json", 1, 9000),
            fragment("data/snapshot.json", 1, 9000),
        )];
        let health = compute(&summary(files), &clones, None, &HealthConfig::default());
        assert_eq!(health.size.lines, 100);
        assert_eq!(dimension(&health, "duplication").lines, Some(0));
    }

    #[test]
    fn a_small_project_does_not_swing_on_one_clone() {
        let small = vec![file("a.js", 100, 3), file("b.js", 100, 3)];
        let clones = vec![clone(fragment("a.js", 1, 41), fragment("b.js", 1, 41))];
        let health = compute(&summary(small), &clones, None, &HealthConfig::default());
        let duplication = dimension(&health, "duplication");
        assert_eq!(duplication.value, Some(20.0));
        assert!(
            duplication.adjusted.unwrap() < 6.0,
            "200 lines against 2000 lines of prior: {duplication:?}"
        );
    }

    #[test]
    fn complexity_is_the_share_of_code_in_complex_files() {
        let files = vec![file("big.js", 3000, 200), file("small.js", 1000, 10)];
        let health = compute(&summary(files), &[], None, &HealthConfig::default());
        let complexity = dimension(&health, "complexity");
        assert_eq!(complexity.value, Some(75.0));
        assert_eq!(complexity.lines, Some(3000));
    }

    #[test]
    fn one_bad_dimension_is_not_averaged_away() {
        let files = (0..40)
            .map(|i| file(&format!("src/{i}.js"), 500, 10))
            .collect();
        let dead = DeadCodeStats {
            files: 40,
            dead_lines: 8000,
            total_lines: 20_000,
            percentage: 40.0,
            ..Default::default()
        };
        let health = compute(&summary(files), &[], Some(&dead), &HealthConfig::default());
        let scores: Vec<f64> = health.dimensions.iter().map(|d| d.score).collect();
        let arithmetic = scores.iter().sum::<f64>() / scores.len() as f64;
        let score = health.score.unwrap();
        assert!(
            score < 40.0 && arithmetic > 60.0,
            "{score} vs {arithmetic}: {scores:?}"
        );
        assert_eq!(health.dimensions[1].id, "dead-code", "kept between the two");
    }

    #[test]
    fn dead_code_weighs_as_much_as_the_code_it_could_read() {
        let mut files: Vec<_> = (0..80)
            .map(|i| file(&format!("src/{i}.rs"), 500, 10))
            .collect();
        files.push(file("tests/fixture.ts", 400, 4));
        let dead = DeadCodeStats {
            files: 1,
            dead_lines: 200,
            total_lines: 400,
            percentage: 50.0,
            ..Default::default()
        };
        let health = compute(&summary(files), &[], Some(&dead), &HealthConfig::default());
        assert!(health.dimensions.iter().all(|d| d.id != "dead-code"));
        assert_eq!(health.skipped[0].id, "dead-code", "400 of 40,400 lines");
        assert_eq!(
            health.grade,
            Some('A'),
            "one half-dead fixture is not the project"
        );

        // A third of the code in TypeScript: scored, at a third of the weight.
        let mut mixed: Vec<_> = (0..20)
            .map(|i| file(&format!("src/{i}.rs"), 500, 10))
            .collect();
        mixed.extend((0..10).map(|i| file(&format!("web/{i}.ts"), 500, 10)));
        let dead = DeadCodeStats {
            files: 10,
            dead_lines: 50,
            total_lines: 5000,
            percentage: 1.0,
            ..Default::default()
        };
        let health = compute(&summary(mixed), &[], Some(&dead), &HealthConfig::default());
        let dead_code = dimension(&health, "dead-code");
        assert_eq!(dead_code.coverage, Some(33.3));
        assert_eq!(dead_code.weight, 0.333);
    }

    #[test]
    fn external_metrics_join_the_mean() {
        let files = vec![file("a.js", 5000, 10)];
        let config = HealthConfig {
            metrics: vec![
                ExternalMetric {
                    id: "coverage".to_string(),
                    value: Some(60.0),
                    direction: Direction::Higher,
                    half_life: Some(40.0),
                    ..Default::default()
                },
                ExternalMetric {
                    id: "security".to_string(),
                    score: Some(90.0),
                    weight: Some(2.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        validate(&config).unwrap();
        let health = compute(&summary(files), &[], None, &config);
        assert_eq!(
            dimension(&health, "coverage").score,
            50.0,
            "40 short of 100"
        );
        assert_eq!(dimension(&health, "security").weight, 2.0);
        assert_eq!(health.dimensions.len(), 4);
    }

    #[test]
    fn a_zero_weight_leaves_a_dimension_out() {
        let files = vec![file("big.js", 5000, 300)];
        let config = HealthConfig {
            complexity: Tuning {
                weight: Some(0.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let health = compute(&summary(files), &[], None, &config);
        assert!(health.dimensions.iter().all(|d| d.id != "complexity"));
    }

    #[test]
    fn nothing_to_score_is_not_a_perfect_score() {
        let health = compute(&summary(vec![]), &[], None, &HealthConfig::default());
        assert_eq!((health.score, health.grade), (None, None));
        assert_eq!(health.skipped.len(), 3);
    }

    #[test]
    fn an_input_file_adds_metrics_and_overrides_tuning() {
        let config: HealthConfig =
            serde_json::from_str(r#"{"duplication": {"halfLife": 5, "weight": 2}}"#).unwrap();
        let input: HealthConfig = serde_json::from_str(
            r#"{"duplication": {"halfLife": 9}, "metrics": [{"id": "tests", "score": 100}]}"#,
        )
        .unwrap();
        let merged = config.merge(input);
        assert_eq!(merged.duplication.half_life, Some(9.0));
        assert_eq!(merged.duplication.weight, Some(2.0), "kept from the config");
        assert_eq!(merged.metrics.len(), 1);
    }

    #[test]
    fn unusable_metrics_are_refused() {
        let bad = |metric: ExternalMetric| {
            validate(&HealthConfig {
                metrics: vec![metric],
                ..Default::default()
            })
        };
        let named = |id: &str| ExternalMetric {
            id: id.to_string(),
            ..Default::default()
        };
        assert!(bad(named("empty")).unwrap_err().contains("either a score"));
        assert!(
            bad(ExternalMetric {
                score: Some(140.0),
                ..named("over")
            })
            .is_err()
        );
        assert!(
            bad(ExternalMetric {
                value: Some(3.0),
                half_life: Some(0.0),
                ..named("flat")
            })
            .is_err()
        );
    }

    #[test]
    fn a_huge_weight_is_refused_rather_than_scoring_nan() {
        let err = validate(&HealthConfig {
            duplication: Tuning {
                weight: Some(1e308),
                ..Default::default()
            },
            complexity: Tuning {
                weight: Some(1e308),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("weight"), "{err}");

        let err = validate(&HealthConfig {
            metrics: vec![ExternalMetric {
                id: "x".to_string(),
                score: Some(50.0),
                weight: Some(f64::INFINITY),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("weight"), "{err}");
    }

    #[test]
    fn a_metric_id_cannot_clash_with_a_built_in_or_repeat() {
        let clashes_with_builtin = validate(&HealthConfig {
            metrics: vec![ExternalMetric {
                id: "duplication".to_string(),
                score: Some(90.0),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(clashes_with_builtin.is_err());

        let repeated = validate(&HealthConfig {
            metrics: vec![
                ExternalMetric {
                    id: "coverage".to_string(),
                    score: Some(90.0),
                    ..Default::default()
                },
                ExternalMetric {
                    id: "coverage".to_string(),
                    score: Some(10.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        assert!(repeated.is_err());
    }

    #[test]
    fn the_config_reads_camel_case_and_rejects_typos() {
        let config: HealthConfig = serde_json::from_str(
            r#"{"deadCode": {"halfLife": 5}, "complexFile": 80,
                "metrics": [{"id": "coverage", "value": 81, "direction": "higher", "halfLife": 40}]}"#,
        )
        .unwrap();
        assert_eq!(config.dead_code.half_life, Some(5.0));
        assert_eq!(config.complex_file, Some(80));
        assert_eq!(config.metrics[0].direction, Direction::Higher);
        assert!(serde_json::from_str::<HealthConfig>(r#"{"deadcode": {}}"#).is_err());
    }
}
