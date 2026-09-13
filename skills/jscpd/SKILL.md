---
name: jscpd
description: Copy-paste detector for 220+ languages. Detect exact, renamed and near-miss duplicated code, measure duplication percentages, and find refactoring hotspots with the codebase summary.
---

# jscpd

Copy-paste detector for programming source code, supports 220+ languages. Use this skill to run jscpd and understand its output.

## Quick Start

```bash
# Run with ai reporter (compact output optimized for agents)
npx jscpd --reporters ai <path>

# With ignore patterns
npx jscpd --reporters ai --ignore "**/node_modules/**,**/dist/**" <path>

# Scope to specific formats
npx jscpd --reporters ai --format "javascript,typescript" <path>

# Second pass, noisier: copies that only differ in names and values (Type-2, "renamed").
# Review each hit before acting on it.
npx jscpd --reporters ai --ignore-identifiers --min-tokens 70 <path>

# Third pass, noisier still: copies with a few edited lines or the same function
# structure (Type-3, "similar"). Keep the settings tight.
npx jscpd --reporters ai --max-gap-lines 1 --similarity 0.85 <path>

# Where to refactor first: clone list plus a hotspot summary
npx jscpd --reporters ai --summary <path>
```

## AI Reporter Output Format

The `ai` reporter produces compact, token-efficient output designed for agent consumption:

```
Clones:
src/ foo.ts:10-25 ~ bar.ts:42-57
src/utils/helpers.ts:100-120 ~ src/utils/other.ts:5-25
src/cart/ basket.js:1-9 ~ cart.js:1-9 (renamed)
src/api/ save-account.js:1-12 ~ save-user.js:1-11 [~0.91 gap]
src/billing/ credit-note.js:1-19 ~ invoice.js:1-17 [~0.75 ast]
---
5 clones · 4.2% duplication
```

Each line represents one clone pair:
- **Same file**: `path/file.ts 10-25 ~ 45-60` (shared path shown once)
- **Same directory**: `shared/prefix/ file-a.ts:10-25 ~ file-b.ts:42-57` (common prefix factored out)
- **Different paths**: `path/a.ts:10-25 ~ path/b.ts:42-57`

A suffix tells the **kind** of clone; no suffix means an exact copy:
- `(renamed)`: the two blocks differ only in identifier names, literal values or annotations (Type-2). Only appears with `--ignore-identifiers`, `--ignore-literals` or `--ignore-annotations`.
- `[~0.91 gap]`: two exact clones merged across up to `--max-gap-lines` unmatched lines (Type-3). The number is matched tokens over the merged span.
- `[~0.75 ast]`: two functions whose syntax-tree structure overlaps at least `--similarity` (Type-3). The number is the structural similarity, names and literal values do not count.

## Options

| Option | Description |
|--------|-------------|
| `--reporters ai` | Use the AI-optimized reporter (compact clone list for agents) |
| `--reporters html` | Generate HTML report |
| `--reporters json` | Output JSON report |
| `--min-tokens N` | Minimum tokens to consider a duplication (default: 50) |
| `--min-lines N` | Minimum lines to consider a duplication (default: 5) |
| `--threshold N` | Exit with error if duplication % exceeds N |
| `--ignore "glob"` | Ignore patterns (comma-separated) |
| `--format "list"` | Limit to specific languages (e.g. `typescript,javascript`) |
| `--cross-formats "groups"` | Detect clones across related formats (e.g. `javascript,typescript` or the `js-ts` preset) |
| `--ignore-identifiers` | Treat all identifiers as equal, so blocks that differ only in variable, function or type names match (Type-2, reported as `renamed`) |
| `--ignore-literals` | Treat all string literals as equal and all numeric literals as equal (Type-2) |
| `--ignore-annotations` | Drop `@Name` / `@Name(...)` annotations and decorators before matching, in languages where `@` means one (Type-2) |
| `--max-gap-lines N` | Merge clones of one file pair separated by at most N unmatched lines into one `similar` clone (Type-3, default: 0 = off) |
| `--similarity RATIO` | Report JavaScript/TypeScript function pairs whose syntax-tree similarity reaches RATIO, in `(0, 1]`, as `similar` clones (Type-3, default: 1 = exact only) |
| `--summary` | Append a codebase summary: top files/folders by tokens, lines, size, complexity, with duplication share |
| `--summary-top N` | Number of entries in each summary top list (default: 10) |
| `--summary-by metric` | Summary ranking metric: `tokens`, `lines`, `size`, `complexity` (default: `tokens`) |
| `--pattern "glob"` | Glob pattern to select files |
| `--no-gitignore` | Do not respect `.gitignore` (it is respected by default) |
| `--output "path"` | Directory to write reports to |
| `--silent` | Suppress console output (useful with file reporters and `--output`) |
| `--list` | List all supported formats and exit |
| `--no-tips` | Disable tips in output (skipped automatically when stdout is not a TTY or `CI` or `JSCPD_NO_TIPS` is set) |
| `--config "path"` | Path to .jscpd.json config file |

