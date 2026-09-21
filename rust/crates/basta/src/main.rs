//! The `basta` binary.

use basta::cli::{Cli, resolve};
use basta::run::run_and_report;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    if cli.list {
        for format in basta::lang::supported_formats() {
            println!("{format}");
        }
        return;
    }

    let (config, output, diagnostics) = resolve(&cli);
    for diagnostic in &diagnostics {
        diagnostic.print();
    }
    if diagnostics.iter().any(basta::cli::Diagnostic::is_error) {
        std::process::exit(1);
    }

    if cli.list_frameworks {
        println!("{}", basta::cli::describe_frameworks(&config.frameworks));
        return;
    }

    if cli.debug {
        println!("{}", basta::cli::describe(&config, &output));
        return;
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    std::process::exit(run_and_report(&config, &output).exit_code);
}
