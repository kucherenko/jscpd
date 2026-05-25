# jscpd

## 4.2.4

### Patch Changes

- Features
  - detectClonesAndStatistic() API — new function returning both clone results and statistics in one call; also exposes an optional statisticProvider injection point on detectClones. Closes #536, #549.
    Bug Fixes
  - .gitignore not respected by default — gitignore option now defaults to true; patterns are read from every scanned directory (not just process.cwd()); fix applies to both CLI and programmatic API. Use --no-gitignore to opt out. Fixes #790.
  - .gitignore negation patterns silently dropped — negated patterns (!test.js, !src/\*\*, etc.) were discarded instead of being passed to fast-glob. Fixes #723.
    Documentation
  - Document path option in .jscpd.json and package.json config examples. (#717)
  - Add Gitignore option section to README with CLI examples, config snippet, and default/type reference.
  - Add detectClonesAndStatistic API example to README.
- Updated dependencies
  - @jscpd/badge-reporter@4.2.4
  - @jscpd/core@4.2.4
  - @jscpd/finder@4.2.4
  - @jscpd/html-reporter@4.2.4
  - jscpd-sarif-reporter@4.2.4
  - @jscpd/tokenizer@4.2.4

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)
- Updated dependencies
  - @jscpd/badge-reporter@4.2.3
  - @jscpd/core@4.2.3
  - @jscpd/finder@4.2.3
  - @jscpd/html-reporter@4.2.3
  - jscpd-sarif-reporter@4.2.3
  - @jscpd/tokenizer@4.2.3

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang
- Updated dependencies
  - @jscpd/badge-reporter@4.2.2
  - @jscpd/core@4.2.2
  - @jscpd/finder@4.2.2
  - @jscpd/html-reporter@4.2.2
  - jscpd-sarif-reporter@4.2.2
  - @jscpd/tokenizer@4.2.2

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection
- Updated dependencies
  - @jscpd/badge-reporter@4.2.1
  - @jscpd/core@4.2.1
  - @jscpd/finder@4.2.1
  - @jscpd/html-reporter@4.2.1
  - jscpd-sarif-reporter@4.2.1
  - @jscpd/tokenizer@4.2.1

## 4.2.0 — 2026-05-14

### New Features

- **Custom tokenizer backend** — `@jscpd/tokenizer` now uses a self-contained reprism-based engine. ~11.5% faster tokenization on real projects (avg 1126 ms → 997 ms on a 548-file, 223-format scan).
- **Cross-format detection** — Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block/per-section. Enables duplicate detection between embedded blocks and standalone source files.
- **223 supported formats** — Apex, CFML/ColdFusion, GDScript, Svelte, Astro, and 70+ additional languages (up from 152). Run `jscpd --list` to see the full list.
- **Shebang detection** — extensionless executable scripts are auto-detected via their `#!` shebang line.
- **`--store-path`** — specify a custom directory for the LevelDB token cache, eliminating collisions when multiple jscpd processes run concurrently. Example: `jscpd --store-path /tmp/jscpd-worker-1 src/`.
- **`--skipComments`** — shorthand for `--mode weak`. Strips comments before tokenization.
- **`--formats-names`** — map specific filenames (e.g. `Makefile`, `Dockerfile`) to a detection format.

### Bug Fixes