## Clone Kinds: Exact, Renamed, Similar

By default jscpd reports **exact** clones only: the token sequences are identical (whitespace, layout and, depending on the mode, comments do not count). Two opt-in families widen the net. Run them as separate passes after the default scan, because they find more and longer clones and change what a "clone" means.

**These passes are noisy by design.** An exact clone is almost always a real copy. A renamed or similar clone is a *candidate*: the flags deliberately ignore the very things (names, values, a statement or two) that often make two blocks different in meaning. Expect false positives from:

- boilerplate that is supposed to look alike: DTOs and models, config tables, enum-like maps, route or handler registrations, builders
- test files: `describe`/`it` blocks, fixtures and setup code repeat the same shape on purpose
- generated code, migrations, serializers, protocol bindings
- language idioms: two `reduce` loops or two `switch` statements that share structure but not logic
- small blocks: with `--ignore-identifiers` a 50-token block is mostly placeholders, so raise `--min-tokens`

Rules that keep the noise manageable:

- Run them **after** the exact clones are handled, one family at a time, so every hit is attributable to one flag.
- Start conservative: `--ignore-identifiers` alone (add `--ignore-literals` only when constants are the known problem), `--max-gap-lines 1` or `2`, `--similarity 0.85` or higher, and `--min-tokens 70` or more for the identifier pass. Widen only when the tight run comes back empty.
- Treat every `(renamed)` or `[~…]` line as a lead to read, not a defect to fix. Do not gate CI (`--threshold`, `--fail-on-new-clones`) on these passes unless the team has reviewed what they report on the codebase.
- Never claim "N duplicates found" from a normalized run without saying which flags produced them.

### Type-2: renamed clones (`--ignore-identifiers`, `--ignore-literals`, `--ignore-annotations`)

Copies where someone renamed the variables or changed the constants:

```bash
npx jscpd --reporters ai --ignore-identifiers <path>                      # function a(x) {…} matches function b(y) {…}
npx jscpd --reporters ai --ignore-identifiers --ignore-literals <path>    # …and 10 matches 25, 'dev' matches 'prod'
npx jscpd --reporters ai --ignore-annotations <path>                      # @Override / @Deprecated no longer split a clone
```

```
Clones:
basket.js:1-9 ~ cart.js:1-9 (renamed)
limits-dev.js:1-13 ~ limits-prod.js:1-13 (renamed)
---
```

- Keywords keep their meaning (`return` never matches `retry`), so structure still has to match.
- `--ignore-annotations` only acts in Java, Kotlin, Scala, Groovy, Python, Dart, Swift, JavaScript and TypeScript; in Ruby, Perl, T-SQL, Razor and CSS `@` means something else and is left alone.
- Positions in the report still point at the original source.
- Config keys: `ignoreIdentifiers`, `ignoreLiterals`, `ignoreAnnotations`.

### Type-3: near-miss clones (`--max-gap-lines`, `--similarity`)

Copies with a few edited lines, or functions rewritten with the same shape:

