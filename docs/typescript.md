# jscpd v4 (TypeScript / Node.js)

Copy/paste detector for programming source code. The TypeScript engine runs on Node.js and is published as [`jscpd`](https://www.npmjs.com/package/jscpd) on npm.

## Installation

```bash
# npm
npm install -g jscpd

# npx (no install required)
npx jscpd /path/to/code
```

## CLI Usage

```bash
jscpd [options] <path ...>
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--min-lines` | `-l` | Minimum lines in a clone | 5 |
| `--min-tokens` | `-k` | Minimum tokens in a clone | 50 |
| `--max-lines` | `-x` | Maximum source file lines | 1000 |
| `--max-size` | `-z` | Maximum source file size (e.g. `1kb`, `1mb`) | `100kb` |
| `--threshold` | `-t` | Duplication percentage threshold (exit with error if exceeded) | — |
| `--config` | `-c` | Path to config file | `.jscpd.json` in path |
| `--ignore` | `-i` | Glob patterns to exclude | — |
| `--ignore-pattern` | | Regex patterns to ignore code blocks | — |
| `--reporters` | `-r` | Reporters (comma-separated) | `time,console` |
| `--output` | `-o` | Output directory for file reporters | `./report/` |
| `--mode` | `-m` | Detection mode: `strict`, `mild`, `weak` | `mild` |
| `--format` | `-f` | Formats to check (comma-separated) | all detected |
| `--pattern` | `-p` | Glob pattern for file search | — |
| `--blame` | `-b` | Enrich clones with git blame author data | off |
| `--silent` | `-s` | Suppress console output | off |
| `--store` | | Custom store (e.g. `leveldb` for large repos) | memory |
| `--store-path` | | Directory for store cache (parallel runs) | — |
| `--absolute` | `-a` | Use absolute paths in reports | off |
| `--noSymlinks` | `-n` | Don't follow symlinks | off |
| `--ignoreCase` | | Ignore case of symbols (experimental) | off |
| `--gitignore` | | Respect `.gitignore` files (default: enabled) | on |
| `--no-gitignore` | | Don't respect `.gitignore` files | — |
| `--formats-exts` | | Custom format-to-extension mapping (e.g. `javascript:es,es6;dart:dt`) | — |
| `--formats-names` | | Custom format-to-filename mapping (e.g. `makefile:Makefile;docker:Dockerfile`) | — |
| `--skipLocal` | | Skip clones within the same directory | off |
| `--skipComments` | | Alias for `--mode weak` (ignore comments) | off |
| `--noTips` | | Suppress tips and promotional messages | off |
| `--exitCode` | | Exit code when clones detected | — |
| `--debug` | `-d` | Show debug info, don't run detection | off |
| `--verbose` | `-v` | Show full info during detection | off |
| `--list` | | List all supported formats and exit | — |
| `--version` | `-V` | Print version | — |
| `--help` | `-h` | Print help | — |

### Reporters

| Reporter | Output |
|----------|--------|
| `console` | Clone list with per-format statistics table |
| `consoleFull` | Full source snippets for each clone |
| `json` | `report/jscpd-report.json` |
| `xml` | `report/jscpd-report.xml` |
| `csv` | `report/jscpd-report.csv` |
| `html` | Interactive HTML report (via `@jscpd/html-reporter`) |
| `markdown` | `report/jscpd-report.md` |
| `badge` | SVG badges (via `@jscpd/badge-reporter`) |
| `sarif` | SARIF output for GitHub Code Scanning (via `jscpd-sarif-reporter`) |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication exceeds `--threshold` |
| `silent` | No console output |

You can also install third-party reporters as npm packages (e.g. `jscpd-full-reporter`).

### Config File

Create `.jscpd.json` in the target directory:

```json
{
  "path": ["./src"],
  "reporters": ["console", "json"],
  "minLines": 5,
  "minTokens": 50,
  "maxLines": 1000,
  "maxSize": "100kb",
  "threshold": 0,
  "format": ["javascript", "typescript"],
  "ignore": ["**/node_modules/**"],
  "gitignore": true,
  "mode": "mild",
  "absolute": false,
  "skipLocal": false,
  "skipComments": false
}
```

### Detection Modes

| Mode | Behavior |
|------|----------|
| `strict` | All tokens must match (including whitespace, newlines) |
| `mild` | Ignore empty and newline tokens |
| `weak` | Ignore comments, empty tokens, and newlines (`--skipComments` is an alias) |

### Examples

```bash
# Scan current directory
jscpd .

# Scan specific paths with options
jscpd --min-lines 10 --min-tokens 100 --reporters console,json,html ./src

# Scan only TypeScript files
jscpd --format typescript --pattern "**/*.ts" ./src

# Ignore directories
jscpd --ignore "**/dist/**,**/node_modules/**" .

# Skip clones within the same folder
jscpd --skipLocal .

# Use LevelDB store for large repos
jscpd --store leveldb /path/to/large/repo

# Configure LevelDB cache directory for parallel runs
jscpd --store leveldb --store-path /tmp/jscpd-cache /path/to/repo
```

## Programming API

### `jscpd` Promise API

```typescript
import { IClone } from '@jscpd/core';
import { jscpd } from 'jscpd';

const clones: IClone[] = await jscpd([]);
```

### `jscpd` with argv

```typescript
import { IClone } from '@jscpd/core';
import { jscpd } from 'jscpd';

const clones: IClone[] = await jscpd(['', '', './fixtures', '-m', 'weak', '--silent']);
```

### `detectClones` API

```typescript
import { detectClones } from 'jscpd';

const clones = await detectClones({
  path: ['./src'],
  silent: true,
  format: ['javascript', 'typescript'],
  minLines: 5,
  minTokens: 50,
  mode: 'mild',
});
```

### `detectClones` with custom store

```typescript
import { detectClones } from 'jscpd';
import { IMapFrame, MemoryStore } from '@jscpd/core';

const store = new MemoryStore<IMapFrame>();

await detectClones({
  path: ['./src'],
}, store);

// Re-use the store for incremental detection
await detectClones({
  path: ['./src'],
  silent: true,
}, store);
```

### Building custom tools

For deep customization, compose the lower-level packages:

- `@jscpd/core` — Core detection algorithm, event emitter interface
- `@jscpd/tokenizer` — Source code tokenization (224+ formats via reprism)
- `@jscpd/finder` — File walking, clone detection, built-in reporters
- `@jscpd/leveldb-store` — LevelDB persistent store for large repos
- `@jscpd/redis-store` — Redis store for distributed/CI environments

## Format Support

v4 supports **224 formats** (verified via `--list`). Use `jscpd --list` to see the full list.

### Cross-Format Detection

Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown (`.md`) files are tokenized per-block/per-section, enabling duplicate detection across file types (e.g., a `<script>` block in a `.vue` file matching a `.ts` file).

### Shebang Detection

Extensionless executable scripts are auto-detected by their shebang line (supports bash, python, node, ruby, perl, php, lua, tcl, R, groovy, swift, kotlin).

### Custom Format Mapping

```bash
# Map extensions to formats
jscpd --formats-exts "javascript:es,es6;dart:dt" ./src

# Map specific filenames to formats
jscpd --formats-names "makefile:Makefile,GNUmakefile;docker:Dockerfile" ./src
```

## Architecture

```
jscpd (CLI + API)
 ├── @jscpd/core        — Detection algorithm (Rabin-Karp), event system
 ├── @jscpd/tokenizer   — Source code tokenization (224+ formats via reprism)
 ├── @jscpd/finder      — File walking, orchestration, built-in reporters
 ├── @jscpd/html-reporter      — Interactive HTML report
 ├── @jscpd/badge-reporter     — SVG badge generation
 ├── @jscpd/sarif-reporter     — SARIF for GitHub Code Scanning
 ├── @jscpd/leveldb-store      — LevelDB persistent store
 └── @jscpd/redis-store        — Redis distributed store
```