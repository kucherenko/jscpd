# @jscpd/finder

## 4.2.5

### Patch Changes

- Bug fixes: JSON reporter duplicate token counts, gitignore parent-directory walk, and Commander v15 migration
- Updated dependencies
  - @jscpd/core@4.2.5
  - @jscpd/tokenizer@4.2.5

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
  - @jscpd/core@4.2.4
  - @jscpd/tokenizer@4.2.4

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)
- Updated dependencies
  - @jscpd/core@4.2.3
  - @jscpd/tokenizer@4.2.3

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang
- Updated dependencies
  - @jscpd/core@4.2.2
  - @jscpd/tokenizer@4.2.2

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection
- Updated dependencies
  - @jscpd/core@4.2.1
  - @jscpd/tokenizer@4.2.1

## 4.2.0 — 2026-05-14

### New Features

- **Shebang-based format detection** — extensionless files that are executable (`chmod +x`) are now inspected for a `#!` shebang line. The interpreter path is mapped to a detection format (e.g. `/usr/bin/env python3` → `python`, `#!/bin/bash` → `bash`). Symlinks are excluded. This allows jscpd to tokenize scripts like `Makefile` runners, deployment helpers, and other extensionless executables that would previously be silently skipped.
- **`--formats-names` option** — map specific filenames to a format independent of file extension. Example: `--formats-names '{"python": ["Pipfile"], "yaml": ["Dockerfile.prod"]}'`. Useful for project-specific conventions where the filename is the canonical indicator of language.

### Dependency Updates

- `@jscpd/core` → 4.2.0
- `@jscpd/tokenizer` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/core@4.1.1
  - @jscpd/tokenizer@4.1.1

The detection engine that drives jscpd. It orchestrates file discovery, tokenisation, clone detection, and reporter invocation. Also home to the built-in reporters: `consoleFull`, `console`, `csv`, `markdown`, `json`, `xml`, `xcode`, `threshold`, `silent`, `execTime`, and the new `ai` reporter.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/finder/v/4.1.0) — 2026-05-09

### New Features

- **AI reporter** (`--reporters ai`) — compact, token-efficient clone output designed for piping results into language models and AI tooling. Each duplicate is formatted to minimise token usage without losing structural meaning.
- **`--noTips` flag** — suppress the usage-tip message printed after a detection run finishes.
- **Execution timer tip** — a timing hint is now shown after detection completes (suppressed by `--noTips`).

### Improvements

- **Test coverage** — 92 new tests added (178 → 270 total), covering reporters, subscribers, validators, hooks, and integration scenarios across all detection modes. Fixture files added for 40+ language formats.

### Bug Fixes

- Dependencies updated to consume the performance improvements in `@jscpd/tokenizer` 4.1.0 (lazy grammar loading, O(n) hot paths, native MD5 hashing).

### Dependency Updates

- `@jscpd/core` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/finder/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release. The `ai` reporter is registered here; this patch updates the dependency chain.

### Dependency Updates

- `@jscpd/core` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/finder/v/4.0.4) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

### Dependency Updates

- `@jscpd/core` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/finder/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/finder/v/4.0.2) — 2026-01-11

### Bug Fixes

- Fixed gitignore pattern parsing: leading-slash patterns and dot-prefixed patterns are now handled correctly. The `gitignore-to-glob` dependency was removed; pattern parsing is now done inline.

### Changes

- Merged several community PRs including minor cleanup and improvements.

### Dependency Updates

- `@jscpd/core` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/finder/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/core` → 4.0.1
- `@jscpd/tokenizer` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/finder/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — package moved to `packages/finder`. Build system changed to `tsup` (ESM+CJS dual output). Test runner migrated to Vitest.

### Changes

- Reporters and detection modes consolidated here from the old package structure.
- Unused constructor removed.

### Dependency Updates

- `@jscpd/core` → 4.0.0
- `@jscpd/tokenizer` → 4.0.0

---

## [3.3.0-rc.3](https://github.com/kucherenko/jscpd/commit/9f388ff) — 2020-05-02

First release as a dedicated `@jscpd/finder` package.

- Extracted file-finding and detection logic from the main `jscpd` package.
- Internal imports reorganised; cyclic dependency with `@jscpd/core` removed.
