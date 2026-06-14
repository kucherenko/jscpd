# cpd — Rust Copy/Paste Detector

Fast copy/paste detector for programming source code. 24-37x faster than Node.js. Rust rewrite of [jscpd](https://github.com/kucherenko/jscpd), supports 223 language formats.

Also available as an npm package: [`jscpd@5`](https://www.npmjs.com/package/jscpd) (installs both `jscpd` and `cpd` commands) or [`cpd`](https://www.npmjs.com/package/cpd) (installs `cpd` command only).

## Performance

| Codebase | Files | Size | jscpd v4 (Node.js) | cpd v5 (Rust) | Speedup |
|----------|-------|------|--------------------|----------------|---------|
| Multi-format fixtures | 548 | 1.5 MB | 1.03 s | 0.03 s | 34.3× |
| Svelte source | 9K | 38 MB | 15.80 s | 0.43 s | 36.9× |
| CopilotKit | 17K | 159 MB | 82.89 s | 3.44 s | 24.1× |

See [performance-comparison.md](../docs/performance-comparison.md) for full methodology and raw data.

## Install

### npm (recommended)

```bash
# installs both jscpd and cpd commands
npm install -g jscpd

# installs only the cpd command
npm install -g cpd
```

Prebuilt binaries for 6 platforms — no Node.js runtime required.

### crates.io

```bash
cargo install jscpd
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
cpd .
cpd ./src ./lib
cpd . --blame --reporters console-full
cpd . --reporters json,html
cpd . --threshold 10
cpd --list
```

## Architecture

```
jscpd/cpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (223 formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (13 reporters)
```

| Crate | Version | Purpose |
|-------|---------|---------|
| `cpd-core` | [0.1.3](https://crates.io/crates/cpd-core) | Detection algorithm, rolling hash, models |
| `cpd-tokenizer` | [0.1.3](https://crates.io/crates/cpd-tokenizer) | Language tokenization (223 formats) |
| `cpd-finder` | [0.1.4](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame |
| `cpd-reporter` | [0.1.4](https://crates.io/crates/cpd-reporter) | Output formatting (13 reporters) |
| `jscpd` | [5.0.4](https://crates.io/crates/jscpd) | CLI binary and entry point |

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

## Building

Requires Rust 1.87+ (see `rust-toolchain.toml`).

```bash
cargo build --release
cargo test
```

## Documentation

- **[docs/rust.md](../docs/rust.md)** — Full CLI reference, all options, reporters, config file, differences from v4
- **[docs/typescript.md](../docs/typescript.md)** — TypeScript/Node.js engine (v4.x) documentation
- **[docs/ai-ready.md](../docs/ai-ready.md)** — AI reporter, agent skills, MCP server
- **[docs/api.md](../docs/api.md)** — Programming APIs (TypeScript and Rust)

## Known Differences from jscpd v4

| Feature | jscpd v4 (Node.js) | cpd v5 (Rust) |
|---------|--------------------|-----------------|
| `--store` (LevelDB) | Persistent store for large repos | Not supported |
| Programming API | `jscpd()` Promise, `detectClones()` | Rust crate API; no Node.js API |
| `--reporters` | All v4 reporters | All except `full` (use `console-full`) |

See [docs/rust.md](../docs/rust.md) for the full differences table.


## License

MIT