- **Entire-file duplicates silently dropped** (#728) — files that are complete copies of each other were previously undetected due to a RabinKarp end-of-file flush bug. Fixed in `@jscpd/core`.
- **ReDoS hang on Lisp/Elisp files** (#737) — catastrophic backtracking in the Lisp string regex replaced with a linear pattern. Fixed in `@jscpd/tokenizer`.
- **Process crash on malformed `package.json`** (#739) — invalid JSON in `package.json` threw an unhandled `SyntaxError`. jscpd now emits a warning and continues with an empty config.
- **Vue SFC cross-file detection broken** — SFC blocks now use their resolved sub-format as the store namespace, enabling cross-file clone detection.
- **Vue SFC incorrect column numbers** — fixed in `@jscpd/tokenizer`.
- **50 dependency security vulnerabilities** remediated.

### Dependency Updates

- `@jscpd/badge-reporter` → 4.2.0
- `@jscpd/core` → 4.2.0
- `@jscpd/finder` → 4.2.0
- `@jscpd/html-reporter` → 4.2.0
- `jscpd-sarif-reporter` → 4.2.0
- `@jscpd/tokenizer` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/badge-reporter@4.1.1
  - @jscpd/core@4.1.1
  - @jscpd/finder@4.1.1
  - @jscpd/html-reporter@4.1.1
  - jscpd-sarif-reporter@4.1.1
  - @jscpd/tokenizer@4.1.1

Copy-paste detector for programming source code. Detects duplicated blocks across 150+ languages and outputs results in a variety of formats: console, HTML, JSON, XML, CSV, Markdown, SARIF, and more.

---

## [4.1.0](https://www.npmjs.com/package/jscpd/v/4.1.0) — 2026-05-09

### New Features

- **AI reporter** (`--reporters ai`) — compact, token-efficient output designed for feeding clone results directly into language models and AI tooling. Each duplicate is summarised in a minimal format that preserves structural meaning while using as few tokens as possible.
- **`--noTips` flag** — suppress the usage-tip message that appears after a detection run finishes. Useful in scripts and CI pipelines where extra output is noise.
- **Execution timer** — a timing summary is shown after detection completes, giving you a quick sense of scan duration. Suppressed by `--noTips`.

### Performance

- **Tokenizer speed** — significant improvements flow in from `@jscpd/tokenizer` 4.1.0: lazy Prism grammar loading, O(n) hot paths, native MD5 hashing (removing `spark-md5`), and an O(1) extension-to-format lookup. Cold-start time and memory usage are noticeably reduced on large codebases.

### New Languages

- **Apex** (Salesforce) and **CFML** (ColdFusion Markup Language) added to the supported languages list.

### Tests

- Test coverage raised to 98%+ (92 new tests added, covering reporters, subscribers, validators, hooks, and all detection modes with 40+ language fixtures).

### Dependency Updates

- `@jscpd/badge-reporter` → 4.1.0
- `@jscpd/core` → 4.1.0
- `@jscpd/finder` → 4.1.0
- `@jscpd/html-reporter` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0
- `jscpd-sarif-reporter` → 4.1.0

---

## [4.0.9](https://www.npmjs.com/package/jscpd/v/4.0.9) — 2026-04-10

### New Features

- **AI reporter** integrated as a named reporter. Use `--reporters ai` to activate it.

### Dependency Updates

- `@jscpd/badge-reporter` → 4.0.5
- `@jscpd/core` → 4.0.5
- `@jscpd/finder` → 4.0.5
- `@jscpd/html-reporter` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5
- `jscpd-sarif-reporter` → 4.0.7

---

## [4.0.8](https://www.npmjs.com/package/jscpd/v/4.0.8) — 2026-01-30

### New Features

- **GDScript support** — detect duplicate code in Godot Engine GDScript files.

### Dependency Updates

- `@jscpd/badge-reporter` → 4.0.4
- `@jscpd/core` → 4.0.4
- `@jscpd/finder` → 4.0.4
- `@jscpd/html-reporter` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4
- `jscpd-sarif-reporter` → 4.0.6

---

## [4.0.7](https://www.npmjs.com/package/jscpd/v/4.0.7) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/badge-reporter` → 4.0.3
- `@jscpd/core` → 4.0.3
- `@jscpd/finder` → 4.0.3
- `@jscpd/html-reporter` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3
- `jscpd-sarif-reporter` → 4.0.5

---

## [4.0.6](https://www.npmjs.com/package/jscpd/v/4.0.6) — 2026-01-11

### Bug Fixes

- Fixed gitignore pattern parsing (leading-slash and dot-prefixed patterns).
- Merged several community pull requests.

### Dependency Updates

- `@jscpd/badge-reporter` → 4.0.2
- `@jscpd/core` → 4.0.2
- `@jscpd/finder` → 4.0.2
- `@jscpd/html-reporter` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2
- `jscpd-sarif-reporter` → 4.0.4

---

## [4.0.4](https://www.npmjs.com/package/jscpd/v/4.0.4) — 2024-05-28

### Bug Fixes

- Fixed package resolution issues in the SARIF reporter integration.

### Dependency Updates

- `jscpd-sarif-reporter` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/jscpd/v/4.0.2) — 2024-05-28

### New Features

- **SARIF reporter** integrated as a named reporter. Use `--reporters sarif` to generate SARIF output for GitHub Code Scanning and similar tools.

### Dependency Updates

- `jscpd-sarif-reporter` → 4.0.0

---

## [4.0.1](https://www.npmjs.com/package/jscpd/v/4.0.1) — 2024-05-26

### Bug Fixes

- Fixed TypeScript type-declaration generation.

### Dependency Updates

- `@jscpd/core` → 4.0.1
- `@jscpd/finder` → 4.0.1
- `@jscpd/html-reporter` → 4.0.1
- `@jscpd/tokenizer` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/jscpd/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — the tool has been moved to `apps/jscpd` and all sub-packages have been extracted into `packages/*`. If you import internal jscpd modules directly, you will need to update your import paths.
- **Build system replaced** — migrated from `tsc` to `tsup`, producing clean ESM+CJS dual-mode bundles.
- **Test runner migrated** — switched from `ava` to [Vitest](https://vitest.dev/).
- **Requires Node.js 18+.**

### Changes

- Monorepo migrated from Lerna to [Turborepo](https://turbo.build/).
- [Changesets](https://github.com/changesets/changesets) adopted as the release management tool.

### Dependency Updates

- `@jscpd/core` → 4.0.0
- `@jscpd/finder` → 4.0.0
- `@jscpd/html-reporter` → 4.0.0
- `@jscpd/tokenizer` → 4.0.0

---

## Earlier releases (v1.x – v3.x)

See the [root CHANGELOG](../../CHANGELOG.md) for the full history of jscpd versions 1.0.0 through 3.5.10, covering the tool's origins, the TypeScript rewrite, LevelDB store, pluggable reporters, monorepo extraction, and all the improvements made from 2018 through 2023.