```bash
npx jscpd --reporters ai --max-gap-lines 2 <path>      # a copy with up to 2 inserted/changed lines becomes one clone
npx jscpd --reporters ai --similarity 0.8 <path>       # JS/TS functions with ≥80% shared syntax-tree structure
```

```
Clones:
save-account.js:1-12 ~ save-user.js:1-11 [~0.91 gap]
credit-note.js:1-19 ~ invoice.js:1-17 [~0.75 ast]
---
```

- `--max-gap-lines N` only joins clones the exact run already found, so it removes fragmentation rather than inventing matches; it works in every language. A merge is refused when the gap holds more tokens than the halves share (similarity would drop under `0.5`).
- `--similarity RATIO` compares whole functions by the bag of 4-grams over their syntax-tree node types, so a renamed copy scores `1.0`, one inserted line about `0.9`, two added statements plus renames about `0.75`. Today it applies to JavaScript, TypeScript, JSX and TSX only; other formats are a silent no-op. Start at `0.85` for near-identical structure and lower to `0.7` only when looking for leads; below `0.8` a large share of pairs merely share an idiom, so read both functions before believing the score.
- `similar` takes precedence over `renamed` when both apply (a merged clone is no longer identical even after normalization).
- Config keys: `maxGapLines`, `similarity`.

### Where the kind shows up

- `console`: `Clone found (javascript, renamed)`, `Clone found (javascript, similar (gap) ~0.91)`, `Clone found (javascript, similar (ast) ~0.75)`.
- `json`: `"kind": "exact" | "renamed" | "similar"`, plus `"similarity"` and `"method": "gap" | "ast"` for similar clones.
- `sarif`: rules `jscpd/duplicate-code`, `jscpd/renamed-code`, `jscpd/similar-code`; Code Climate uses the same three `check_name` values.
- A default run reports only `exact` clones and its output is unchanged by these features.
- Normalized runs produce different clone fingerprints than exact runs: keep a separate `--baseline` file per configuration.

## Codebase Summary (`--summary`)

`--summary` appends a refactoring-hotspot overview to the run output — use it to decide **where to refactor first** before diving into individual clones:

```bash
# Compact clone list + compact summary, optimized for agents
npx jscpd --reporters ai --summary --no-tips <path>

# Rank by complexity instead of tokens, top 5 lists
npx jscpd --reporters ai --summary --summary-by complexity --summary-top 5 <path>
```

With the `ai` reporter the summary is one line per entry:

```
Summary by tokens (321 files, 129 folders):
files (tokens/lines/size/cx/dup%):
src/files.ts 2052/363/11662/80/0.0%
long-line/theme-branded.js 498/15/2.5K/10/93.3%
...
folders (files/tokens/lines/size):
src/core 8/5264/843/27136
...
```

How to read it:
- **Top files** are ranked by the `--summary-by` metric, but every row carries all metrics — `tokens/lines/size/cx/dup%`.
- **cx** is a language-agnostic cyclomatic-complexity estimate from the token stream (1 + decision-point tokens like `if`/`while`/`&&`); treat it as a ranking signal, not an exact metric.
- **dup%** is the share of the file's lines covered by detected clones — a large file with high `dup%` is the best refactoring target. It reflects the clones of the current run, so with the Type-2/Type-3 flags it rises accordingly.
- **Folders** aggregate files into their direct parent directory (no cumulative ancestor totals).
- In `console` reporters the summary renders as aligned tables; in the `json` report it appears as an additive `summary` key (absent when the flag is off).

Config file equivalents: `"summary": true`, `"summaryTop": 10`, `"summaryBy": "tokens"`.

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
  "ignoreIdentifiers": false,
  "ignoreLiterals": false,
  "maxGapLines": 0,
  "similarity": 1,
  "summary": false,
  "output": "./reports/jscpd"
}
```

## Refactoring Duplicated Code

Once you've detected clones, use the **dry-refactoring** skill for a guided workflow to eliminate them, with a strategy per clone kind:

→ **dry-refactoring** — step-by-step refactoring strategies and workflow for removing duplication. Install with:
  ```bash
  npx skills add https://github.com/kucherenko/jscpd --skill dry-refactoring
  ```
