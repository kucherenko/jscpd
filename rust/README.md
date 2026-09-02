# jscpd — Rust engine

Copy/paste detector for programming source code. A self-contained binary that finds duplicated blocks across 224 language formats, writes reports in 15 formats and can fail CI when duplication grows.

Published as [`jscpd`](https://www.npmjs.com/package/jscpd) on npm (installs the `jscpd` command), [`cpd`](https://www.npmjs.com/package/cpd) on npm (installs the `cpd` command), and [`jscpd`](https://crates.io/crates/jscpd) on crates.io (installs both). Full documentation: [jscpd.dev](https://jscpd.dev) and [docs/rust.md](../docs/rust.md).

## Install

### npm

```bash
# installs the jscpd command
npm install -g jscpd

# installs only the cpd command
npm install -g cpd
```

Prebuilt binaries for 8 platforms (macOS arm64/x64, Linux arm64/x64 with glibc or musl, Windows arm64/x64) — no Node.js runtime required.

### crates.io

```bash
cargo install jscpd
# or, without compiling (prebuilt release binaries via cargo-binstall):
cargo binstall jscpd
```

Installs both `jscpd` and `cpd` binaries.

### Nix

```bash
# Run without installing
nix run github:kucherenko/jscpd -- /path/to/code

# Install permanently
nix profile install github:kucherenko/jscpd
```

### Homebrew

```bash
brew install jscpd
```

### From source

```bash
git clone https://github.com/kucherenko/jscpd.git
cd jscpd/rust
cargo build --release
# binaries at target/release/jscpd and target/release/cpd
```

## Quick Start

```bash
jscpd .
jscpd ./src ./lib
jscpd . --blame --reporters console-full
jscpd . --reporters json,html
jscpd . --threshold 10
jscpd --baseline-from-ref origin/main --fail-on-new-clones .
jscpd --mcp .
jscpd --list
```

## Architecture

```
jscpd/cpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (224 formats)
 ├── cpd-finder    — File walking, orchestration, baseline, git blame
 └── cpd-reporter  — Output formatting (15 reporters)
```

| Crate | Purpose |
|-------|---------|
| [`jscpd`](https://crates.io/crates/jscpd) | CLI binary and entry point (`crates/cpd`) |
| [`cpd-core`](https://crates.io/crates/cpd-core) | Detection algorithm, rolling hash, models |
| [`cpd-tokenizer`](https://crates.io/crates/cpd-tokenizer) | Language tokenization (224 formats); pure, no I/O |
| [`cpd-finder`](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame |
| [`cpd-reporter`](https://crates.io/crates/cpd-reporter) | Output formatting (15 reporters) |

Current versions are in each crate's `Cargo.toml`; `scripts/sync-version.mjs` keeps them and the npm packages in step. The workspace layout, npm launcher packages and platform packages are described in [docs/packages.md](../docs/packages.md).

## Programmatic Usage (Rust)

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

See [docs/api.md](../docs/api.md) and [examples/rust-cpd-finder](../examples/rust-cpd-finder).

## Building and testing

Requires Rust 1.96+ (the MSRV declared in `Cargo.toml`, enforced by the `msrv`
job in CI). Development uses the exact toolchain pinned in `rust-toolchain.toml`.

```bash
cargo build --release
cargo nextest run --workspace        # or: cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# End-to-end run over the multi-format corpus (what CI's smoke job does)
./target/release/jscpd ../fixtures --reporters console,json --output ../smoke-report --min-tokens 50
```

After changing the format table in `crates/cpd-tokenizer/src/formats.rs`, regenerate the root [FORMATS.md](../FORMATS.md) with `node scripts/gen-formats-md.mjs`.

## Benchmarks

Compared against other copy/paste detectors (jscpd-rs, Duplo, Fallow, Simian, PMD CPD) on the repository's `fixtures/` corpus — timing, detection counts, cross-format detection and AI-token efficiency — in [benchmark/BENCHMARK.md](../benchmark/BENCHMARK.md). Re-run with [`benchmark/benchmark.sh`](../benchmark/benchmark.sh).

## Documentation

- **[docs/rust.md](../docs/rust.md)** — Full CLI reference, all options, reporters, baseline, summary, config file
- **[docs/ai-ready.md](../docs/ai-ready.md)** — AI reporter, agent skills, MCP server
- **[docs/api.md](../docs/api.md)** — Rust API
- **[docs/ci-and-hooks.md](../docs/ci-and-hooks.md)** — GitHub Action, Docker, pre-commit hooks
- **[CHANGELOG.md](CHANGELOG.md)** — Release notes

## Coming from jscpd v4?

The CLI flags, `.jscpd.json` config and reporters are the same; see the [migration table](../docs/rust.md#migrating-from-jscpd-v4) for the few differences. jscpd v4 (TypeScript engine, Node.js API, LevelDB/Redis stores) is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published as `jscpd@4`.

## License

MIT
