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
    // The threads that read stdin and write stdout are not joined: the
    // reader waits for stdin to close, and the exit code does not.
    let (connection, _io_threads) = lsp_server::Connection::stdio();
    let cli = cli.clone();
    let server = std::thread::Builder::new()
        .name("jscpd-lsp".to_string())
        // The parsers recurse, and a file in an editor can nest deeper than
        // a main thread's stack allows (1 MiB on Windows); the scan's pool
        // threads have the same size.
        .stack_size(64 * 1024 * 1024)
        .spawn(move || server::run(connection, &cli, defaults));
    let result = match server {
        Ok(handle) => handle
            .join()
            .unwrap_or_else(|_| Err("the server stopped".to_string())),
        Err(error) => Err(error.to_string()),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Error: --lsp: {error}");
            1
        }
    }
}
