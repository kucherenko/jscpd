# cpd / jscpd — Copy/Paste Detector

Fast copy/paste detector for programming source code. Rust rewrite of [jscpd](https://github.com/kucherenko/jscpd), supports 225+ formats.

> **jscpd v5.x** is the Rust-based implementation. For the TypeScript/Node.js version, see [jscpd v4.x](https://www.npmjs.com/package/jscpd/v/4).

## Install

```bash
npm install cpd
# or
npm install jscpd
```

The correct platform-specific binary is selected automatically:

| Package | OS | Arch | libc |
|---------|----|------|------|
| `cpd-darwin-arm64` | macOS | arm64 | — |
| `cpd-darwin-x64` | macOS | x64 | — |
| `cpd-linux-arm64-gnu` | Linux | arm64 | glibc |
| `cpd-linux-x64-gnu` | Linux | x64 | glibc |
| `cpd-linux-x64-musl` | Linux | x64 | musl |
| `cpd-windows-x64-msvc` | Windows | x64 | — |

## Usage

```bash
# Scan current directory
npx cpd .

# Scan specific paths
npx cpd ./src ./lib

# Minimum tokens/lines for a clone
npx cpd . --min-tokens 30 --min-lines 3

# Only check specific formats
npx cpd . --format rust,typescript,python

# Ignore patterns
npx cpd . --ignore-pattern "node_modules,dist"

# Verbose output with source snippets
npx cpd . --reporters console-full

# Output to JSON, HTML, SARIF, etc.
npx cpd . --reporters json,html,sarif

# Exit with error code if duplicates found
npx cpd . --exit-code

# Fail if duplication exceeds threshold
npx cpd . --threshold 10

# Git blame enrichment
npx cpd . --blame

# List all supported formats
npx cpd --list
```

## Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--min-tokens` | `-k` | 50 | Minimum tokens to consider a duplicate |
| `--min-lines` | `-l` | 5 | Minimum lines to consider a duplicate |
| `--max-lines` | `-x` | — | Maximum lines per duplicate block |
| `--mode` | `-m` | mild | Detection mode: `mild`, `weak`, `strict` |
| `--skip-comments` | — | — | Alias for `--mode weak` |
| `--format` | `-f` | all | Comma-separated list of formats to check |
| `--ignore-pattern` | `-i` | — | Glob patterns to ignore |
| `--reporters` | `-r` | console | Output reporters: `console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`, `sarif`, `ai`, `xcode`, `threshold`, `silent` |
| `--output` | `-o` | report | Output directory for file reporters |
| `--config` | `-c` | — | Path to config file (`.jscpd.json`) |
| `--exit-code` | — | `1` | Exit with code if duplicates found |
| `--threshold` | `-t` | — | Maximum duplication % before exit 1 |
| `--blame` | `-b` | — | Enrich clones with git blame data |
| `--no-gitignore` | — | — | Do not respect `.gitignore` files |
| `--follow-symlinks` | — | — | Follow symbolic links |
| `--max-size` | `-z` | 512KB | Skip files larger than N bytes |
| `--workers` | — | auto | Number of worker threads |
| `--no-colors` | — | — | Disable ANSI color output |
| `--skip-local` | — | — | Skip clones within the same directory |
| `--silent` | `-s` | — | Suppress console output |
| `--no-tips` | — | — | Suppress tips and promotional messages |
| `--list` | — | — | List all supported formats and exit |

## Output Example

```
Clone found (javascript)
 - src/auth.js [10:1 - 35:2] (25 lines, 180 tokens)
   src/helpers.js [40:1 - 65:2]

Clone found (rust)
 - src/main.rs [5:1 - 30:2] (25 lines, 145 tokens)
   src/lib.rs [100:1 - 125:2]

┌────────────┬────────────────┬─────────────┬──────────────┬──────────────┬──────────────────┬───────────────────┐
│ Format     │ Files analyzed │ Total lines │ Total tokens │ Clones found │ Duplicated lines  │ Duplicated tokens │
├────────────┼────────────────┼─────────────┼──────────────┼──────────────┼──────────────────┼───────────────────┤
│ javascript │ 12             │ 340         │ 2100         │ 3            │ 85 (25.00%)       │ 420 (20.00%)      │
├────────────┼────────────────┼─────────────┼──────────────┼──────────────┼──────────────────┼───────────────────┤
│ rust       │ 8              │ 450         │ 2800         │ 2            │ 50 (11.11%)       │ 290 (10.36%)      │
├────────────┼────────────────┼─────────────┼──────────────┼──────────────────┼───────────────────┤
│ Total:     │ 20             │ 790         │ 4900         │ 5            │ 135 (17.09%)      │ 710 (14.49%)      │
└────────────┴────────────────┴─────────────┴──────────────┴──────────────┴──────────────────┴───────────────────┘
Found 5 clones.
```

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

## Reporters

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Clone list with source snippets |
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

Multiple reporters can be combined: `--reporters console,json,html`

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
 ├── cpd-core      — Detection algorithm (Rabin-Karp hashing)
 ├── cpd-tokenizer — Language tokenization (225+ formats)
 ├── cpd-finder    — File walking + orchestration pipeline
 └── cpd-reporter  — Output formatting (13 reporters)
```

## License

MIT