//! Detection plus reporting, as one call.
//!
//! Both front ends go through here — the `basta` binary and jscpd's
//! `--dead-code` — so a dead-code run behaves identically whichever one
//! starts it: the same reporter selection, the same output directory, the
//! same exit codes.

use crate::analyze;
use crate::config::BastaConfig;
use cpd_reporter::deadcode::{DeadCodeContext, create_dead_code_reporter};
use cpd_reporter::reporter::{ReporterError, ReporterOptions};
use std::path::PathBuf;
use std::time::Instant;

/// How a run should present itself.
#[derive(Debug, Clone)]
pub struct OutputOptions {
    /// Reporter names, in the order they should run.
    pub reporters: Vec<String>,
    /// Directory file-writing reporters write into.
    pub output_dir: PathBuf,
    pub no_colors: bool,
    /// Suppress console output; file reporters still write.
    pub silent: bool,
    /// Fail when dead lines exceed this percentage of the codebase.
    pub threshold: Option<f64>,
    /// Exit with this code when the run found anything at all.
    pub exit_code: Option<i32>,
    /// Version stamped into SARIF and the HTML footer.
    pub tool_version: String,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            reporters: vec!["console".to_string()],
            output_dir: PathBuf::from("report"),
            no_colors: false,
            silent: false,
            threshold: None,
            exit_code: None,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Reporters that write to stdout rather than to a file.
fn is_console_reporter(name: &str) -> bool {
    matches!(
        normalize_reporter_name(name),
        "ai" | "console" | "console-full" | "silent" | "xcode"
    )
}

/// Fold the accepted spellings of a reporter name onto one.
pub fn normalize_reporter_name(name: &str) -> &str {
    match name {
        "full" | "consoleFull" => "console-full",
        "gitlab" => "codeclimate",
        other => other,
    }
}

/// Outcome of a run, for a caller that wants more than an exit code.
pub struct Outcome {
    pub report: cpd_core::deadcode::Report,
    /// Process exit code: 0 clean, 1 for a threshold breach or a reporter
    /// failure, or `--exit-code` when the run found something.
    pub exit_code: i32,
}

/// Run detection and every selected reporter.
pub fn run_and_report(config: &BastaConfig, output: &OutputOptions) -> Outcome {
    let started = Instant::now();
    let result = analyze::run(config);
    let elapsed = started.elapsed();

    let reporter_options = ReporterOptions {
        output_dir: output.output_dir.clone(),
        threshold: output.threshold,
        blame: false,
        no_colors: output.no_colors,
        blame_data: Default::default(),
        absolute: false,
        tool_version: output.tool_version.clone(),
        sarif_error_tokens: None,
    };

    // `--silent` drops the reporters that write to stdout but keeps the ones
    // that write files, so a CI job can stay quiet and still publish a report.
    let mut names: Vec<String> = output.reporters.clone();
    if output.silent {
        names.retain(|name| !is_console_reporter(name));
    }
    names.retain(|name| normalize_reporter_name(name) != "threshold");
    // The threshold gate runs last, once, whether it was asked for by name or
    // implied by `--threshold`.
    let wants_threshold = output.threshold.is_some()
        || output
            .reporters
            .iter()
            .any(|name| normalize_reporter_name(name) == "threshold");

    // Console reporters first so their output precedes the "saved to" lines.
    let (console, files): (Vec<String>, Vec<String>) = names
        .into_iter()
        .partition(|name| is_console_reporter(name));

    let roots = config.paths.clone();
    let mut threshold_exceeded = false;
    let mut reporter_failed = false;
    let mut run_batch = |batch: &[String]| {
        for name in batch {
            let Some(reporter) =
                create_dead_code_reporter(normalize_reporter_name(name), &reporter_options)
            else {
                eprintln!("Warning: unknown reporter '{name}'");
                continue;
            };
            let ctx = DeadCodeContext::new(&result.report.statistics, elapsed).with_roots(&roots);
            match reporter.report(&result.report.findings, &ctx, &output.output_dir) {
                Ok(()) => {}
                Err(ReporterError::ThresholdExceeded { actual, threshold }) => {
                    eprintln!(
                        "ERROR: basta found too much dead code ({actual:.1}%) over threshold ({threshold:.1}%)"
                    );
                    threshold_exceeded = true;
                }
                Err(error) => {
                    eprintln!("Reporter '{name}' error: {error}");
                    reporter_failed = true;
                }
            }
        }
    };
    run_batch(&console);
    run_batch(&files);
    if wants_threshold {
        run_batch(&["threshold".to_string()]);
    }

    let exit_code = if reporter_failed {
        eprintln!("ERROR: a reporter failed to write its output (see the message above)");
        1
    } else if threshold_exceeded {
        1
    } else {
        match output.exit_code {
            Some(code) if !result.report.findings.is_empty() => code,
            _ => 0,
        }
    };
    Outcome {
        report: result.report,
        exit_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::deadcode::Category;

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("basta-run-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/index.ts"), "import './api';\n").unwrap();
        std::fs::write(
            root.join("src/api.ts"),
            "export function neverImported() {\n  return 1;\n}\n",
        )
        .unwrap();
        std::fs::write(root.join("src/orphan.ts"), "export const x = 1;\n").unwrap();
        root
    }

