# @jscpd/tokenizer

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)
- Updated dependencies
  - @jscpd/core@4.2.3

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang
- Updated dependencies
  - @jscpd/core@4.2.2

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection
- Updated dependencies
  - @jscpd/core@4.2.1

## 4.2.0 — 2026-05-14

### New Features

- **reprism-based grammar engine** — replaced the `prismjs` npm package with a self-contained [reprism](https://github.com/tannerlinsley/reprism)-based backend. ~11.5% faster tokenization on real projects (avg 1126 ms → 997 ms on a 548-file, 223-format scan). Startup overhead is slightly higher (more grammars initialised) but per-file throughput is meaningfully faster.
- **Vue SFC per-block tokenization** — `.vue` files are now processed with sub-format dispatch: `<script>` → `javascript`, `<script lang="ts">` → `typescript`, `<template>` → `markup`, `<style>` → `css`, `<style lang="scss">` → `scss`, `<style lang="less">` → `less`. Enables cross-format duplicate detection between Vue SFC blocks and standalone source files.
- **Svelte SFC support** — `.svelte` files are tokenized per-block (script, style, markup), enabling detection of duplicated logic shared between Svelte components and standalone files.
- **Astro SFC support** — `.astro` files have their frontmatter and template blocks tokenized independently.
- **Markdown cross-format detection** — fenced code blocks in `.md` files are tokenized by the declared language, so a ` ```python ` block in Markdown can match a `.py` source file.
- **`txt` grammar** — plain-text files (`.txt`) are now a recognised format.
- **223 total supported formats** — up from 152 at 4.1.1.

### Bug Fixes

- **ReDoS hang on Lisp/Elisp files** (#737) — the Lisp string pattern `/"(?:[^"\\]*|\\.)*"/` could catastrophically backtrack (O(2ⁿ)) on unterminated string literals. Replaced with the linear equivalent `/"(?:[^"\\]|\\[\s\S])*"/`.
- **Vue SFC incorrect column numbers** — tokens on the first line of a block carried block-relative column 1. The offset (characters from the last newline to the block content start) is now added to every token on the block's opening line, producing correct file-absolute column numbers.

### Dependency Updates

- `@jscpd/core` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/core@4.1.1

Converts source code into token streams for duplicate detection. Supports 150+ languages via [Prism.js](https://prismjs.com/) grammars and handles format-to-extension mapping, hashing, and source-location calculation.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.1.0) — 2026-05-09

### Performance

This release contains the most significant tokenizer performance improvements since v4.0.0:

- **Lazy grammar loading** — Prism grammars are now loaded on demand (`ensureGrammarReady`) instead of all ~300 being imported at startup. Cold-start time drops dramatically on large codebases.
- **O(n) hot paths** — replaced O(n²) `concat`/spread patterns in `createTokens` and `groupByFormat` with `push` loops.
- **Faster line counting** — `calculateLocation` now uses a single-pass character loop instead of `split('\n')`, avoiding an extra array allocation per file.
- **Native hashing** — swapped `spark-md5` for Node.js built-in `crypto.createHash('md5')`. This also fixes a broken local binary issue and removes an external dependency.
- **O(1) format lookup** — added a reverse `Map` for extension→format resolution, removing a linear scan on every file.

### New Languages

- **Apex** (Salesforce) and **CFML** (ColdFusion) language support added with test fixtures (closes [#83](https://github.com/kucherenko/jscpd/issues/83), [#619](https://github.com/kucherenko/jscpd/issues/619)).

### Maintenance

- Replaced the vendored `reprism` syntax library with the official `prismjs` npm package, reducing the installed footprint.
- Added a comprehensive test suite: 117 tests covering hash, formats, token-map, and tokenize modules.
- CI now tests against Node.js 22.x and 24.x; Node.js 20.x dropped.

### Dependency Updates

- `@jscpd/core` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release. No tokenizer-specific changes.

### Dependency Updates

- `@jscpd/core` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.4) — 2026-01-30

### New Languages

- **GDScript** (Godot Engine) support added.

### Dependency Updates

- `@jscpd/core` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.2) — 2026-01-11

### Changes

- Merged several community pull requests; minor housekeeping.

### Dependency Updates

- `@jscpd/core` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/core` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/tokenizer/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — package moved to `packages/tokenizer`. Build system changed from `tsc` to `tsup` (ESM+CJS dual output).
- Replaced `prism.js` (the old bundled copy) with `reprism` during this transition.

### Changes

- Performance optimisation: language grammars loader refactored to load lazily.
- Cyclic dependency between `@jscpd/tokenizer` and `@jscpd/core` resolved.

### Dependency Updates

- `@jscpd/core` → 4.0.0

---

## [3.3.0-rc.3](https://github.com/kucherenko/jscpd/commit/9f388ff) — 2020-05-02

First release as a dedicated `@jscpd/tokenizer` package, extracted from the main `jscpd` package to allow independent development and versioning.

- Removed cyclic dependency with `@jscpd/core`.
- Internal imports reorganised.
