//! `--lsp`: jscpd as a language server on stdio (issue #1120).
//!
//! An editor starts `jscpd --lsp` for a workspace. The server splits the
//! workspace into projects by its `.jscpd.json` files, scans each project
//! once, and keeps the tokens in memory. The files open in the editor get
//! their findings as diagnostics, and an edit updates them from the text in
//! the editor, saved or not. `--lsp-analyses` and the `lsp` section of a
//! config choose the analyses; see [`settings`].
//!
//! stdout carries protocol messages only.

mod complexity;
mod dead_code;
mod findings;
mod index;
mod position;
mod project;
mod server;
pub mod settings;

use crate::cli::Cli;

/// Serve the language server protocol on stdin and stdout until the client
/// shuts the server down. Returns the exit code.
pub fn serve(cli: &Cli) -> i32 {
    // Three modes of the command line are analyses of the server.
    for (on, flag, name) in [
        (cli.semantic, "--semantic", "semantic"),
        (cli.dead_code, "--dead-code", "dead-code"),
        (cli.complexity, "--complexity", "complexity"),
    ] {
        if on {
            eprintln!(
                "Error: the argument '--lsp' cannot be used with '{flag}': the server runs it as an analysis, turn it on with --lsp-analyses {name}"
            );
            return 2;
        }
    }
    let defaults = match settings::parse_analyses(&cli.lsp_analyses) {
        Ok(analyses) => analyses,
        Err(error) => {
            eprintln!("Error: --lsp-analyses: {error}");
            return 1;
        }
    };
    let (connection, io_threads) = lsp_server::Connection::stdio();
    let result = server::run(connection, cli, defaults);
    let joined = io_threads.join();
    match (result, joined) {
        (Ok(()), Ok(())) => 0,
        (Err(error), _) => {
            eprintln!("Error: --lsp: {error}");
            1
        }
        (_, Err(error)) => {
            eprintln!("Error: --lsp: {error}");
            1
        }
    }
}