    fn config(root: &std::path::Path) -> BastaConfig {
        BastaConfig {
            paths: vec![root.to_path_buf()],
            categories: Category::ALL.to_vec(),
            min_confidence: 0,
            ..BastaConfig::default()
        }
    }

    #[test]
    fn a_clean_run_exits_zero() {
        let root = std::env::temp_dir().join(format!("basta-run-clean-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.ts"), "export const a = 1;\n").unwrap();
        let outcome = run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["silent".to_string()],
                ..OutputOptions::default()
            },
        );
        assert!(outcome.report.findings.is_empty());
        assert_eq!(outcome.exit_code, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn exit_code_is_returned_only_when_something_was_found() {
        let root = fixture("exit");
        let options = OutputOptions {
            reporters: vec!["silent".to_string()],
            exit_code: Some(3),
            ..OutputOptions::default()
        };
        let outcome = run_and_report(&config(&root), &options);
        assert!(!outcome.report.findings.is_empty());
        assert_eq!(outcome.exit_code, 3);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_threshold_gate_runs_even_when_it_was_not_named() {
        let root = fixture("threshold");
        let outcome = run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["silent".to_string()],
                threshold: Some(0.0),
                ..OutputOptions::default()
            },
        );
        assert_eq!(
            outcome.exit_code, 1,
            "--threshold implies the gate without naming the reporter"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_threshold_that_is_not_reached_passes() {
        let root = fixture("threshold-ok");
        let outcome = run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["silent".to_string()],
                threshold: Some(99.0),
                ..OutputOptions::default()
            },
        );
        assert_eq!(outcome.exit_code, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn file_reporters_write_into_the_output_directory() {
        let root = fixture("files");
        let output_dir = root.join("report");
        let outcome = run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["json".to_string(), "sarif".to_string(), "html".to_string()],
                output_dir: output_dir.clone(),
                no_colors: true,
                ..OutputOptions::default()
            },
        );
        assert_eq!(outcome.exit_code, 0);
        for file in [
            "basta-report.json",
            "basta-report.sarif",
            "basta-report.html",
        ] {
            assert!(output_dir.join(file).is_file(), "{file} was not written");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn silent_keeps_file_reporters_and_drops_console_ones() {
        let root = fixture("silent");
        let output_dir = root.join("report");
        run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["console".to_string(), "json".to_string()],
                output_dir: output_dir.clone(),
                silent: true,
                no_colors: true,
                ..OutputOptions::default()
            },
        );
        assert!(
            output_dir.join("basta-report.json").is_file(),
            "a quiet CI job still needs its artifact"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unknown_reporter_warns_instead_of_failing_the_run() {
        let root = fixture("unknown");
        let outcome = run_and_report(
            &config(&root),
            &OutputOptions {
                reporters: vec!["nonsense".to_string(), "silent".to_string()],
                ..OutputOptions::default()
            },
        );
        assert_eq!(outcome.exit_code, 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reporter_aliases_resolve_the_way_jscpd_spells_them() {
        assert_eq!(normalize_reporter_name("full"), "console-full");
        assert_eq!(normalize_reporter_name("consoleFull"), "console-full");
        assert_eq!(normalize_reporter_name("gitlab"), "codeclimate");
        assert_eq!(normalize_reporter_name("json"), "json");
    }
}
