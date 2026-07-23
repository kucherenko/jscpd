# jscpd v5 (Rust Engine)

The Rust engine is a ground-up rewrite of jscpd. It is a drop-in replacement for the Node.js CLI — same algorithm, same reporters, same `.jscpd.json` config — but 24-37x faster.

The Rust engine is distributed as two npm packages:

| Package | Installs commands | Notes |
|---------|-------------------|-------|
| [`jscpd@5`](https://www.npmjs.com/package/jscpd) | `jscpd` | Same command name as v4; drop-in CLI replacement |
| [`cpd`](https://www.npmjs.com/package/cpd) | `cpd` | Lighter package, shorter command only |
| [`jscpd` (crates.io)](https://crates.io/crates/jscpd) | `jscpd` **and** `cpd` | Rust-native install; both binaries |

All three install the identical Rust binary and accept the same CLI options. Only the crates.io install exposes both command names from a single package.

## Performance

Benchmarks on macOS (Apple Silicon), 10 runs per target (3 for CopilotKit). v4 ran with `--no-gitignore -i "node_modules"` to ensure comparable file scanning. See [performance-comparison.md](performance-comparison.md) for full methodology.

| Codebase | Files | Size | `jscpd` v4 (Node.js) | `cpd`/`jscpd` v5 (Rust) | Speedup |
|----------|-------|------|----------------------|-------------------------|---------|
| Multi-format fixtures | 548 | 1.5 MB | 1.03s | 0.03s | **34.3x** |
| Svelte source | 9K | 38 MB | 15.80s | 0.43s | **36.9x** |
| CopilotKit | 17K | 159 MB | 82.89s | 3.44s | **24.1x** |

## Installation

```bash
# npm — installs the jscpd command (same binary as v4 command name)
npm install -g jscpd@5
jscpd /path/to/code

# npm — installs only the cpd command (lighter)
npm install -g cpd
cpd /path/to/code

# crates.io — Rust-native install (exposes both jscpd and cpd commands)
cargo install jscpd
jscpd /path/to/code
cpd /path/to/code

# Nix — run without installing
nix run github:kucherenko/jscpd -- /path/to/code

# Nix — install permanently
nix profile install github:kucherenko/jscpd

# Homebrew (macOS/Linux)
brew install jscpd
```

The npm packages ship prebuilt binaries for 6 platforms: macOS arm64/x64, Linux arm64/x64 (glibc/musl), Windows x64. No Node.js runtime is required — the binary is self-contained.

## CLI Usage

The `jscpd` command is available after installing `jscpd@5`; the `cpd` command is available after installing either `cpd` (npm) or `jscpd` (crates.io). Both commands accept the same options and are identical:

```bash
jscpd [OPTIONS] [PATH]...
cpd [OPTIONS] [PATH]...
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--min-tokens` | `-k` | Minimum tokens in a clone | 50 |
| `--min-lines` | `-l` | Minimum lines in a clone | 5 |
| `--max-lines` | `-x` | Maximum source file lines | — |
| `--max-size` | `-z` | Skip files larger than SIZE (e.g. `1kb`, `1mb`, `100kb`) | no limit |
| `--mode` | `-m` | Detection mode: `mild`, `weak`, `strict` | `mild` |
| `--workers` | | Number of worker threads for parallel tokenization/detection | auto (all CPU cores) |
| `--no-colors` | | Disable ANSI color output | off |
| `--absolute` | `-a` | Use absolute paths in reports | off |
| `--ignore-case` | | Ignore case of symbols in code (experimental) | off |
| `--formats-exts` | | Custom format-to-extension mapping (e.g. `javascript:es,es6;dart:dt`) | — |
| `--formats-names` | | Custom format-to-filename mapping | — |
| `--cross-formats` | | Detect clones across formats: `;`-separated groups of `,`-separated formats (e.g. `javascript,typescript`). Preset `js-ts` = `javascript,jsx,typescript,tsx` | — |
| `--list` | | List all supported formats and exit | — |
| `--skip-local` | | Skip clones where both fragments are in the same directory | off |
| `--min-duplicated-lines` | | Minimum percentage of duplication to report (0-100) | 0 |
| `--silent` | `-s` | Suppress console output | off |
| `--no-tips` | | Suppress tips and promotional messages | off |
| `--version` | `-V` | Print version | — |
| `--help` | `-h` | Print help | — |

### Reporters

13 built-in reporters:

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Clone list with source snippets; with `--blame` shows side-by-side author comparison |
| `json` | `report/jscpd-report.json` |
| `xml` | `report/jscpd-report.xml` |
| `csv` | `report/jscpd-report.csv` |
| `html` | `report/jscpd-report.html` |
| `markdown` | `report/jscpd-report.md` |
| `badge` | `report/jscpd-badge.svg` + `report/jscpd-lines-badge.svg` |
| `sarif` | `report/jscpd-report.sarif` (GitHub Code Scanning) |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication percentage exceeds `--threshold` |
| `silent` | No console output |

Output file names differ from v4: v5 uses `jscpd-report.*` prefix (e.g. `jscpd-report.json`, `jscpd-report.sarif`) while v4 uses `jscpd-report.json`, `html/` directory, etc.

### Blame Output

With `--blame --reporters console-full`, clones are displayed with a side-by-side author comparison:

```
176 │ Andrii Kucherenko │ <= │ 196 │ Josh Soref │ ## TODO
177 │ Andrii Kucherenko │ <= │ 197 │ Josh Soref │
180 │ Andrii Kucherenko │ == │ 200 │ Andrii Kucherenko │ ## License
```

`==` means both lines were written by the same author; `<=` means different authors (potential copy).

### Examples

```bash
# Drop-in replacement for jscpd v4
jscpd /path/to/source
# or
cpd /path/to/source

# Same flags as v4
cpd /path/to/source --min-tokens 30 --min-lines 3 --reporters console,json,html

# Git blame with side-by-side author comparison
cpd /path/to/source --blame --reporters console-full

# List supported formats
cpd --list

# Use multiple reporters with custom output
cpd ./src -r console,json,sarif -o ./reports

# Skip clones within the same directory
cpd --skip-local /path/to/source
```

### Config File

v5 reads the same `.jscpd.json` config file format as v4:

```json
{
  "path": ["./src"],
  "reporters": ["console", "json"],
  "minLines": 5,
  "minTokens": 50,
  "threshold": 0,
  "format": ["javascript", "typescript"],
  "ignore": ["**/node_modules/**"],
  "gitignore": true,
  "mode": "mild"
}
```

## Format Support

v5 supports **223 formats** (verified via `--list`). Use `cpd --list` to see the full list.

### Cross-Format Detection

Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown (`.md`) files are tokenized per-block/per-section, enabling duplicate detection across file types — same as v4.

### Cross-Format Groups (`--cross-formats`)

By default every format is compared in its own isolated pool, so a TypeScript file never matches a near-identical JavaScript file. `--cross-formats` declares format equivalence groups that share one comparison pool — useful for finding leftover `.js` copies during a TypeScript migration:

```bash
cpd --cross-formats "javascript,typescript" ./src
cpd --cross-formats js-ts ./src                      # preset: javascript,jsx,typescript,tsx
cpd --cross-formats "js-ts;css,scss" ./src           # multiple groups
```

When a group mixes TypeScript (`typescript`/`tsx`) with JavaScript (`javascript`/`jsx`), TypeScript files are compared with erasable type syntax stripped from the detection token stream — type annotations, generics, `interface`/`type` declarations, `as`/`satisfies`, `?`/`!` markers, access modifiers, `implements` clauses, type-only imports/exports, overload signatures, and `declare` statements. Reported clone positions always reference the original sources.

Config file equivalents (all three shapes are accepted):

```json
{ "crossFormats": "javascript,typescript;css,scss" }
{ "crossFormats": ["javascript,typescript", "css,scss"] }
{ "crossFormats": [["javascript", "typescript"], ["css", "scss"]] }
```

Notes:

- TypeScript syntax with runtime semantics is not erased and will not cross-match: `enum`, non-declare `namespace`, parameter properties (`constructor(private x)`), `import x = require()`, `export =`.
- A cross-format clone is attributed to one member format in the per-format statistics.
- Overlapping groups are merged; groups with fewer than two formats are ignored.

## Differences from jscpd v4 (Node.js)

| Feature | jscpd v4 (Node.js) | cpd v5 (Rust) |
|---------|--------------------|-----------------|
| `--blame` | Calls `git` CLI for each file | Same output (`==`/`<=` markers), calls `git blame --porcelain` per file |
| `--store` (LevelDB/Redis) | Persistent store for large repos | Not supported. Use jscpd v4.x for external stores. |
| `--formats-exts` | Custom format-to-extension mapping | Same flag name, same behavior |
| `--formats-names` | Custom format-to-filename mapping | Same flag name, same behavior |
| Programming API | `jscpd()` Promise API, `detectClones()` | Rust API via `cpd-finder` crate; no Node.js API |
| Config file | `.jscpd.json` with camelCase keys | Same — `.jscpd.json` with camelCase keys |
| Cross-format detection | Vue SFC, Svelte, Astro, Markdown | Same — per-block tokenization |
| Token counts | Varies by tokenizer | May differ by 1-2% due to Rust tokenizer; clone detection matches |
| `--reporters` | All v4 reporters | All v4 reporters except `full` (use `console-full`) |
| `--no-gitignore` | Default respects `.gitignore` | Same behavior, same flag name |
| `--workers` | Not available | Available — control parallelism for file tokenization/detection |
| Output filenames | `jscpd-report.json`, `html/` directory | `jscpd-report.json`, `jscpd-report.html`, `jscpd-report.sarif`, `jscpd-report.csv`, `jscpd-report.md`, `jscpd-badge.svg`, `jscpd-lines-badge.svg` |

## Rust API

For integration in Rust applications:

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