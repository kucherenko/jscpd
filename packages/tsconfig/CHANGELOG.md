# @jscpd/tsconfig

## 5.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)

## 5.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang

## 5.2.1

### Patch Changes

- fix tokenization issue for cross formats detection

## 5.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support

Shared TypeScript compiler configuration for all packages and apps in the jscpd monorepo. Other packages extend this base `tsconfig.json` to ensure consistent compiler settings across the workspace.

---

## [5.1.0](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.1.0) — 2026-05-09

### Changes

- Aligned with the monorepo 4.1.0 release (the tsconfig package uses its own versioning scheme). No configuration changes.
- CI now tests against Node.js 22.x and 24.x.

---

## [5.0.5](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.5) — 2026-04-10

Aligned with the AI reporter release.

---

## [5.0.4](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.4) — 2026-01-30

Aligned with the MCP server and GDScript release.

---

## [5.0.3](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.3) — 2026-01-11

Build fix patch.

---

## [5.0.2](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.2) — 2026-01-11

Minor housekeeping.

---

## [5.0.1](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.1) — 2024-05-26

First public release under the v4 monorepo.

---

## [5.0.0](https://www.npmjs.com/package/@jscpd/tsconfig/v/5.0.0) — 2024-05-26

Initial release. Extracted shared TypeScript compiler settings into a standalone package as part of the v4 monorepo restructure. Packages now extend `@jscpd/tsconfig` instead of duplicating `tsconfig.json` settings.
