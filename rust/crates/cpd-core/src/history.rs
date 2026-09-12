//! Duplication trend over git history (`--history`, issue #1002).
//!
//! One [`HistoryPoint`] per scanned commit, oldest first, plus a final point
//! for the working tree. The CLI collects the points by scanning each commit
//! in a temporary worktree with the run's own configuration; this module only
//! holds the data model and the pure helpers reporters need (sparkline,
//! per-point change, threshold hint). Nothing here touches git.

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
}

impl HistoryPoint {
    pub fn is_working_tree(&self) -> bool {
        self.commit == WORKING_TREE
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

impl History {
    /// Percentages in series order.
    pub fn percentages(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.percentage).collect()
    }

    pub fn sparkline(&self) -> String {
        sparkline(&self.percentages())
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
        }
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
    }
}
