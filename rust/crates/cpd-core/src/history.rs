//! Duplication and health trend over git history (`--history`, issue #1002).
//!
//! One [`HistoryPoint`] per scanned commit, oldest first, plus a final point
//! for the working tree. The CLI collects the points by scanning each commit
//! in a temporary worktree with the run's own configuration; this module only
//! holds the data model and the pure helpers reporters need (sparkline,
//! per-point change, health trend, threshold hint). Nothing here touches
//! git.

use crate::health::Health;
use serde::{Deserialize, Serialize};

/// Identifier used for the working-tree point instead of a commit hash.
pub const WORKING_TREE: &str = "working tree";

/// Detection totals for one commit (or the working tree).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    /// Full commit hash, or [`WORKING_TREE`] for the uncommitted state.
    pub commit: String,
    /// Abbreviated hash for display (7 characters), or `working`.
    pub short: String,
    /// Committer date as `YYYY-MM-DD`; the detection date for the working tree.
    pub date: String,
    /// First line of the commit message; empty for the working tree.
    pub subject: String,
    pub sources: u64,
    pub lines: u64,
    pub tokens: u64,
    pub clones: u64,
    pub duplicated_lines: u64,
    /// Duplicated lines as a percentage of all lines.
    pub percentage: f64,
    /// The health score of the tree at this point; `None` when it was not
    /// scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<PointHealth>,
}

impl HistoryPoint {
    pub fn is_working_tree(&self) -> bool {
        self.commit == WORKING_TREE
    }

    /// The health score, when the point has one.
    pub fn health_score(&self) -> Option<f64> {
        self.health.as_ref().and_then(|health| health.score)
    }
}

/// The health score of one point, the one `--health` gives the same tree,
/// from the dimensions jscpd measures itself: duplication, dead code and
/// complexity. The metrics of other tools (`--health-input`) describe the
/// working tree alone, so no point of a series is scored with them, the
/// working tree's included: a series is comparable only when every point is
/// built from the same kind of evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointHealth {
    /// 0-100; `None` when the tree had no code to score.
    pub score: Option<f64>,
    /// `A` to `E`, as `--health` grades the score.
    pub grade: Option<char>,
    /// Lines of code files, the base of every share.
    pub code_lines: u64,
    /// The scored dimensions, in the order `--health` lists them. One that
    /// could not be measured at this point, such as dead code in a tree
    /// with no JavaScript, TypeScript or Python, is absent.
    pub dimensions: Vec<PointDimension>,
}

/// One scored dimension of a [`PointHealth`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointDimension {
    /// `duplication`, `dead-code` or `complexity`.
    pub id: String,
    /// The share of the code lines it found, in percent.
    pub value: Option<f64>,
    /// Its 0-100 sub-score.
    pub score: f64,
}

impl From<&Health> for PointHealth {
    /// The score, grade, size and dimensions of `health`, which the caller
    /// computed without external metrics.
    fn from(health: &Health) -> Self {
        Self {
            score: health.score,
            grade: health.grade,
            code_lines: health.size.lines,
            dimensions: health
                .dimensions
                .iter()
                .map(|d| PointDimension {
                    id: d.id.clone(),
                    value: d.value,
                    score: d.score,
                })
                .collect(),
        }
    }
}

impl PointHealth {
    /// The sub-score of dimension `id`, when it was scored.
    pub fn dimension(&self, id: &str) -> Option<&PointDimension> {
        self.dimensions.iter().find(|d| d.id == id)
    }
}

/// The series a `--history` run produces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    /// What was walked, for display: `v5.0.0..HEAD`, `since 2026-01-01`.
    pub range: String,
    /// `--threshold` in effect, if any; drives the tightening hint.
    pub threshold: Option<f64>,
    /// Oldest first; the last point is the working tree.
    pub points: Vec<HistoryPoint>,
}

/// Levels used by [`sparkline`], lowest to highest.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One character per value, scaled between the series' min and max. A flat
/// series renders at mid height so it still reads as "present, unchanged".
pub fn sparkline(values: &[f64]) -> String {
    let (min, max) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        });
    values
        .iter()
        .map(|v| {
            if max <= min {
                BARS[3]
            } else {
                let level = ((v - min) / (max - min) * (BARS.len() - 1) as f64).round() as usize;
                BARS[level.min(BARS.len() - 1)]
            }
        })
        .collect()
}

/// The dimensions one point was scored from and the latest point was not,
/// and the other way round. A score built from other dimensions than the
/// latest one is not directly comparable with it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DimensionGap {
    /// Scored at the latest point, not at this one.
    pub missing: Vec<String>,
    /// Scored at this point, not at the latest one.
    pub extra: Vec<String>,
}

impl DimensionGap {
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }
}

