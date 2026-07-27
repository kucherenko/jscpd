---
name: jscpd
description: Copy-paste detector for 220+ languages. Detect duplicated code and measure duplication percentages.
---

# jscpd

Copy-paste detector for programming source code, supports 150+ languages. Use this skill to run jscpd and understand its output.

## Quick Start

```bash
# Run with ai reporter (compact output optimized for agents)
npx jscpd --reporters ai <path>

# With ignore patterns
npx jscpd --reporters ai --ignore "**/node_modules/**,**/dist/**" <path>

# Scope to specific formats
npx jscpd --reporters ai --format "javascript,typescript" <path>
```

## AI Reporter Output Format

The `ai` reporter produces compact, token-efficient output designed for agent consumption:

```
Clones:
src/ foo.ts:10-25 ~ bar.ts:42-57
src/utils/helpers.ts:100-120 ~ src/utils/other.ts:5-25
---
3 clones · 4.2% duplication
```

Each line represents one clone pair:
- **Same file**: `path/file.ts 10-25 ~ 45-60` (shared path shown once)
- **Same directory**: `shared/prefix/ file-a.ts:10-25 ~ file-b.ts:42-57` (common prefix factored out)
- **Different paths**: `path/a.ts:10-25 ~ path/b.ts:42-57`

## Options

| Option | Description |
|--------|-------------|
| `--reporters ai` | Use the AI-optimized reporter (compact clone list for agents) |
| `--reporters html` | Generate HTML report |
| `--reporters json` | Output JSON report |
| `--min-tokens N` | Minimum tokens to consider a duplication (default: 50) |
| `--min-lines N` | Minimum lines to consider a duplication (default: 5) |
| `--min-similarity N` | Minimum similarity percentage (default: 100, range: 1-100) |
| `--threshold N` | Exit with error if duplication % exceeds N |
| `--ignore "glob"` | Ignore patterns (comma-separated) |
| `--format "list"` | Limit to specific languages (e.g. `typescript,javascript`) |
| `--cross-formats "groups"` | Detect clones across related formats (e.g. `javascript,typescript` or the `js-ts` preset) |
| `--pattern "glob"` | Glob pattern to select files |
| `--gitignore` | Respect .gitignore |
| `--output "path"` | Directory to write reports to |
| `--silent` | Suppress output (useful with `--output` only) |
| `--list-output` | Print clone list to stdout (alternative to ai reporter) |
| `--store-path "path"` | Directory for LevelDB cache |
| `--no-tips` | Disable tips in output (enabled by default in CI) |
| `--config "path"` | Path to .jscpd.json config file |

## Cross-Format Clone Detection

By default each format is compared only against itself. `--cross-formats` defines groups of related formats that share one comparison pool, so a block duplicated between a `.js` and a `.ts` file is reported as a clone:

```bash
# One group: compare JavaScript and TypeScript files together
npx jscpd --reporters ai --cross-formats "javascript,typescript" <path>

# Preset covering javascript, jsx, typescript, tsx
npx jscpd --reporters ai --cross-formats "js-ts" <path>

# Multiple groups are separated by ";"
npx jscpd --reporters ai --cross-formats "javascript,typescript;css,scss" <path>
```

Notes:
- When a group mixes TypeScript with JavaScript, TS files are compared with erasable type syntax stripped, so `function f(a: number): void` matches `function f(a)`. Reported positions still reference the original source.
- Groups need at least two formats; groups sharing a format are merged into one pool.
- In per-format statistics, a cross-format clone is attributed to one member format of the group.
- In config files the key is `crossFormats` (or `cross-formats`) and accepts a string (`"javascript,typescript;css,scss"`), an array of strings (`["javascript,typescript", "css,scss"]`), or an array of arrays (`[["javascript","typescript"],["css","scss"]]`).

## Configuration File

Create a `.jscpd.json` in your project root:

```json
{
  "threshold": 0,
  "reporters": ["ai"],
  "ignore": ["**/node_modules/**", "**/dist/**", "**/*.min.*"],
  "format": ["typescript", "javascript"],
  "minLines": 5,
  "minTokens": 50,
  "output": "./reports/jscpd"
}
```

## Refactoring Duplicated Code

Once you've detected clones, use the **dry-refactoring** skill for a guided workflow to eliminate them:

→ **dry-refactoring** — step-by-step refactoring strategies and workflow for removing duplication. Install with:
  ```bash
  npx skills add https://github.com/kucherenko/jscpd --skill dry-refactoring
  ```
