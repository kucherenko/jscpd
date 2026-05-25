# @jscpd/html-reporter

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

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection

## 4.2.0 — 2026-05-14

### Dependency Updates

- `@jscpd/core` → 4.2.0
- `@jscpd/finder` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support

Generates a self-contained HTML report from jscpd clone results. The report shows duplicate code side-by-side with syntax highlighting, file statistics, and a summary of duplication across the scanned codebase.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.1.0) — 2026-05-09

### New Features

- **Branded footer** — the generated HTML report now includes a footer displaying the jscpd version, a project badge, and a sponsor link, making it easy to tell which version produced a given report.

### Dependency Updates

- `@jscpd/finder` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release cycle.

### Dependency Updates

- `@jscpd/finder` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.4) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

### Dependency Updates

- `@jscpd/finder` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue that caused the report assets to be missing.

### Dependency Updates

- `@jscpd/finder` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.2) — 2026-01-11

### Changes

- Merged community PRs; minor housekeeping.

### Dependency Updates

- `@jscpd/finder` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/finder` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/html-reporter/v/4.0.0) — 2024-05-26

### Breaking Changes

- **HTML reporter rebuilt** — the report was redesigned as a standalone, dependency-free HTML page. The previous Vue.js single-page application has been removed, making the output simpler to open, share, and archive without needing a local server.
- Build system changed from `tsc` to `tsup`.

### Changes

- HTML tags in code blocks are now properly escaped, fixing rendering issues when scanned code contained `<` / `>` characters.
- `pug` dependency upgraded.

### Dependency Updates

- `@jscpd/finder` → 4.0.0
