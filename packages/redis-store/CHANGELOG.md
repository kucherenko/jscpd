# @jscpd/redis-store

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

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/core@4.1.1
  - @jscpd/tokenizer@4.1.1

A [Redis](https://redis.io/)-backed store for jscpd. Use this store when running jscpd across multiple machines or CI agents that need to share token data — for example, in a distributed monorepo setup where different services are scanned in parallel jobs.

---

## [4.1.0](https://www.npmjs.com/package/@jscpd/redis-store/v/4.1.0) — 2026-05-09

### Changes

- Aligned with the monorepo 4.1.0 release. No store-specific changes.
- CI now tests against Node.js 22.x and 24.x.

### Dependency Updates

- `@jscpd/core` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0

---

## [4.0.5](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.5) — 2026-04-10

### Changes

- Aligned with the AI reporter release. No store-specific changes.

### Dependency Updates

- `@jscpd/core` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5

---

## [4.0.4](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.4) — 2026-01-30

### Changes

- Aligned with the MCP server and GDScript support release.

### Dependency Updates

- `@jscpd/core` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4

---

## [4.0.3](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.3) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3

---

## [4.0.2](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.2) — 2026-01-11

### Changes

- Merged community PRs; minor housekeeping.

### Dependency Updates

- `@jscpd/core` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2

---

## [4.0.1](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.1) — 2024-05-26

### Changes

- First public release as a versioned standalone package under v4.

### Dependency Updates

- `@jscpd/core` → 4.0.1
- `@jscpd/tokenizer` → 4.0.1

---

## [4.0.0](https://www.npmjs.com/package/@jscpd/redis-store/v/4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructure** — package moved to `packages/redis-store`. Build system changed to `tsup`.
- Requires **Node.js 18+**.

### Dependency Updates

- `@jscpd/core` → 4.0.0
- `@jscpd/tokenizer` → 4.0.0

---

## [3.3.0-rc.3](https://github.com/kucherenko/jscpd/commit/9f388ff) — 2020-05-02

First release of `@jscpd/redis-store`. The Redis store was introduced in jscpd v3.3 to provide a shared, persistent token store for distributed CI environments. This initial standalone package release extracts it from the main monorepo for independent versioning.
