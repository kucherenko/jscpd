# @jscpd/badge-reporter

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

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support

Generates a shield badge (SVG) showing your project's copy-paste duplication percentage. Drop the badge URL straight into your README to give contributors an at-a-glance view of code health.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.1.0) — 2026-05-09

### Changes

- Aligned with the monorepo 4.1.0 release. No badge-specific changes.
- CI now tests against Node.js 22.x and 24.x.

### Dependency Updates

- `@jscpd/core` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release. No badge-specific changes.

### Dependency Updates

- `@jscpd/core` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.4) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

### Dependency Updates

- `@jscpd/core` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.2) — 2026-01-11

### Changes

- Merged community PRs; minor housekeeping.

### Dependency Updates

- `@jscpd/core` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/core` → 4.0.1
- `@jscpd/tokenizer` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/badge-reporter/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — package moved to `packages/badge-reporter`. Build system changed to `tsup`.

### Changes

- `colors` added as an explicit runtime dependency (was previously an implicit transitive dependency).

---

## Prior history

The badge reporter was first introduced in **jscpd v3.3.22** (December 2020) as part of the main `jscpd` package before being extracted into a standalone `@jscpd/badge-reporter` package in v4.0.0.
