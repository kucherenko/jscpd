# Packages

The `master-v4` branch is a pnpm workspace containing the two v4 apps and the supporting `@jscpd/*` packages. All of them are published to npm from this branch (see [CONTRIBUTING.md](../CONTRIBUTING.md) for the changeset-based release flow).

## Apps

### jscpd

**Path:** `apps/jscpd`
**npm:** [`jscpd`](https://www.npmjs.com/package/jscpd) (dist-tag `latest-4`)
**Version:** 4.3.0

Main package for jscpd — CLI and Node.js API for copy/paste detection. See [TypeScript docs](./typescript.md).

### jscpd-server

**Path:** `apps/jscpd-server`
**npm:** [`jscpd-server`](https://www.npmjs.com/package/jscpd-server)
**Version:** 4.3.0

Standalone server application providing a REST API and an MCP server (Streamable HTTP) for on-demand code duplication detection. See [AI-Ready docs](./ai-ready.md) for details.

## Packages

### @jscpd/core

**Path:** `packages/core`
**npm:** [`@jscpd/core`](https://www.npmjs.com/package/@jscpd/core)
**Version:** 4.2.5

Core detection algorithm. Implements Rabin-Karp rolling hash for finding duplicate code blocks. Single dependency on `eventemitter3`. Provides `IClone`, `IMapFrame`, `IOptions`, `IStatistic`, `MemoryStore`, and event interfaces.

### @jscpd/finder

**Path:** `packages/finder`
**npm:** [`@jscpd/finder`](https://www.npmjs.com/package/@jscpd/finder)
**Version:** 4.3.0

Detector of duplications in files. Walks the filesystem (with `.gitignore` support), runs clone detection, provides the built-in reporters (`console`, `consoleFull`, `json`, `xml`, `csv`, `markdown`, `ai`, `xcode`, `threshold`, `silent`), subscribers, validators, and hooks.

### @jscpd/tokenizer

**Path:** `packages/tokenizer`
**npm:** [`@jscpd/tokenizer`](https://www.npmjs.com/package/@jscpd/tokenizer)
**Version:** 4.2.6

Tokenizer — converts source code into tokens for duplicate detection. Supports 224 languages/formats via a reprism-based grammar engine with lazy loading. Cross-format tokenization for Vue SFC, Svelte, Astro, and Markdown. The format list in [FORMATS.md](../FORMATS.md) is derived from `packages/tokenizer/src/formats.ts`.

### @jscpd/html-reporter

**Path:** `packages/html-reporter`
**npm:** [`@jscpd/html-reporter`](https://www.npmjs.com/package/@jscpd/html-reporter)
**Version:** 4.2.5

HTML reporter — generates an interactive HTML report with per-format statistics, duplication graph, and syntax-highlighted clone diffs.

### @jscpd/badge-reporter

**Path:** `packages/badge-reporter`
**npm:** [`@jscpd/badge-reporter`](https://www.npmjs.com/package/@jscpd/badge-reporter)
**Version:** 4.2.5

Badge reporter — generates SVG badges showing the copy/paste level.

### jscpd-sarif-reporter

**Path:** `packages/sarif-reporter`
**npm:** [`jscpd-sarif-reporter`](https://www.npmjs.com/package/jscpd-sarif-reporter)
**Version:** 4.2.5

SARIF reporter — generates Static Analysis Results Interchange Format output for GitHub Code Scanning. Emits warning-level results per clone, plus an error if the threshold is exceeded.

### @jscpd/leveldb-store

**Path:** `packages/leveldb-store`
**npm:** [`@jscpd/leveldb-store`](https://www.npmjs.com/package/@jscpd/leveldb-store)
**Version:** 4.2.6

LevelDB store — persistent disk-backed token store for large repositories. Slower than the default in-memory store but can handle very large codebases.

### @jscpd/redis-store

**Path:** `packages/redis-store`
**npm:** [`@jscpd/redis-store`](https://www.npmjs.com/package/@jscpd/redis-store)
**Version:** 4.2.6

Redis store — offloads the in-memory hash map to Redis. Useful for large codebases or distributed/CI environments.

## Internal

### @jscpd/tsconfig

**Path:** `packages/tsconfig` (private, not published)

Shared TypeScript configuration for the workspace.
