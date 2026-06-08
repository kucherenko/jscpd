# Packages

The jscpd monorepo contains two apps and several supporting packages.

## Apps

### jscpd

**Path:** `apps/jscpd`
**npm:** [`jscpd`](https://www.npmjs.com/package/jscpd)
**Version:** 4.2.5

Main package for jscpd — CLI and Node.js API for copy/paste detection. See [TypeScript docs](./typescript.md).

### jscpd-server

**Path:** `apps/jscpd-server`
**npm:** [`jscpd-server`](https://www.npmjs.com/package/jscpd-server)
**Version:** 4.2.5

Standalone server application providing REST API and MCP server for on-demand code duplication detection. See [AI-Ready docs](./ai-ready.md) for details.

## Packages (TypeScript / Node.js)

### @jscpd/core

**Path:** `packages/core`
**npm:** [`@jscpd/core`](https://www.npmjs.com/package/@jscpd/core)
**Version:** 4.2.5

Core detection algorithm. Implements Rabin-Karp rolling hash for finding duplicate code blocks. Single dependency on `eventemitter3`. Provides `IClone`, `IMapFrame`, `MemoryStore`, and event interfaces.

### @jscpd/finder

**Path:** `packages/finder`
**npm:** [`@jscpd/finder`](https://www.npmjs.com/package/@jscpd/finder)
**Version:** 4.2.5

Detector of duplications in files. Walks filesystem, runs clone detection, provides built-in reporters, subscribers, validators, and hooks.

### @jscpd/tokenizer

**Path:** `packages/tokenizer`
**npm:** [`@jscpd/tokenizer`](https://www.npmjs.com/package/@jscpd/tokenizer)
**Version:** 4.2.5

Tokenizer — converts source code into tokens for duplicate detection. Supports 224 languages/formats via reprism-based grammar engine with lazy loading. Cross-format tokenization for Vue SFC, Svelte, Astro, and Markdown.

### @jscpd/html-reporter

**Path:** `packages/html-reporter`
**npm:** [`@jscpd/html-reporter`](https://www.npmjs.com/package/@jscpd/html-reporter)
**Version:** 4.2.5

HTML reporter — generates interactive HTML report with per-format statistics, duplication graph, and syntax-highlighted clone diffs.

### @jscpd/badge-reporter

**Path:** `packages/badge-reporter`
**npm:** [`@jscpd/badge-reporter`](https://www.npmjs.com/package/@jscpd/badge-reporter)
**Version:** 4.2.5

Badge reporter — generates SVG badges showing copy/paste level.

### jscpd-sarif-reporter

**Path:** `packages/sarif-reporter`
**npm:** [`jscpd-sarif-reporter`](https://www.npmjs.com/package/jscpd-sarif-reporter)
**Version:** 4.2.5

SARIF reporter — generates Static Analysis Results Interchange Format output for GitHub Code Scanning. Emits warning-level results per clone, plus error if threshold exceeded.

### @jscpd/leveldb-store

**Path:** `packages/leveldb-store`
**npm:** [`@jscpd/leveldb-store`](https://www.npmjs.com/package/@jscpd/leveldb-store)
**Version:** 4.2.5

LevelDB store — persistent disk-backed token store for large repositories. Slower than default in-memory store but can handle very large codebases.

### @jscpd/redis-store

**Path:** `packages/redis-store`
**npm:** [`@jscpd/redis-store`](https://www.npmjs.com/package/@jscpd/redis-store)
**Version:** 4.2.5

Redis store — offloads in-memory hash map to Redis. Useful for large codebases or distributed/CI environments.

## Crates (Rust / v5)

### cpd (binary)

**Path:** `rust/crates/cpd`
**npm:** [`jscpd@5`](https://www.npmjs.com/package/jscpd) (installs both `jscpd` and `cpd` commands) | [`cpd`](https://www.npmjs.com/package/cpd) (installs `cpd` command only)
**crates.io:** [`jscpd`](https://crates.io/crates/jscpd) (installs both `jscpd` and `cpd` binaries)
**Version:** 5.0.4 (npm) / 0.1.4 (crates.io)

CLI binary, entry point. Published as `jscpd@5` on npm (self-contained binary, installs both `jscpd` and `cpd` commands, no Node.js runtime) and `cpd` on npm (installs only `cpd` command). See [Rust docs](./rust.md).

### cpd-core

**Path:** `rust/crates/cpd-core`
**Version:** 0.1.3

Core data models and Rabin-Karp rolling hash implementation.

### cpd-tokenizer

**Path:** `rust/crates/cpd-tokenizer`
**Version:** 0.1.3

Source code tokenizer (223+ formats). Uses `oxc_parser` for Go, TypeScript/JSX tokenization.

### cpd-finder

**Path:** `rust/crates/cpd-finder`
**Version:** 0.1.4

File walking, orchestration, and git blame. Uses `rayon` for parallelism, `ignore` + `globset` for file matching.

### cpd-reporter

**Path:** `rust/crates/cpd-reporter`
**Version:** 0.1.4

Output format rendering for 13 reporters.