// health_render.rs — the project health score as a console badge, a compact
// line for the `ai` reporter, and an SVG badge.

use crate::badge::make_badge;
use crate::dashboard::thousands;
use crate::shared::Style;
use cpd_core::health::{Dimension, Health};

const BAR: usize = 20;

/// Green for A and B, yellow for C and D, red for E; dim when unscored.
fn ansi_color(grade: Option<char>) -> u8 {
    match grade {
        Some('A' | 'B') => 32,
        Some('C' | 'D') => 33,
        Some(_) => 31,
        None => 90,
    }
}

fn svg_color(grade: Option<char>) -> &'static str {
    match grade {
        Some('A') => "#27ae60",
        Some('B') => "#7cb342",
        Some('C') => "#f1c40f",
        Some('D') => "#f39c12",
        Some(_) => "#e74c3c",
        None => "#9f9f9f",
    }
}

/// `dead-code` reads better as `dead code` in a sentence.
fn label(id: &str) -> String {
    id.replace('-', " ")
}

/// What a sub-score was computed from, in the dimension's own terms.
fn measured(dimension: &Dimension) -> Option<String> {
    let value = dimension.value?;
    let share = match (dimension.source, dimension.id.as_str()) {
        ("jscpd", "complexity") => format!("{value:.1}% in complex files"),
        ("jscpd", _) => format!("{value:.1}%"),
        _ => format!("{value}"),
    };
    Some(match dimension.coverage {
        Some(coverage) => format!("{share}, reads {coverage:.0}% of the code"),
        None => share,
    })
}

/// The badge: grade, score, a bar, the size of the project, then one line
/// with every sub-score and what it was measured from.
pub fn print_badge(health: &Health, style: &Style) {
    let color = ansi_color(health.grade);
    let headline = match (health.score, health.grade) {
        (Some(score), Some(grade)) => {
            let filled = ((score / 100.0 * BAR as f64).round() as usize).min(BAR);
            format!(
                "{} {}  {}{}",
                style.bold(&style.paint(&format!(" {grade} "), color)),
                style.bold(&format!("{score:.0}/100")),
                style.paint(&"█".repeat(filled), color),
                style.dim(&"░".repeat(BAR - filled)),
            )
        }
        _ => style.dim("not scored: no code files and no external metrics"),
    };
    println!(
        "{} {headline}  {}",
        style.bold("Health"),
        style.dim(&format!(
            "{} lines of code ({})",
            thousands(health.size.lines),
            health.size.class
        ))
    );
    let mut parts: Vec<String> = health
        .dimensions
        .iter()
        .map(|d| {
            let score = style.paint(
                &format!("{:.0}", d.score),
                ansi_color(Some(cpd_core::health::grade(d.score))),
            );
            match measured(d) {
                Some(from) => format!(
                    "{} {score} {}",
                    label(&d.id),
                    style.dim(&format!("({from})"))
                ),
                None => format!("{} {score}", label(&d.id)),
            }
        })
        .collect();
    parts.extend(
        health
            .skipped
            .iter()
            .map(|s| style.dim(&format!("{} n/a", label(s.id)))),
    );
    if !parts.is_empty() {
        println!("  {}", parts.join(" · "));
    }
}

/// One line, no padding or colour: the `ai` reporter's form.
pub fn print_compact(health: &Health) {
    let dimensions = health
        .dimensions
        .iter()
        .map(|d| format!("{} {:.0}", d.id, d.score))
        .collect::<Vec<_>>()
        .join(", ");
    match (health.score, health.grade) {
        (Some(score), Some(grade)) => println!(
            "health {score:.0} {grade} ({dimensions}; {} code lines)",
            health.size.lines
        ),
        _ => println!("health n/a (no code files)"),
    }
}

/// A shields-style SVG: `health | B 73`.
pub fn svg(health: &Health) -> String {
    let value = match (health.score, health.grade) {
        (Some(score), Some(grade)) => format!("{grade} {score:.0}"),
        _ => "n/a".to_string(),
    };
    make_badge("health", &value, svg_color(health.grade))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::health::{Size, Skipped};

    fn sample() -> Health {
        Health {
            score: Some(73.4),
            grade: Some('B'),
            size: Size {
                lines: 43_390,
                files: 105,
                class: "M",
            },
            dimensions: vec![Dimension {
                id: "dead-code".to_string(),
                source: "jscpd",
                value: Some(1.2),
                adjusted: Some(1.3),
                lines: Some(520),
                half_life: Some(7.3),
                coverage: None,
                weight: 1.0,
                score: 88.4,
            }],
            skipped: vec![Skipped {
                id: "complexity",
                reason: "no code files",
            }],
        }
    }

    #[test]
    fn the_svg_carries_grade_and_score() {
        let svg = svg(&sample());
        assert!(svg.contains(">B 73<") && svg.contains("#7cb342"), "{svg}");
    }

    #[test]
    fn an_unscored_project_gets_a_grey_badge() {
        let health = Health {
            score: None,
            grade: None,
            ..sample()
        };
        assert!(svg(&health).contains(">n/a<"));
        print_badge(&health, &Style::new(true));
        print_compact(&health);
    }

    #[test]
    fn a_built_in_share_is_shown_as_a_percentage() {
        assert_eq!(measured(&sample().dimensions[0]).unwrap(), "1.2%");
        assert_eq!(label("dead-code"), "dead code");
        print_badge(&sample(), &Style::new(false));
        print_compact(&sample());
    }
}
