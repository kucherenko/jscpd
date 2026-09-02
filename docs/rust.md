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

The npm packages ship prebuilt binaries for 8 platforms — no Node.js runtime is required, the binary is self-contained:

| Platform | npm package | Rust target |
|----------|-------------|-------------|
| macOS arm64 | `jscpd-darwin-arm64` | `aarch64-apple-darwin` |
| macOS x64 | `jscpd-darwin-x64` | `x86_64-apple-darwin` |
| Linux arm64 (glibc) | `jscpd-linux-arm64-gnu` | `aarch64-unknown-linux-gnu` |
| Linux arm64 (musl) | `jscpd-linux-arm64-musl` | `aarch64-unknown-linux-musl` |
| Linux x64 (glibc) | `jscpd-linux-x64-gnu` | `x86_64-unknown-linux-gnu` |
| Linux x64 (musl) | `jscpd-linux-x64-musl` | `x86_64-unknown-linux-musl` |
| Windows arm64 | `jscpd-windows-arm64-msvc` | `aarch64-pc-windows-msvc` |
| Windows x64 | `jscpd-windows-x64-msvc` | `x86_64-pc-windows-msvc` |

The same binaries are attached to every [GitHub Release](https://github.com/kucherenko/jscpd/releases) as `jscpd-<platform>.tar.gz` with a `checksums.txt` and SLSA provenance, and packaged as a multi-arch Docker image at `ghcr.io/kucherenko/jscpd` (see [CI docs](ci-and-hooks.md#docker)).

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
| `--skip-isolated` | | Skip clones between different folders of the same isolation group: `,`-separated groups of `\|`-separated folders (e.g. `packages/a\|packages/b`). Useful in monorepos where teams own separate packages | — |
| `--baseline` | | Clone baseline file (e.g. `.jscpd-baseline.json`): clones whose fingerprint is absent from it are reported as new. See [Baseline](#baseline) | — |
| `--update-baseline` | | Rewrite the baseline file from the current run, creating it if missing (requires `--baseline`) | off |
| `--fail-on-new-clones` | | Exit 1 when more than N new clones are found (`--fail-on-new-clones` alone means N=0; requires `--baseline` or `--baseline-from-ref`) | — |
| `--baseline-from-ref` | | Compare against an ephemeral baseline built from a git ref's tree (e.g. `origin/main`). Conflicts with `--baseline` | — |
| `--sarif-error-tokens` | | Report SARIF results as `error` for clones with at least this many tokens (smaller clones stay `warning`). When overall duplication exceeds `--threshold`, all SARIF results become `error` regardless of size. | — (all `warning`) |
| `--min-duplicated-lines` | | Minimum percentage of duplication to report (0-100) | 0 |
| `--mcp` | | Serve the [Model Context Protocol over stdio](ai-ready.md#stdio-transport-rust-v5): scan PATHs once, then expose `check_duplication` / `get_statistics` / `check_current_directory` tools to MCP clients | off |
| `--summary` | | Print a codebase summary: top files and folders by tokens, lines, size, and a complexity estimate. See [Summary](#summary) | off |
| `--summary-top` | | Number of entries in each summary top list | 10 |
| `--summary-by` | | Summary sort metric: `tokens`, `lines`, `size`, `complexity` | `tokens` |
| `--silent` | `-s` | Suppress console output | off |
| `--no-tips` | | Suppress tips and promotional messages | off |
| `--version` | `-V` | Print version | — |
| `--help` | `-h` | Print help | — |

### Reporters

15 built-in reporters:

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
| `codeclimate` (alias `gitlab`) | `report/gl-code-quality-report.json` — CodeClimate issue format, ready for GitLab `artifacts:reports:codequality` |
| `openmetrics` | `report/jscpd-metrics.txt` — OpenMetrics text format, ready for GitLab `artifacts:reports:metrics` |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication percentage exceeds `--threshold` |
| `silent` | No console output |

Output file names differ from v4: v5 uses `jscpd-report.*` prefix (e.g. `jscpd-report.json`, `jscpd-report.sarif`) while v4 uses `jscpd-report.json`, `html/` directory, etc.

### Summary

`--summary` appends a codebase summary to the run output — the statistics jscpd already collects while scanning, aggregated to answer "where should I refactor first":

```
Summary (by tokens; 321 files, 129 folders analyzed)
Top files:
  TOKENS  LINES   SIZE  CX  DUP%  PATH
    2052    363  11.4K  80   0.0  files.ts
    ...
Top folders:
  FILES  TOKENS  LINES   SIZE  CX  PATH
      8    5264    843  26.5K  15  src/core
      ...
```

- **Top files** lists the top `--summary-top` files ranked by the `--summary-by` metric. Every row carries all metrics, so re-ranking by another lens is a `--summary-by size` (or `lines`, `complexity`) away.
- **Top folders** aggregates files into their direct parent directory (each file counted exactly once; no cumulative ancestor totals).
- **CX** is a language-agnostic cyclomatic-complexity estimate computed from the token stream: 1 + the number of decision-point tokens (`if`, `elif`/`elsif`/`elseif`, `unless`, `for`, `foreach`, `while`, `until`, `case`, `cond`, `when`, `catch`, `rescue`, `except`, `and`, `or`, `andalso`, `orelse`, `&&`, `||`, `?`, `??`). Matching is case-insensitive, so uppercase-keyword languages (SQL, PL/SQL, Fortran, COBOL, BASIC) count too. For folders it is the per-file mean. Languages that branch without such keywords (Smalltalk `ifTrue:` messages, Prolog clauses) stay at 1 — treat CX as a ranking signal, not an exact metric.
- **DUP%** is the share of the file's lines covered by detected clone fragments (both fragments of a clone count toward their files; display is capped at 100%).

The summary is fully opt-in and computed after detection from data already in memory, so runs without `--summary` are unaffected. It integrates with:

- `console` / `console-full` — the block shown above
- `ai` — a compact, LLM-token-efficient variant (one line per file/folder)
- `json` — an additive `summary` key in `jscpd-report.json` (absent when the flag is off, so the schema is unchanged for existing consumers)

Config file equivalents: `"summary": true`, `"summaryTop": 10`, `"summaryBy": "tokens"`.

```bash
# Refactoring hotspots: biggest files by tokens plus duplication share
cpd ./src --summary

# Agent-friendly: compact clone list + compact summary
cpd ./src --summary --reporters ai --no-tips

# Focus on the most complex files, top 5 lists, machine-readable
cpd ./src --summary --summary-by complexity --summary-top 5 --reporters json
```

### Baseline

Gate CI on *new* duplication only, tolerating clones that already exist. A baseline file records a content-hash fingerprint per accepted clone (the same hash the SARIF reporter emits as `partialFingerprints["jscpdCloneHash/v1"]`); clones absent from it are reported as new.

```bash
# Create or refresh the committed baseline
jscpd --baseline .jscpd-baseline.json --update-baseline .

# Fail when new clones appear (independent of --threshold)
jscpd --baseline .jscpd-baseline.json --fail-on-new-clones .

# Allow up to 3 new clones
jscpd --baseline .jscpd-baseline.json --fail-on-new-clones 3 .

# Stateless variant for PR gates: compare against a git ref instead of a file
jscpd --baseline-from-ref origin/main --fail-on-new-clones .
```

`--baseline-from-ref` checks the base ref's tree out into a temporary detached worktree, scans it with the same configuration, and compares fingerprints in memory. It costs a second scan of the corpus; the committed file needs only one. In CI, fetch the ref first (`fetch-depth: 0` or `git fetch origin main`).

New-clone information flows through the reporters: `[NEW]` markers in `console`/`console-full`, per-clone `isNew` plus `newClones` / `newDuplicatedLines` statistics in `json`, level `error` in `sarif`, severity `major` in `codeclimate`, and `jscpd_new_clones` / `jscpd_new_duplicated_lines` gauges in `openmetrics`.

Config file keys: `baseline`, `baselineFromRef`, `failOnNewClones`.

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

# Monorepo: don't compare team-owned packages with each other
cpd . --skip-isolated "packages/team-a|packages/team-b"
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

Isolation groups use the nested-array form in the config file: `"skipIsolated": [["packages/a", "packages/b"]]`.

Config discovery order: `--config <path>` → `.jscpd.json` → `.config/jscpd.json` (the [dot-config convention](https://dot-config.github.io/), also accepts `.config/.jscpd.json`) → the `jscpd` key in `package.json`.

## Format Support

v5 supports **224 formats** (verified via `--list`). Use `cpd --list` to see the full list.

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
 ├── cpd-tokenizer — Language tokenization (224 formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (15 reporters)
```
