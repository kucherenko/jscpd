# jscpd — Copy/Paste Detector

> **jscpd v5.x** is the Rust-based implementation. For the TypeScript/Node.js version, see [jscpd v4.x](https://www.npmjs.com/package/jscpd/v/4).

Fast copy/paste detector for programming source code. This package wraps the [cpd](https://www.npmjs.com/package/cpd) CLI — a Rust rewrite of the original jscpd with 10–30× faster detection, git blame, and 225+ language formats.

## Why Two Packages?

| Package | Version | Engine | When to use |
|---------|---------|--------|-------------|
| `jscpd` | 4.x | TypeScript / Node.js | Need the Node.js API, LevelDB/Redis stores, or programmatic usage |
| `jscpd` | 5.x | Rust (this package) | Want maximum speed, CLI-only usage, or git blame |
| `cpd` | 5.x | Rust | Same binary as jscpd 5.x, just with the `cpd` command name |

Both `jscpd` v5 and `cpd` v5 install the same Rust binary. The only difference is the command name (`jscpd` vs `cpd`).

## Performance

| Codebase | Files | jscpd v4 (Node.js) | jscpd v5 (Rust) | Speedup |
|----------|-------|--------------------|-----------------|---------|
| Multi-format fixtures | 353 | 1.59 s | 0.45 s | 3.5× |
| Rust sources (homogeneous) | 46 | 0.87 s | 0.03 s | 29× |

## Install

### npm (recommended)

```bash
npm install -g jscpd
```

Prebuilt binaries for 6 platforms — no Node.js runtime required:

| Package | OS | Arch | libc |
|---------|----|------|------|
| `cpd-darwin-arm64` | macOS | arm64 | — |
| `cpd-darwin-x64` | macOS | x64 | — |
| `cpd-linux-arm64-gnu` | Linux | arm64 | glibc |
| `cpd-linux-x64-gnu` | Linux | x64 | glibc |
| `cpd-linux-x64-musl` | Linux | x64 | musl |
| `cpd-windows-x64-msvc` | Windows | x64 | — |

### crates.io

```bash
cargo install jscpd
```

### From source

```bash
git clone https://github.com/kucherenko/jscpd.git
cd jscpd/rust
cargo build --release
# binary at target/release/jscpd (or target/release/cpd)
```

## Quick Start

```bash
# Scan current directory (defaults: min-tokens 50, min-lines 5)
jscpd .

# Scan specific paths
jscpd ./src ./lib

# Git blame with side-by-side author comparison
jscpd . --blame --reporters console-full

# Output to JSON + HTML
jscpd . --reporters json,html

# Fail CI if duplication exceeds threshold
jscpd . --threshold 10

# List all 225+ supported formats
jscpd --list
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
| `--reporters` | `-r` | console | comma-separated reporters |
| `--output` | `-o` | report | Output directory for file reporters |
| `--config` | `-c` | — | Path to config file (`.jscpd.json`) |
| `--exit-code` | — | `1` | Exit with code if duplicates found |
| `--threshold` | `-t` | — | Max duplication % before exit 1 |
| `--blame` | `-b` | — | Enrich clones with git blame data |
| `--no-gitignore` | — | — | Ignore `.gitignore` files |
| `--follow-symlinks` | — | — | Follow symbolic links |
| `--max-size` | `-z` | 512 KB | Skip files larger than N bytes |
| `--workers` | — | auto | Number of worker threads |
| `--no-colors` | — | — | Disable ANSI color output |
| `--skip-local` | — | — | Skip clones within the same directory |
| `--silent` | `-s` | — | Suppress console output |
| `--no-tips` | — | — | Suppress tips and promotional messages |
| `--list` | — | — | List all supported formats and exit |

## Reporters

13 built-in reporters:

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Source snippets + optional blame comparison |
| `json` | `report/cpd.json` |
| `xml` | `report/cpd.xml` |
| `csv` | `report/cpd.csv` |
| `html` | `report/cpd.html` |
| `markdown` | `report/cpd.md` |
| `badge` | `report/cpd-badge.svg` |
| `sarif` | `report/cpd.sarif.json` (GitHub Code Scanning) |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication % exceeds `--threshold` |
| `silent` | No console output |

Combine reporters: `--reporters console,json,html`

## Git Blame

```bash
jscpd . --blame --reporters console-full
```

Produces a side-by-side author comparison:

```
176 │ Andrii Kucherenko │ <= │ 196 │ Josh Soref │ ## TODO
177 │ Andrii Kucherenko │ <= │ 197 │ Josh Soref │
180 │ Andrii Kucherenko │ == │ 200 │ Andrii Kucherenko │ ## License
```

`==` = same author (original). `<=` = different author (potential copy).

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
  "blame": false,
  "noGitignore": false,
  "noColors": false,
  "silent": false
}
```

## Cross-Format Detection

Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block, enabling duplicate detection across file types:

```
Clone found (javascript)
 - app.vue:javascript [10:1 - 35:2] (25 lines, 180 tokens)
   utils.js [40:1 - 65:2]

Clone found (yaml)
 - docker-compose.yml:yaml [7:1 - 25:33] (18 lines, 36 tokens)
   config.yml:yaml [7:1 - 25:37]
```

## Known Differences from jscpd v4

| Feature | jscpd v4 (Node.js) | jscpd v5 (Rust) |
|---------|--------------------|-------------------|
| `--blame` in `console-full` | Per-line side-by-side author comparison | Same — `==` / `<=` markers |
| `--store` (LevelDB) | Persistent store for large repos | Not supported. Use jscpd v4.x |
| `--formatsExts` | Custom format-to-extension mapping | Not supported. Use `--format` |
| Programming API | `jscpd()` Promise API, `detectClones()` | Rust crate API; no Node.js API |
| Config file | `.jscpd.json` with camelCase keys | Same |
| Cross-format detection | Vue, Svelte, Astro, Markdown | Same — per-block tokenization |
| Token counts | May differ slightly | May differ by 1–2%; clone detection matches |
| `--reporters` | All v4 reporters | All v4 reporters except `full` (use `console-full`) |

## Architecture

```
jscpd (binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (225+ formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (13 reporters)
```

## License

MIT