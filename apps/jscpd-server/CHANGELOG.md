# jscpd-server

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
  - @jscpd/finder@4.2.4
  - @jscpd/html-reporter@4.2.4
  - jscpd-sarif-reporter@4.2.4
  - @jscpd/tokenizer@4.2.4

## 4.2.3

### Patch Changes

- fix(finder): resolve relative ignore patterns against scan dirs (#611)
- Updated dependencies
  - @jscpd/core@4.2.3
  - @jscpd/finder@4.2.3
  - @jscpd/html-reporter@4.2.3
  - jscpd-sarif-reporter@4.2.3
  - @jscpd/tokenizer@4.2.3

## 4.2.2

### Patch Changes

- fix(tokenizer): resolve quadratic bash tokenization hang
- Updated dependencies
  - @jscpd/core@4.2.2
  - @jscpd/finder@4.2.2
  - @jscpd/html-reporter@4.2.2
  - jscpd-sarif-reporter@4.2.2
  - @jscpd/tokenizer@4.2.2

## 4.2.1

### Patch Changes

- fix tokenization issue for cross formats detection
- Updated dependencies
  - @jscpd/core@4.2.1
  - @jscpd/finder@4.2.1
  - @jscpd/html-reporter@4.2.1
  - jscpd-sarif-reporter@4.2.1
  - @jscpd/tokenizer@4.2.1

## 4.2.0 — 2026-05-14

### New Features

- **`--store-path` support** — the server now accepts a `storePath` option to configure a custom LevelDB cache directory, consistent with the `jscpd` CLI. Useful when running multiple server instances or integrating with CI environments that run parallel scans.

### Bug Fixes

- **Process crash on malformed `package.json`** (#739) — invalid JSON in the project's `package.json` threw an unhandled `SyntaxError` that killed the server process. The server now emits a warning and continues with an empty config.

### Dependency Updates

- `@jscpd/core` → 4.2.0
- `@jscpd/finder` → 4.2.0
- `@jscpd/html-reporter` → 4.2.0
- `@jscpd/tokenizer` → 4.2.0
- `jscpd-sarif-reporter` → 4.2.0

---

## 4.1.1

### Patch Changes

- Update hash function, improve performance and keep browser support
- Updated dependencies
  - @jscpd/core@4.1.1
  - @jscpd/finder@4.1.1
  - @jscpd/html-reporter@4.1.1
  - jscpd-sarif-reporter@4.1.1
  - @jscpd/tokenizer@4.1.1

An HTTP server that exposes jscpd's copy-paste detection as a RESTful API. Ideal for IDE plugins, CI services, and web dashboards that need on-demand duplicate analysis without launching a CLI process for every request.

---

## [4.1.0](https://www.npmjs.com/package/jscpd-server/v/4.1.0) — 2026-05-09

### New Features

- **MCP server enhancements** — the [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) server now exposes a `jscpd://statistics` resource endpoint, allowing AI agents and MCP-compatible clients to query duplication statistics directly.
- **Recheck endpoint** — a new API endpoint lets clients trigger a rescan of the codebase without restarting the server process. Useful for long-running server instances where the codebase changes over time.

### Changes

- CI now tests against Node.js 22.x and 24.x.

### Dependency Updates

- `@jscpd/core` → 4.1.0
- `@jscpd/finder` → 4.1.0
- `@jscpd/html-reporter` → 4.1.0
- `@jscpd/tokenizer` → 4.1.0
- `jscpd-sarif-reporter` → 4.1.0

---

## [4.0.9](https://www.npmjs.com/package/jscpd-server/v/4.0.9) — 2026-04-10

### Changes

- Aligned with the AI reporter release.

### Dependency Updates

- `@jscpd/core` → 4.0.5
- `@jscpd/finder` → 4.0.5
- `@jscpd/html-reporter` → 4.0.5
- `@jscpd/tokenizer` → 4.0.5
- `jscpd-sarif-reporter` → 4.0.7

---

## [4.0.8](https://www.npmjs.com/package/jscpd-server/v/4.0.8) — 2026-01-30

### New Features

- **MCP protocol server** — the server now implements the Model Context Protocol, enabling AI agents (such as those built with Claude, GPT, or other MCP-compatible clients) to interact with jscpd programmatically. Includes an icon-menu toggle in the web UI.

### Dependency Updates

- `@jscpd/core` → 4.0.4
- `@jscpd/finder` → 4.0.4
- `@jscpd/html-reporter` → 4.0.4
- `@jscpd/tokenizer` → 4.0.4
- `jscpd-sarif-reporter` → 4.0.6

---

## [4.0.7](https://www.npmjs.com/package/jscpd-server/v/4.0.7) — 2026-01-11

### Bug Fixes

- Fixed a build output issue.

### Dependency Updates

- `@jscpd/core` → 4.0.3
- `@jscpd/finder` → 4.0.3
- `@jscpd/html-reporter` → 4.0.3
- `@jscpd/tokenizer` → 4.0.3
- `jscpd-sarif-reporter` → 4.0.5

---

## [4.0.6](https://www.npmjs.com/package/jscpd-server/v/4.0.6) — 2026-01-11

### New Package

First release of `jscpd-server`. Provides a RESTful HTTP API for code-duplication detection built on top of the jscpd engine.

**Key endpoints (initial release):**

- `POST /detect` — submit a path or code snippet for duplicate analysis.
- `GET /report` — retrieve the latest detection results.

**To start the server:**

```sh
npx jscpd-server
```

### Dependency Updates

- `@jscpd/core` → 4.0.2
- `@jscpd/finder` → 4.0.2
- `@jscpd/html-reporter` → 4.0.2
- `@jscpd/tokenizer` → 4.0.2
- `jscpd-sarif-reporter` → 4.0.4