impl History {
    /// Percentages in series order.
    pub fn percentages(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.percentage).collect()
    }

    pub fn sparkline(&self) -> String {
        sparkline(&self.percentages())
    }

    /// Health scores in series order; `None` where a point has none.
    pub fn health_scores(&self) -> Vec<Option<f64>> {
        self.points.iter().map(HistoryPoint::health_score).collect()
    }

    /// Whether any point has a health score.
    pub fn has_health(&self) -> bool {
        self.points.iter().any(|p| p.health_score().is_some())
    }

    /// The first point with a health score and the latest point, which must
    /// have one too: the span the health trend covers. `None` when they are
    /// the same point.
    pub fn health_trend(&self) -> Option<(&HistoryPoint, &HistoryPoint)> {
        let last = self.points.len().checked_sub(1)?;
        self.points[last].health_score()?;
        let first = self
            .points
            .iter()
            .position(|p| p.health_score().is_some())?;
        (first < last).then(|| (&self.points[first], &self.points[last]))
    }

    /// How the dimensions point `index` was scored from differ from the
    /// latest point's. Empty unless both have a score.
    pub fn dimension_gap(&self, index: usize) -> DimensionGap {
        fn scored(p: &HistoryPoint) -> Option<&PointHealth> {
            p.health.as_ref().filter(|h| h.score.is_some())
        }
        let (Some(point), Some(last)) = (
            self.points.get(index).and_then(scored),
            self.points.last().and_then(scored),
        ) else {
            return DimensionGap::default();
        };
        let not_in = |of: &PointHealth, other: &PointHealth| -> Vec<String> {
            of.dimensions
                .iter()
                .filter(|d| other.dimension(&d.id).is_none())
                .map(|d| d.id.clone())
                .collect()
        };
        DimensionGap {
            missing: not_in(last, point),
            extra: not_in(point, last),
        }
    }

    /// Change in percentage points from the previous point; `None` for the
    /// first one.
    pub fn change_at(&self, index: usize) -> Option<f64> {
        if index == 0 || index >= self.points.len() {
            return None;
        }
        Some(self.points[index].percentage - self.points[index - 1].percentage)
    }

    /// Change in percentage points from the first to the last point.
    pub fn overall_change(&self) -> Option<f64> {
        match (self.points.first(), self.points.last()) {
            (Some(first), Some(last)) if self.points.len() > 1 => {
                Some(last.percentage - first.percentage)
            }
            _ => None,
        }
    }

    /// Room between the threshold and the latest value, in percentage points,
    /// when the latest value is below the threshold. This is what `--ratchet`
    /// would apply automatically; jscpd only reports it.
    pub fn threshold_headroom(&self) -> Option<f64> {
        let threshold = self.threshold?;
        let last = self.points.last()?;
        let headroom = threshold - last.percentage;
        (headroom > 0.05).then_some(headroom)
    }

    pub fn commit_count(&self) -> usize {
        self.points.iter().filter(|p| !p.is_working_tree()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(short: &str, percentage: f64) -> HistoryPoint {
        HistoryPoint {
            commit: if short == "working" {
                WORKING_TREE.to_string()
            } else {
                format!("{short}0000000000000000000000000000000000")
            },
            short: short.to_string(),
            date: "2026-09-12".to_string(),
            subject: String::new(),
            sources: 10,
            lines: 1000,
            tokens: 5000,
            clones: 2,
            duplicated_lines: (percentage * 10.0) as u64,
            percentage,
            health: None,
        }
    }

    /// A health score from `dimensions`, each `(id, score)`.
    fn scored(score: Option<f64>, dimensions: &[(&str, f64)]) -> PointHealth {
        PointHealth {
            score,
            grade: score.map(crate::health::grade),
            code_lines: 900,
            dimensions: dimensions
                .iter()
                .map(|(id, score)| PointDimension {
                    id: id.to_string(),
                    value: Some(1.0),
                    score: *score,
                })
                .collect(),
        }
    }

    /// A series with these health scores, `None` for a point without one.
    fn health_history(scores: &[Option<f64>]) -> History {
        let mut h = history(&vec![1.0; scores.len()], None);
        for (point, score) in h.points.iter_mut().zip(scores) {
            point.health = Some(scored(*score, &[("duplication", 80.0)]));
        }
        h
    }

    fn history(values: &[f64], threshold: Option<f64>) -> History {
        History {
            range: "a..b".to_string(),
            threshold,
            points: values
                .iter()
                .enumerate()
                .map(|(i, v)| point(&format!("c{i}"), *v))
                .collect(),
        }
    }

    #[test]
    fn sparkline_scales_between_min_and_max() {
        assert_eq!(sparkline(&[0.0, 50.0, 100.0]), "▁▅█");
        assert_eq!(sparkline(&[1.0, 1.0, 1.0]), "▄▄▄");
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn change_at_is_difference_to_previous_point() {
        let h = history(&[2.0, 3.5, 3.0], None);
        assert_eq!(h.change_at(0), None);
        assert!((h.change_at(1).unwrap() - 1.5).abs() < 1e-9);
        assert!((h.change_at(2).unwrap() + 0.5).abs() < 1e-9);
        assert_eq!(h.change_at(3), None);
    }

    #[test]
    fn overall_change_spans_first_to_last() {
        assert!((history(&[4.0, 1.0, 2.5], None).overall_change().unwrap() + 1.5).abs() < 1e-9);
        assert_eq!(history(&[4.0], None).overall_change(), None);
    }

    #[test]
    fn threshold_headroom_only_when_below_threshold() {
        assert!(
            (history(&[3.0, 2.1], Some(5.0))
                .threshold_headroom()
                .unwrap()
                - 2.9)
                .abs()
                < 1e-9
        );
        assert_eq!(history(&[3.0, 6.0], Some(5.0)).threshold_headroom(), None);
        assert_eq!(history(&[3.0, 5.0], Some(5.0)).threshold_headroom(), None);
        assert_eq!(history(&[3.0, 2.0], None).threshold_headroom(), None);
    }

    #[test]
    fn commit_count_excludes_working_tree() {
        let mut h = history(&[1.0, 2.0], None);
        h.points.push(point("working", 2.0));
        assert_eq!(h.commit_count(), 2);
        assert!(h.points[2].is_working_tree());
    }

    #[test]
    fn json_uses_camel_case() {
        let json = serde_json::to_string(&history(&[1.5], Some(3.0))).unwrap();
        assert!(json.contains("\"duplicatedLines\""));
        assert!(json.contains("\"threshold\":3.0"));
        assert!(!json.contains("\"health\""), "absent unless scored: {json}");

        let json = serde_json::to_string(&health_history(&[Some(74.2)])).unwrap();
        assert!(json.contains("\"health\":{\"score\":74.2,\"grade\":\"B\",\"codeLines\":900,\"dimensions\":[{\"id\":\"duplication\""), "{json}");
        let back: History = serde_json::from_str(&json).unwrap();
        assert_eq!(back, health_history(&[Some(74.2)]));
    }

    #[test]
    fn a_point_takes_the_score_and_dimensions_of_a_health() {
        let files = (0..10)
            .map(|i| crate::summary::FileSummary {
                path: format!("src/{i}.js"),
                format: "javascript".to_string(),
                lines: 500,
                tokens: 4000,
                bytes: 15000,
                duplicated_lines: 0,
                duplicated_tokens: 0,
                complexity: 10,
            })
            .collect();
        let summary = crate::summary::Summary {
            by: crate::summary::SummaryMetric::Complexity,
            total_files: 10,
            total_folders: 1,
            files,
            folders: vec![],
        };
        let health = crate::health::compute(&summary, &[], None, &Default::default());
        let point = PointHealth::from(&health);
        assert_eq!((point.score, point.grade), (health.score, health.grade));
        assert_eq!(point.code_lines, 5000);
        let ids: Vec<&str> = point.dimensions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["duplication", "complexity"]);
        assert_eq!(point.dimension("duplication").unwrap().value, Some(0.0));
        assert!(point.dimension("dead-code").is_none());
    }

    #[test]
    fn health_scores_leave_a_gap_where_a_point_has_none() {
        let h = health_history(&[Some(70.0), None, Some(60.0)]);
        assert_eq!(h.health_scores(), [Some(70.0), None, Some(60.0)]);
        assert!(h.has_health());
        assert!(!history(&[1.0, 2.0], None).has_health());
        assert!(!health_history(&[None, None]).has_health());
    }

    #[test]
    fn the_health_trend_runs_from_the_first_scored_point_to_the_latest() {
        let h = health_history(&[None, Some(70.0), Some(72.0), Some(61.0)]);
        let (first, last) = h.health_trend().unwrap();
        assert_eq!((first.short.as_str(), last.short.as_str()), ("c1", "c3"));
        assert_eq!(health_history(&[Some(70.0), None]).health_trend(), None);
        assert_eq!(health_history(&[None, Some(70.0)]).health_trend(), None);
        assert_eq!(history(&[1.0, 2.0], None).health_trend(), None);
    }

    #[test]
    fn the_dimension_gap_compares_a_point_with_the_latest() {
        let mut h = history(&[1.0, 1.0, 1.0, 1.0], None);
        let both = [("duplication", 80.0), ("complexity", 70.0)];
        h.points[0].health = Some(scored(Some(75.0), &both[..1]));
        h.points[1].health = Some(scored(None, &[]));
        h.points[2].health = Some(scored(
            Some(70.0),
            &[("duplication", 80.0), ("dead-code", 60.0)],
        ));
        h.points[3].health = Some(scored(Some(74.0), &both));
        assert_eq!(
            h.dimension_gap(0),
            DimensionGap {
                missing: vec!["complexity".to_string()],
                extra: vec![],
            }
        );
        assert!(h.dimension_gap(1).is_empty(), "nothing scored there");
        assert_eq!(
            h.dimension_gap(2),
            DimensionGap {
                missing: vec!["complexity".to_string()],
                extra: vec!["dead-code".to_string()],
            }
        );
        assert!(h.dimension_gap(3).is_empty());
        assert!(h.dimension_gap(9).is_empty());
    }
}
