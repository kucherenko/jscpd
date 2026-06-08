# jscpd — Copy/Paste Detector

> **jscpd v5.x** is the Rust-based implementation. For the TypeScript/Node.js version, see [jscpd v4.x](https://github.com/kucherenko/jscpd).

Fast copy/paste detector for programming source code. 10–30× faster than the Node.js version. Supports 223 language formats, git blame, and 13 output reporters.

## Packages

| Package | Version | Installs | When to use |
|---------|---------|----------|-------------|
| `jscpd` | 4.x | `jscpd` | Need the Node.js API, LevelDB/Redis stores, or programmatic usage |
| `jscpd` | 5.x | `jscpd` and `cpd` | Maximum speed, CLI-only usage, or git blame |
| `cpd` | 5.x | `cpd` | Same binary as jscpd 5.x, shorter command name only |

Both `jscpd` v5 and `cpd` v5 install the same Rust binary. Installing `jscpd@5` gives you both `jscpd` and `cpd` commands.

## Performance

| Codebase | Files | jscpd v4 (Node.js) | jscpd v5 (Rust) | Speedup |
|----------|-------|--------------------|-----------------|---------|
| Multi-format fixtures | 353 | 1.59 s | 0.45 s | 3.5× |
| Rust sources (homogeneous) | 46 | 0.87 s | 0.03 s | 29× |

## Install

```bash
# npm — installs both jscpd and cpd commands
npm install -g jscpd

# or install just the cpd command
npm install -g cpd

# crates.io — installs both jscpd and cpd binaries
cargo install jscpd
```

Prebuilt binaries for 6 platforms — no Node.js runtime required.

## Quick Start

```bash
# Scan current directory
jscpd .
cpd .           # same command, shorter name

# Git blame with side-by-side author comparison
jscpd . --blame --reporters console-full

# Output to JSON + HTML
jscpd . --reporters json,html

# Fail CI if duplication exceeds threshold
jscpd . --threshold 10

# List all supported formats
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

## Known Differences from jscpd v4

| Feature | jscpd v4 (Node.js) | jscpd v5 (Rust) |
|---------|--------------------|-------------------|
| `--store` (LevelDB) | Persistent store for large repos | Not supported |
| Programming API | `jscpd()` Promise API, `detectClones()` | Rust crate API; no Node.js API |
| `--reporters` | All v4 reporters | All except `full` (use `console-full`) |

See [docs/rust.md](../../docs/rust.md) for the full differences table and detailed documentation.

## License

MIT