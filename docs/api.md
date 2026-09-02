# Programming API

jscpd is a Rust engine. The CLI ships as the `jscpd` crate on crates.io (installs the `jscpd` and `cpd` binaries) and as the `jscpd` / `cpd` npm packages (prebuilt binaries); the engine itself is a set of library crates you can depend on directly.

## Rust

Use the `cpd-finder` crate — it is the orchestration entry point and pulls in the tokenizer, detection core, and reporters:

```toml
[dependencies]
cpd-finder = "0.1"
```

```rust
use cpd_finder::orchestrate::{RunConfig, run};

let config = RunConfig {
    paths: vec!["./src".into()],
    min_tokens: 50,
    ..Default::default()
};

let result = run(&config).unwrap();
println!("Found {} clones", result.clones.len());
println!("Analyzed {} files", result.statistics.total.sources);
```

A complete, runnable version is in [`examples/rust-cpd-finder`](../examples/rust-cpd-finder).

The `jscpd` crate's own library target is **not a public API** — it exists to share helpers with the crate's tests and may change in any release. Depend on the engine crates instead.

### Crate Architecture

| Crate | Description |
|-------|-------------|
| [`cpd-core`](https://crates.io/crates/cpd-core) | Core data models and hashing (Rabin-Karp rolling hash) |
| [`cpd-tokenizer`](https://crates.io/crates/cpd-tokenizer) | Source code tokenization (224 formats, uses `oxc_parser` for JavaScript/TypeScript) — pure, no I/O |
| [`cpd-finder`](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame (`rayon` + `ignore` + `globset`) |
| [`cpd-reporter`](https://crates.io/crates/cpd-reporter) | Output format rendering (15 reporters) |

See [Packages](./packages.md) for versions and paths.

## Other languages

There is no Node.js (or other language) binding for the v5 engine. From another language, run the binary and read the `json` reporter's output (`--reporters json --output <dir>` writes `<dir>/jscpd-report.json`), or talk to it over the [MCP stdio transport](./ai-ready.md#stdio-transport-rust-v5).

The TypeScript API of jscpd v4 (`jscpd()`, `detectClones()`, `@jscpd/core`, LevelDB/Redis stores) is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published as `jscpd@4`.
