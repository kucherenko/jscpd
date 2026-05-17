# @jscpd/leveldb-store

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

- **`--store-path` / `storePath` option** — the store now accepts a configurable root directory for its LevelDB data files. Previously the store always wrote to `.jscpd/` relative to the working directory, causing collisions when multiple jscpd processes ran in parallel. Set `storePath` to a unique path per process to avoid contention.

### Dependency Updates

- `@jscpd/core` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/core@4.1.1
  - @jscpd/tokenizer@4.1.1

A [LevelDB](https://github.com/google/leveldb)-backed persistent store for jscpd. Use this store when scanning very large codebases where keeping the entire token map in memory is impractical. Token data is flushed to disk and read back on demand, keeping the Node.js heap small.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.1.0) — 2026-05-09

### Changes

- Aligned with the monorepo 4.1.0 release. No store-specific changes.
- Upgraded `level` to v10.0.0.
- CI now tests against Node.js 22.x and 24.x.

### Dependency Updates

- `@jscpd/core` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release. No store-specific changes.

### Dependency Updates

- `@jscpd/core` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.4) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

### Dependency Updates

- `@jscpd/core` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.2) — 2026-01-11

### Changes

- Merged community PRs; minor housekeeping.

### Dependency Updates

- `@jscpd/core` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/core` → 4.0.1
- `@jscpd/tokenizer` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/leveldb-store/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — package moved to `packages/leveldb-store`. Build system changed to `tsup`.
- Requires **Node.js 18+**.

### Changes

- `level` dependency updated to v8+.

### Dependency Updates

- `@jscpd/core` → 4.0.0
- `@jscpd/tokenizer` → 4.0.0

---

## [3.3.0-rc.3](https://github.com/kucherenko/jscpd/commit/9f388ff) — 2020-05-02

First release as a standalone `@jscpd/leveldb-store` package.

- Extracted from the main `jscpd` package where LevelDB was the default store.
- Internal imports reorganised.
