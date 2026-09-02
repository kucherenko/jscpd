//! Minimal example of running the jscpd v5 engine from Rust.
//!
//! Usage: `cargo run -- [PATH]...` (defaults to the current directory).

use std::env;

use cpd_finder::orchestrate::{RunConfig, run};

fn main() {
    let mut paths: Vec<_> = env::args().skip(1).map(Into::into).collect();
    if paths.is_empty() {
        paths.push(".".into());
    }

    let config = RunConfig {
        paths,
        min_tokens: 50,
        ..Default::default()
    };

    match run(&config) {
        Ok(result) => {
            println!("Found {} clones", result.clones.len());
            println!("Analyzed {} files", result.statistics.total.sources);
            for clone in result.clones.iter().take(10) {
                println!(
                    "{}:{} <-> {}:{} ({} lines)",
                    clone.fragment_a.source_id,
                    clone.fragment_a.start.line,
                    clone.fragment_b.source_id,
                    clone.fragment_b.start.line,
                    clone.fragment_a.end.line - clone.fragment_a.start.line + 1,
                );
            }
        }
        Err(err) => {
            eprintln!("jscpd failed: {err}");
            std::process::exit(1);
        }
    }
}
