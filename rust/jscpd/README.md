# jscpd — Copy/Paste Detector

[![npm version](https://img.shields.io/npm/v/jscpd.svg)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd.svg)](https://www.npmjs.com/package/jscpd)
[![license](https://img.shields.io/npm/l/jscpd.svg)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/jscpd.svg)](https://crates.io/crates/jscpd)
[![homepage](https://img.shields.io/badge/homepage-jscpd.dev-blue.svg)](https://jscpd.dev)

> **jscpd v5** is the Rust engine — a self-contained binary, no Node.js runtime. The TypeScript engine (v4: Node.js API, LevelDB/Redis stores) is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published as `jscpd@4`.

Copy/paste detector for programming source code. Supports **224 language formats**, **15 output reporters**, and per-line author attribution via git blame. Prebuilt binaries for 8 platforms — no Node.js runtime required.

## Packages

| Package | Installs | Use it when |
|---------|----------|-------------|
| [`jscpd`](https://www.npmjs.com/package/jscpd) | `jscpd` | Default install; the `jscpd` command |
| [`cpd`](https://www.npmjs.com/package/cpd) | `cpd` | Shorter command name only |
| [`jscpd` (crates.io)](https://crates.io/crates/jscpd) | `jscpd` + `cpd` | Rust-native install; both binaries |

The npm `jscpd` package installs a single `jscpd` command that runs the same Rust binary as `cpd`. For the shorter `cpd` alias on npm, install the separate [`cpd`](https://www.npmjs.com/package/cpd) package.

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

Prebuilt binaries for: macOS arm64/x64, Linux arm64/x64 (glibc + musl), Windows arm64/x64.

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

Full options: [docs/rust.md](https://github.com/kucherenko/jscpd/blob/master/docs/rust.md).

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

Also: `xml`, `csv`, `markdown`, `codeclimate`, `openmetrics`, `xcode`, `threshold`, `silent` (15 total).

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

224 formats including: JavaScript, TypeScript, Python, Go, Rust, Java, C/C++, C#, Ruby, PHP, Swift, Kotlin, Scala, Vue SFC, Svelte, Astro, Markdown, SQL, HTML, CSS, Bash, Dart, Lua, R, Haskell, Clojure, Elixir, Apex, CFML, and 200+ more.

Run `jscpd --list` for the full list, or see [FORMATS.md](https://github.com/kucherenko/jscpd/blob/master/FORMATS.md).

**Cross-format detection:** Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block, enabling duplicate detection across file types.

## CI

GitHub Action — installs the Rust engine, runs detection, uploads SARIF to GitHub Code Scanning:

```yaml
- uses: kucherenko/jscpd@v5
  with:
    threshold: 5
```

Pre-commit hook (Husky):

```bash
echo 'npx jscpd --threshold 5 --reporters console,silent .' > .husky/pre-commit
```

Full CI/pre-commit guide: [docs/ci-and-hooks.md](https://github.com/kucherenko/jscpd/blob/master/docs/ci-and-hooks.md).

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

## Coming from jscpd v4

Flags, config file and reporters are the same. The differences:

| Feature | jscpd v4 (Node.js) | jscpd v5 (Rust) |
|---------|--------------------|-----------------|
| `--store` (LevelDB/Redis) | Persistent store for large repos | Not supported (flag ignored with a warning) |
| Programming API | `jscpd()` Promise, `detectClones()` | Rust crate API; no Node.js API |
| `--reporters` | All v4 reporters | All except `full` (use `console-full`) |
| Output filenames | `jscpd-report.json`, `html/` dir | `jscpd-report.*` prefix |

Full table: [docs/rust.md](https://github.com/kucherenko/jscpd/blob/master/docs/rust.md#migrating-from-jscpd-v4). jscpd v4 lives on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch.

## Architecture

```
jscpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (224 formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (15 reporters)
```

## Links

- [Homepage](https://jscpd.dev)
- [Documentation](https://github.com/kucherenko/jscpd/blob/master/docs/rust.md)
- [FORMATS.md — all 224 formats](https://github.com/kucherenko/jscpd/blob/master/FORMATS.md)
- [Benchmark vs other tools](https://github.com/kucherenko/jscpd/blob/master/benchmark/BENCHMARK.md)
- [AI reporter, MCP server](https://github.com/kucherenko/jscpd/blob/master/docs/ai-ready.md)
- [GitHub](https://github.com/kucherenko/jscpd)
- [Changelog](https://github.com/kucherenko/jscpd/blob/master/rust/CHANGELOG.md)

## License

MIT