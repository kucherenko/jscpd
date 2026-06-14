# cpd — Copy/Paste Detector

Fast copy/paste detector for programming source code. Rust rewrite of [jscpd](https://github.com/kucherenko/jscpd), supports 223 language formats.

> **jscpd v5.x** is the Rust-based implementation. For the TypeScript/Node.js version, see [jscpd v4.x](https://www.npmjs.com/package/jscpd/v/4).

## Packages

| Package | Installs | When to use |
|---------|----------|-------------|
| `jscpd@5` | `jscpd` and `cpd` | Same binary, both command names available |
| `cpd` | `cpd` | Shorter command name only |

Both packages install the same Rust binary. Choose based on which command name you prefer.

## Install

```bash
# installs both jscpd and cpd commands
npm install -g jscpd

# installs only the cpd command
npm install -g cpd

# crates.io — installs both jscpd and cpd binaries
cargo install jscpd

# Nix — run without installing
nix run github:kucherenko/jscpd -- /path/to/code

# Nix — install permanently
nix profile install github:kucherenko/jscpd

# Homebrew (macOS/Linux)
brew install jscpd
```

Prebuilt binaries for 6 platforms — no Node.js runtime required.

## Quick Start

```bash
# Scan current directory
cpd .

# Scan specific paths
cpd ./src ./lib

# Minimum tokens/lines for a clone
cpd . --min-tokens 30 --min-lines 3

# Git blame with side-by-side author comparison
cpd . --blame --reporters console-full

# Output to JSON + HTML
cpd . --reporters json,html

# Fail CI if duplication exceeds threshold
cpd . --threshold 10

# List all supported formats
cpd --list
```

## Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--min-tokens` | `-k` | 50 | Minimum tokens to consider a duplicate |
| `--min-lines` | `-l` | 5 | Minimum lines to consider a duplicate |
| `--max-lines` | `-x` | — | Maximum lines per duplicate block |
| `--mode` | `-m` | mild | Detection mode: `mild`, `weak`, `strict` |
| `--skip-comments` | — | — | Alias for `--mode weak` |
| `--format` | `-f` | all | Comma-separated formats to check |
| `--ignore-pattern` | `-i` | — | Glob patterns to ignore |
| `--reporters` | `-r` | console | Comma-separated reporters |
| `--output` | `-o` | report | Output directory for file reporters |
| `--config` | `-c` | — | Path to config file (`.jscpd.json`) |
| `--threshold` | `-t` | — | Max duplication % before exit 1 |
| `--blame` | `-b` | — | Enrich clones with git blame data |
| `--skip-local` | — | — | Skip clones within the same directory |
| `--silent` | `-s` | — | Suppress console output |
| `--list` | — | — | List all supported formats and exit |

For the full options list, see [docs/rust.md](../../docs/rust.md).

## Reporters

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Source snippets + blame comparison |
| `json` | `report/jscpd-report.json` |
| `html` | `report/jscpd-report.html` |
| `sarif` | `report/jscpd-report.sarif` (GitHub Code Scanning) |
| `ai` | Token-efficient output for LLM pipelines |
| `badge` | `report/jscpd-badge.svg` + `report/jscpd-lines-badge.svg` |

Plus: `xml`, `csv`, `markdown`, `xcode`, `threshold`, `silent`.

## Config File

Create `.jscpd.json` in your project root:

```json
{
  "minTokens": 30,
  "minLines": 3,
  "format": ["javascript", "typescript", "python"],
  "ignorePattern": ["node_modules", "dist", "*.min.js"],
  "reporters": ["console", "json"],
  "output": "report",
  "threshold": 5,
  "blame": false
}
```

## Cross-Format Detection

Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block, enabling duplicate detection across file types.

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

## Architecture

```
cpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (223 formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (13 reporters)
```

See [docs/rust.md](../../docs/rust.md) for detailed documentation, the full differences table from jscpd v4, and the Rust API.


## License

MIT