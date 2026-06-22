# jscpd — Copy/Paste Detector

[![npm version](https://img.shields.io/npm/v/jscpd.svg)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd.svg)](https://www.npmjs.com/package/jscpd)
[![license](https://img.shields.io/npm/l/jscpd.svg)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/jscpd.svg)](https://crates.io/crates/jscpd)
[![homepage](https://img.shields.io/badge/homepage-jscpd.dev-blue.svg)](https://jscpd.dev)

> **jscpd v5.x** is the Rust-based engine — 24-37x faster than v4. For the TypeScript/Node.js version (programmatic API, LevelDB/Redis stores), see [jscpd v4.x](https://www.npmjs.com/package/jscpd/v/4).

Fast copy/paste detector for programming source code. Supports **223 language formats**, **13 output reporters**, and per-line author attribution via git blame. Prebuilt binaries for 6 platforms — no Node.js runtime required.

## Packages

| Package | Installs | Use it when |
|---------|----------|-------------|
| [`jscpd@5`](https://www.npmjs.com/package/jscpd) | `jscpd` | Same command name as v4; drop-in CLI replacement |
| [`cpd`](https://www.npmjs.com/package/cpd) | `cpd` | Shorter command name only |
| [`jscpd` (crates.io)](https://crates.io/crates/jscpd) | `jscpd` + `cpd` | Rust-native install; both binaries |

The npm `jscpd@5` package installs a single `jscpd` command that runs the same Rust binary as `cpd`. For the shorter `cpd` alias on npm, install the separate [`cpd`](https://www.npmjs.com/package/cpd) package.

## Performance

| Codebase | Files | Size | jscpd v4 (Node.js) | jscpd v5 (Rust) | Speedup |
|----------|-------|------|--------------------|-----------------|---------|
| Multi-format fixtures | 548 | 1.5 MB | 1.03 s | 0.03 s | **34.3×** |
| Svelte source | 9K | 38 MB | 15.80 s | 0.43 s | **36.9×** |
| CopilotKit | 17K | 159 MB | 82.89 s | 3.44 s | **24.1×** |

Methodology: [docs/performance-comparison.md](../../docs/performance-comparison.md).

## Install

```bash
# npm — installs the jscpd command
npm install -g jscpd

# crates.io — installs both jscpd and cpd binaries
cargo install jscpd

# Nix — run without installing
nix run github:kucherenko/jscpd -- /path/to/code

# Nix — install permanently
nix profile install github:kucherenko/jscpd

# Homebrew (macOS/Linux)
brew install jscpd
```

Prebuilt binaries for: macOS arm64/x64, Linux arm64/x64 (glibc + musl), Windows x64.

## Quick Start

```bash
jscpd .                                       # scan current directory
jscpd ./src ./lib                             # scan specific paths
jscpd . --min-tokens 30 --min-lines 3         # tune detection sensitivity
jscpd . --blame --reporters console-full      # git blame, side-by-side authors
jscpd . --reporters json,html                 # write report files
jscpd . --threshold 10                        # fail CI if >10% duplicated
jscpd --list                                  # list all supported formats
```

## Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--min-tokens` | `-k` | 50 | Minimum tokens in a clone |
| `--min-lines` | `-l` | 5 | Minimum lines in a clone |
| `--max-lines` | `-x` | — | Maximum lines per duplicate block |
| `--max-size` | `-z` | — | Skip files larger than SIZE (e.g. `1mb`) |
| `--mode` | `-m` | `mild` | Detection mode: `mild`, `weak`, `strict` |
| `--skip-comments` | — | — | Alias for `--mode weak` |
| `--format` | `-f` | all | Comma-separated formats to check |
| `--ignore-pattern` | `-i` | — | Glob patterns to ignore |
| `--reporters` | `-r` | `console` | Comma-separated reporters |
| `--output` | `-o` | `report` | Output directory for file reporters |
| `--config` | `-c` | — | Path to `.jscpd.json` config file |
| `--threshold` | `-t` | — | Max duplication % before exit 1 |
| `--blame` | `-b` | — | Enrich clones with git blame data |
| `--workers` | — | auto | Worker threads for parallel scan |
| `--skip-local` | — | — | Skip clones within the same directory |
| `--absolute` | `-a` | — | Use absolute paths in reports |
| `--silent` | `-s` | — | Suppress console output |
| `--list` | — | — | List all supported formats and exit |

Full options: [docs/rust.md](../../docs/rust.md).

## Reporters

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Source snippets; with `--blame` shows side-by-side author comparison |
| `json` | `report/jscpd-report.json` |
| `html` | `report/jscpd-report.html` |
| `sarif` | `report/jscpd-report.sarif` (GitHub Code Scanning) |
| `ai` | Token-efficient output for LLM pipelines |
| `badge` | `report/jscpd-badge.svg` + `report/jscpd-lines-badge.svg` |

Also: `xml`, `csv`, `markdown`, `xcode`, `threshold`, `silent` (13 total).

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

## Supported Formats

223 formats including: JavaScript, TypeScript, Python, Go, Rust, Java, C/C++, C#, Ruby, PHP, Swift, Kotlin, Scala, Vue SFC, Svelte, Astro, Markdown, SQL, HTML, CSS, Bash, Dart, Lua, R, Haskell, Clojure, Elixir, Apex, CFML, and 200+ more.

Run `jscpd --list` for the full list, or see [FORMATS.md](../../FORMATS.md).

**Cross-format detection:** Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block, enabling duplicate detection across file types.

## CI

GitHub Action — installs the Rust engine, runs detection, uploads SARIF to GitHub Code Scanning:

```yaml
- uses: kucherenko/jscpd@master
  with:
    threshold: 5
```

Pre-commit hook (Husky):

```bash
echo 'npx jscpd --threshold 5 --reporters console,silent .' > .husky/pre-commit
```

Full CI/pre-commit guide: [docs/ci-and-hooks.md](../../docs/ci-and-hooks.md).

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

## Differences from jscpd v4

| Feature | jscpd v4 (Node.js) | jscpd v5 (Rust) |
|---------|--------------------|-----------------|
| `--store` (LevelDB/Redis) | Persistent store for large repos | Not supported |
| Programming API | `jscpd()` Promise, `detectClones()` | Rust crate API; no Node.js API |
| `--reporters` | All v4 reporters | All except `full` (use `console-full`) |
| Output filenames | `jscpd-report.json`, `html/` dir | `jscpd-report.*` prefix |

Full differences: [docs/rust.md](../../docs/rust.md).

## Architecture

```
jscpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (223 formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (13 reporters)
```

## Links

- [Homepage](https://jscpd.dev)
- [Documentation](../../docs/rust.md)
- [FORMATS.md — all 223 formats](../../FORMATS.md)
- [Performance comparison](../../docs/performance-comparison.md)
- [AI reporter, MCP server](../../docs/ai-ready.md)
- [GitHub](https://github.com/kucherenko/jscpd)
- [Changelog](../../CHANGELOG.md)

## License

MIT