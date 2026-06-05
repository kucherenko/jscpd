# Changelog

All notable changes to **jscpd** are documented here. Releases follow [Semantic Versioning](https://semver.org/).

---

## 4.3.0 — 2026-06-04

### Breaking Changes

- **Reporter trait signature** — the `Reporter` trait method signature has changed from `fn report(&self, statistics: &Statistics)` to `fn report(&self, ctx: &ReportContext)`. All custom reporter implementations must be updated to access statistics through `ctx.statistics`.

  **Migration example:**
  ```rust
  // Before
  impl Reporter for MyReporter {
      fn report(&self, statistics: &Statistics) {
          println!("Total files: {}", statistics.total.sources);
      }
  }

  // After
  impl Reporter for MyReporter {
      fn report(&self, ctx: &ReportContext) {
          println!("Total files: {}", ctx.statistics.total.sources);
          println!("Execution time: {:?}", ctx.duration);
      }
  }
  ```

- **Console output format** — when using the time reporter (`--reporters time`), timing information is printed to stdout before the primary reporter output. Scripts that parse jscpd console output may need updating to handle the timing line (format: `time: 123.456ms` for durations < 1000ms, `time: 2.34s` for durations >= 1000ms).

### New Features

- **Time reporter** — new `time` reporter that displays execution timing using a decorator pattern. Activated via `--reporters time` and wraps the default console reporter unless another primary reporter is specified. Timing output format is adaptive: milliseconds for durations under 1 second (e.g., `time: 123.456ms`), seconds for longer durations (e.g., `time: 2.34s`). Matches TypeScript jscpd's time reporter behavior for parity.
- **CLI short-form aliases** — added 11 short-form aliases matching TypeScript jscpd conventions for frequently-used options:
  - `-l` for `--min-lines`
  - `-k` for `--min-tokens`
  - `-x` for `--max-lines`
  - `-z` for `--max-size`
  - `-r` for `--reporters`
  - `-o` for `--output`
  - `-t` for `--threshold`
  - `-m` for `--mode`
  - `-f` for `--format`
  - `-i` for `--ignore-pattern`
  - `-b` for `--blame`
- **ReportContext data structure** — new `ReportContext` struct encapsulates statistics reference and execution duration, providing extensibility for future context fields without polluting the core Statistics data model.

### Internal

- **Comprehensive integration tests** — added `tests/reporters_integration.rs` covering all 13 reporters (json, console, xml, csv, html, markdown, sarif, ai, badge, xcode, threshold, silent, console-full) with time reporter wrapper verification.
- **Timer infrastructure** — execution timing captured using `std::time::Instant` with negligible overhead (< 5ms impact or < 0.5% of total runtime).

---

## 4.2.0 — 2026-05-14

### Breaking Changes

- **Vue SFC tokenization** — `.vue` files are no longer tokenized as `markup`. Each block is now dispatched to its own sub-format: `<script>` → `javascript`, `<script lang="ts">` → `typescript`, `<template>` → `markup`, `<style>` → `css`, `<style lang="scss">` → `scss`, `<style lang="less">` → `less`. Clone reports for `.vue` files now appear under these resolved sub-format names. Any tooling or configuration that relied on `.vue` clones being reported under `markup` must be updated.
- **`--formatsExts` users** — custom mappings that pointed `.vue` to `markup` (e.g. `"formatsExts": { "markup": ["vue"] }`) will no longer take effect because `.vue` is handled by the dedicated `vue` format processor. Remove or update such mappings.

### New Features

- **Custom tokenizer backend** — replaced the `prismjs` npm package with a self-contained [reprism](https://github.com/tannerlinsley/reprism)-based grammar engine. ~11.5% faster tokenization on real projects (avg 1126 ms → 997 ms on a 548-file, 223-format scan).
- **Cross-format detection** — Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are now tokenized per-block/per-section. A `<script>` block in a `.vue` file can match a `.ts` file; a fenced code block in Markdown can match a `.py` file.
- **223 supported formats** — Apex, CFML/ColdFusion, GDScript, Svelte, Astro, and 70+ additional languages added (up from 152). See [FORMATS.md](FORMATS.md).
- **Shebang detection** — extensionless executable scripts (e.g. `/usr/bin/env python3`) are auto-detected by their `#!` shebang line and tokenized in the correct language.
- **`--store-path`** — configure a custom directory for the LevelDB cache, eliminating collisions when multiple jscpd processes run in parallel on the same machine.
- **`--skipComments`** — shorthand flag for `--mode weak`, which strips comments before detection.
- **`--formats-names`** — map specific filenames (e.g. `Makefile`, `Dockerfile`) to a detection format.

### Bug Fixes

- **Entire-file duplicates silently dropped** (`@jscpd/core` #728) — RabinKarp flushed the pending clone on a store *hit* at end-of-file instead of on a *miss*. Files that are complete copies of each other were undetected. Fixed.
- **ReDoS hang on Lisp/Elisp files** (`@jscpd/tokenizer` #737) — the Lisp string regex `/"(?:[^"\\]*|\\.)*"/` could catastrophically backtrack (O(2ⁿ)) on unterminated strings. Replaced with a linear `/"(?:[^"\\]|\\[\s\S])*"/` pattern.
- **Process crash on malformed `package.json`** (#739) — `readJSONSync` threw an unhandled `SyntaxError` when `package.json` contained invalid JSON, killing the process. Now emits a warning and continues with an empty config.
- **Vue SFC cross-file detection broken** — the detector used the file-level format (`vue`) as the store namespace for all SFC blocks, preventing a `<script>` block in one `.vue` file from ever matching a `<script>` block in another. The namespace now reflects each block's resolved sub-format.
- **Vue SFC incorrect column numbers** — tokens on the first line of a block carried block-relative column 1 instead of file-absolute column numbers. Fixed in `@jscpd/tokenizer`.
- **50 dependency security vulnerabilities** remediated across the monorepo (Dependabot batches).

### Known Limitations

- Malformed SFC blocks (e.g. unclosed tags, invalid attributes) are silently skipped and do not contribute tokens.

---

## [4.1.0](https://github.com/kucherenko/jscpd/compare/jscpd@4.0.7...jscpd@4.1.0) — 2026-05-09

### New Features

- **AI Reporter** — new `ai` reporter that produces compact, token-efficient clone output specifically designed for feeding results into language models and AI tooling. Use `--reporters ai` to activate it.
- **MCP Server enhancements** — the Model Context Protocol server now exposes a `jscpd://statistics` resource and supports a recheck endpoint so AI agents can trigger a rescan without restarting the process.
- **Apex & CFML language support** — jscpd can now detect duplicate code in Salesforce Apex and ColdFusion Markup Language (CFML) files (closes [#83](https://github.com/kucherenko/jscpd/issues/83), [#619](https://github.com/kucherenko/jscpd/issues/619)).
- **GDScript support** — detect copy-paste duplication in Godot Engine GDScript files.
- **HTML reporter footer** — the HTML report now displays a branded footer with the jscpd version and a sponsor link.
- **`--noTips` flag** — suppress the usage-tip messages that appear after a detection run.
- **CI: Node.js 22.x / 24.x** — continuous integration updated to test against the latest Node.js LTS and current releases.

### Performance

- **Tokenizer** — grammars are now loaded lazily, hot paths are O(n), and the `spark-md5` dependency has been removed in favour of a lighter built-in implementation. Startup time and memory usage are noticeably reduced on large codebases.
- Replaced the vendored `reprism` syntax library with the official `prismjs` npm package, shrinking the installed footprint.

### Bug Fixes

- Restored the correct `start.line` expectation for weak-mode clone detection.

---

## [4.0.7](https://github.com/kucherenko/jscpd/compare/jscpd@4.0.6...jscpd@4.0.7) — 2026-01-11

### New Features

- **jscpd-server** — a new `jscpd-server` package ships a RESTful HTTP API for code-duplication detection. Ideal for CI pipelines, IDE plugins, and services that need on-demand analysis without spinning up a CLI process.
- **GitHub Actions example** — an `example_github_action.yml` starter workflow is included in the repository.

### Bug Fixes

- Ignore patterns defined in configuration files are now applied correctly (the path-matching bug in `resolveIgnorePattern` has been fixed).
- Importing jscpd as a Node.js module no longer auto-executes the CLI entry point.
- Fixed an invalid documentation link.

---

## [4.0.6](https://github.com/kucherenko/jscpd/compare/jscpd@4.0.5...jscpd@4.0.6) — 2026-01-11

### Bug Fixes

- Dependency and lock-file updates to address security advisories.

---

## [4.0.5](https://github.com/kucherenko/jscpd/compare/jscpd@4.0.1...jscpd@4.0.5) — 2024-07-03

### New Features

- **SARIF reporter** — jscpd now supports the [SARIF](https://sarifweb.azurewebsites.net/) output format (Static Analysis Results Interchange Format), making it easy to integrate reports with GitHub Code Scanning and other SARIF-aware tooling. Use `--reporters sarif`.

### Bug Fixes

- Fixed TypeScript type-declaration generation for the jscpd app package.
- Fixed `colors` being a missing runtime dependency in the SARIF reporter.

---

## [4.0.0](https://github.com/kucherenko/jscpd/compare/v3.5.10...jscpd@4.0.0) — 2024-05-26

### Breaking Changes

- **Monorepo restructured** — packages have been reorganised and renamed. If you import sub-packages directly (e.g. `@jscpd/core`, `@jscpd/finder`) please review the updated package names and paths.
- **Build system replaced** — switched from the old TypeScript compiler pipeline to `tsup-node`, which produces cleaner ESM/CJS dual-mode bundles.
- **Test framework migrated** — tests are now powered by [Vitest](https://vitest.dev/) instead of the previous runner.

### Highlights

This is a major release that brings the entire jscpd ecosystem up to modern tooling standards. The public API remains largely compatible, but the internal architecture, package layout, and build artefacts have changed significantly.

---

## [3.5.10](https://github.com/kucherenko/jscpd/compare/v3.5.9...v3.5.10) — 2023-09-17

### Maintenance

- Updated dependencies that had known issues.
- Added a `dependabot.yml` configuration to keep dependencies up to date automatically.

---

## [3.5.9](https://github.com/kucherenko/jscpd/compare/v3.5.8...v3.5.9) — 2023-05-02

### Bug Fixes

- Fixed an issue where files that had not been published were incorrectly processed.

---

## [3.5.8](https://github.com/kucherenko/jscpd/compare/v3.5.7...v3.5.8) — 2023-05-01

### Bug Fixes

- Fixed the HTML reporter build script that was producing broken output.

---

## [3.5.7](https://github.com/kucherenko/jscpd/compare/v3.5.6...v3.5.7) — 2023-05-01

### Bug Fixes

- Fixed a crash that occurred when a path specified for HTML reporting did not exist.

---

## [3.5.6](https://github.com/kucherenko/jscpd/compare/v3.5.5...v3.5.6) — 2023-05-01

### Bug Fixes

- Fixed a missing-dependency error in the HTML reporter.

---

## [3.5.5](https://github.com/kucherenko/jscpd/compare/v3.5.4...v3.5.5) — 2023-04-27

### Maintenance

- Updated the `blamer` dependency to its latest version.

---

## [3.5.4](https://github.com/kucherenko/jscpd/compare/v3.5.3...v3.5.4) — 2023-03-24

### New Features

- **pre-commit hook support** — a `.pre-commit-hooks.yaml` file is now included so jscpd can be used as a [pre-commit](https://pre-commit.com/) hook with zero extra configuration.

---

## [3.5.3](https://github.com/kucherenko/jscpd/compare/v3.5.2...v3.5.3) — 2022-12-15

### Maintenance

- Upgraded the Vue.js version used by the HTML report viewer.

---

## [3.5.2](https://github.com/kucherenko/jscpd/compare/v3.5.1...v3.5.2) — 2022-10-24

### Bug Fixes

- Fixed incorrect HTML escaping in code snippets shown in reports.

---

## [3.5.1](https://github.com/kucherenko/jscpd/compare/v3.5.0...v3.5.1) — 2022-10-24

### New Features

- **Modern JS/TS module extensions** — jscpd now detects duplicates in `.mjs`, `.cjs`, `.mts`, and `.cts` files out of the box.

### Bug Fixes

- Ensure that ignore patterns specified in configuration files are respected even when not passed on the command line.

---

## [3.5.0](https://github.com/kucherenko/jscpd/compare/v3.4.5...v3.5.0) — 2022-10-01

### New Features

- **HTML reporter redesigned** — the HTML report has been rebuilt as a standalone page, removing the Vue.js SPA dependency and making it simpler to open and share.

### Bug Fixes

- Fixed symlink detection so symlinked files are correctly handled when `--noSymlinks` is set.
- Fixed HTML tag escaping in code blocks within the HTML report (rendering issues when code contained `<` / `>` characters).
- Dropped the unused constructor that was causing a minor overhead at startup.

---

## [3.4.5](https://github.com/kucherenko/jscpd/compare/v3.4.2...v3.4.5) — 2022-01-10

### Bug Fixes

- Pinned `colors` to v1.4.0 to avoid the intentionally broken `colors@1.4.1` release that caused console output corruption.

---

## [3.4.2](https://github.com/kucherenko/jscpd/compare/v3.4.1...v3.4.2) — 2021-11-06

### Bug Fixes

- Fixed the exit callback not being invoked when duplicates were detected.

---

## [3.4.0](https://github.com/kucherenko/jscpd/compare/v3.3.26...v3.4.0) — 2021-11-06

### New Features

- **`--exitCode` option** — you can now configure the exit code that jscpd returns when duplicates are found, making it easier to integrate into pipelines that use non-zero exit codes to signal failures.
- **`--ignore-pattern` option** — supply a glob or regex pattern to exclude matching code fragments from detection (closes [#435](https://github.com/kucherenko/jscpd/issues/435)).

---

## [3.3.26](https://github.com/kucherenko/jscpd/compare/v3.3.25...v3.3.26) — 2021-05-23

### Bug Fixes

- Silent mode is now truly silent — no output is produced when `--silent` is used.

### Security

- Bumped several transitive dependencies (`hosted-git-info`, `handlebars`, `url-parse`, `ssri`, `y18n`) to patched versions to close known vulnerabilities.

---

## [3.3.25](https://github.com/kucherenko/jscpd/compare/v3.3.24...v3.3.25) — 2021-03-04

### Maintenance

- Bumped `pug` to v3.0.1 (security fix).

---

## [3.3.24](https://github.com/kucherenko/jscpd/compare/v3.3.23...v3.3.24) — 2021-02-27

### Bug Fixes

- Fixed a tokenizer bug that caused incorrect source-location calculation.
- Fixed a crash when `calculateLocation()` received an empty array.

---

## [3.3.23](https://github.com/kucherenko/jscpd/compare/v3.3.22...v3.3.23) — 2020-12-13

### Bug Fixes

- Added TAP format support so jscpd can now detect copy-paste in TAP (Test Anything Protocol) files.
- Fixed a crash that occurred when an unsupported language was encountered instead of silently skipping it.

---

## [3.3.22](https://github.com/kucherenko/jscpd/compare/v3.3.21...v3.3.22) — 2020-12-01

### New Features

- **Badge reporter** — generates a jscpd shield badge (SVG/URL) showing your project's duplication percentage. Drop it straight into your README.

---

## [3.3.21](https://github.com/kucherenko/jscpd/compare/v3.3.20...v3.3.21) — 2020-11-20

### Bug Fixes

- Fixed a crash that occurred when the clone list was empty.

---

## [3.3.20](https://github.com/kucherenko/jscpd/compare/v3.3.19...v3.3.20) — 2020-11-20

### Bug Fixes

- Fixed a crash that occurred when the source list was empty.

---

## [3.3.19](https://github.com/kucherenko/jscpd/compare/v3.3.17...v3.3.19) — 2020-09-01

### Bug Fixes

- Fixed the coverage report output.
- Removed cyclic package dependencies that caused intermittent build failures.

---

## [3.3.17](https://github.com/kucherenko/jscpd/compare/v3.3.16...v3.3.17) — 2020-08-30

### New Features

- **CSV and Markdown reporters** — two new output formats for jscpd reports. Use `--reporters csv` or `--reporters markdown` to generate spreadsheet-friendly or documentation-ready output.
- **Duplicated lines and tokens in HTML report** — the HTML report now shows both the number of duplicated lines and the token count for each clone, giving you more context at a glance.
- **Ability to persist the detection store** — the store can now be saved between runs, enabling incremental analysis on large codebases.
- **Redis store** — an optional Redis-backed store (`@jscpd/redis-store`) is available for teams that want a shared, persistent store across multiple machines or CI agents.
- **New programmatic API** — `detectClones()` and related helpers are now properly exported, making it straightforward to embed jscpd in your own tooling.
- **Xcode reporter** — outputs results in the format Xcode's Issue Navigator understands, useful for Swift/Objective-C projects.
- **File-search glob pattern** — you can now pass a glob pattern to control which files are scanned.

### Bug Fixes

- Fixed a bug with empty files being processed incorrectly.
- Fixed filenames being escaped incorrectly in XML output.
- Fixed an empty token-map payload in event hooks.
- Fixed wrong exit codes in some edge cases.
- Fixed a SQL grammar tokenization issue.
- Fixed the path option not being resolved correctly.

---

## [3.3.14](https://github.com/kucherenko/jscpd/compare/v3.3.13...v3.3.14) — 2020-08-20

### New Features

- **Improved HTML reporter** — internal refactor to optimise language loading and tokeniser performance. The HTML report includes more detailed clone statistics.

---

## [3.3.1](https://github.com/kucherenko/jscpd/compare/v3.2.1...v3.3.1) — 2020-07-27

### New Features

- Migrated the project to a **monorepo** structure, splitting functionality into focused packages (`@jscpd/core`, `@jscpd/finder`, `@jscpd/html-reporter`, etc.).
- Added Node.js 14 to the CI matrix.

### Bug Fixes

- Fixed the HTML reporter producing broken output in some configurations.

---

## [3.2.1](https://github.com/kucherenko/jscpd/compare/v3.2.0...v3.2.1) — 2020-04-18

### Bug Fixes

- Used `fs-extra` v8.0.0 for compatibility with Node.js v8 (closes [#346](https://github.com/kucherenko/jscpd/issues/346), [#345](https://github.com/kucherenko/jscpd/issues/345)).

---

## [3.2.0](https://github.com/kucherenko/jscpd/compare/v3.1.0...v3.2.0) — 2020-04-08

### New Features

- **`--skipLocal` flag** — skip duplicates that exist only within the same folder, reducing noise in reports for projects that intentionally have similar files in isolated directories (closes [#326](https://github.com/kucherenko/jscpd/issues/326)).

### Bug Fixes

- Updated `cli-table3` to v0.6.0.
- Updated `fs-extra` to v9.0.0.

---

## [3.1.0](https://github.com/kucherenko/jscpd/compare/v3.0.1...v3.1.0) — 2020-03-11

### New Features

- **Plain-text file support** — jscpd can now detect duplicates in `.txt` files (closes [#272](https://github.com/kucherenko/jscpd/issues/272)).

---

## [3.0.1](https://github.com/kucherenko/jscpd/compare/v3.0.0...v3.0.1) — 2020-03-10

### Bug Fixes

- Fixed incorrect usage of the blamer module (closes [#238](https://github.com/kucherenko/jscpd/issues/238)).
- Updated `blamer` to v1.0.1.

---

## [3.0.0](https://github.com/kucherenko/jscpd/compare/v2.0.16...v3.0.0) — 2020-03-08

### Breaking Changes

- **XML reporter** — the CDATA format in the XML report has changed to fix a correctness issue (closes [#331](https://github.com/kucherenko/jscpd/issues/331)). Tools that parse the XML output may need updating.

### Bug Fixes

- Updated `commander` to v4.0.1.
- Updated `level` to v6.0.0.
- Fixed CDATA handling in the XML reporter.

### Changes

- Updated the CLI entry script for running jscpd.

---

## [2.0.16](https://github.com/kucherenko/jscpd/compare/v2.0.15...v2.0.16) — 2019-09-24

### Bug Fixes

- Updated several dependencies to close known security vulnerabilities (`commander`, `eventemitter3`, `fs-extra`, `rimraf`, `snyk`).
- Fixed a typo and a broken screenshot URL in the README.
- Fixed failing test snapshots.

---

## [2.0.15](https://github.com/kucherenko/jscpd/compare/v2.0.14...v2.0.15) — 2019-04-24

### Bug Fixes

- Updated `level` to v5.0.1.

---

## [2.0.14](https://github.com/kucherenko/jscpd/compare/v2.0.13...v2.0.14) — 2019-04-18

### Bug Fixes

- Fixed a crash in the Prism tokenizer caused by a language entry with an empty name (closes [#223](https://github.com/kucherenko/jscpd/issues/223)).

---

## [2.0.13](https://github.com/kucherenko/jscpd/compare/v2.0.12...v2.0.13) — 2019-03-29

### Bug Fixes

- Fixed empty-statistic display in the HTML reporter (closes [#214](https://github.com/kucherenko/jscpd/issues/214)).

---

## [2.0.4](https://github.com/kucherenko/jscpd/compare/v2.0.3...v2.0.4) — 2019-01-08

### Bug Fixes

- Split C/C++ and C/C++ header formats so that header files (`.h`, `.hpp`) are now tokenised separately from source files. This prevents spurious matches across file types (closes [#188](https://github.com/kucherenko/jscpd/issues/188)).

---

## [2.0.3](https://github.com/kucherenko/jscpd/compare/v2.0.2...v2.0.3) — 2019-01-08

### Bug Fixes

- Fixed a bug where duplicates within a single file were not detected correctly (closes [#189](https://github.com/kucherenko/jscpd/issues/189)).

---

## [2.0.2](https://github.com/kucherenko/jscpd/compare/v2.0.1...v2.0.2) — 2018-12-28

### Bug Fixes

- Replaced GPL-licensed packages with MIT-licensed equivalents.

---

## [2.0.1](https://github.com/kucherenko/jscpd/compare/v2.0.0...v2.0.1) — 2018-12-28

### Bug Fixes

- The `--threshold` option now accepts `0` as a valid value (closes [#182](https://github.com/kucherenko/jscpd/issues/182)).

---

## [2.0.0](https://github.com/kucherenko/jscpd/compare/v1.2.3...v2.0.0) — 2018-12-28

### Breaking Changes

- **Persistent store** — jscpd now uses [LevelDB](https://github.com/google/leveldb) as its default store to keep memory usage low on very large codebases. The in-memory store from v1.x is no longer the default (closes [#66](https://github.com/kucherenko/jscpd/issues/66), [#184](https://github.com/kucherenko/jscpd/issues/184)).

---

## [1.2.3](https://github.com/kucherenko/jscpd/compare/v1.2.2...v1.2.3) — 2018-12-27

### Bug Fixes

- Fixed a bug with files that use multiple format extensions (e.g. `.html.erb`).

---

## [1.2.1](https://github.com/kucherenko/jscpd/compare/v1.2.0...v1.2.1) — 2018-12-23

### Bug Fixes

- Fixed an unhandled promise rejection in the blamer module (closes [#185](https://github.com/kucherenko/jscpd/issues/185)).

---

## [1.2.0](https://github.com/kucherenko/jscpd/compare/v1.1.0...v1.2.0) — 2018-12-14

### New Features

- **Graph view in HTML report** — the HTML report now includes an interactive graph showing clone relationships between files.

### Bug Fixes

- Fixed empty lines being rendered incorrectly in HTML code blocks.

---

## [1.1.0](https://github.com/kucherenko/jscpd/compare/v1.0.3...v1.1.0) — 2018-12-02

### New Features

- **Blamed lines in reports** — the `html` and `consoleFull` reporters now show Git blame information alongside duplicate code, so you know who introduced each clone and when.
- **Syntax highlighting** in the HTML reporter.
- **Custom mode** — a new `custom` detection mode that lets you tune detection behaviour beyond the built-in `strict` and `weak` presets (closes [#172](https://github.com/kucherenko/jscpd/issues/172)).

---

## [1.0.3](https://github.com/kucherenko/jscpd/compare/v1.0.2...v1.0.3) — 2018-11-27

### Bug Fixes

- Fixed the `--path` option not being applied correctly (closes [#177](https://github.com/kucherenko/jscpd/issues/177)).

---

## [1.0.2](https://github.com/kucherenko/jscpd/compare/v1.0.1...v1.0.2) — 2018-11-27

### Bug Fixes

- Added support for locally installed reporters and modes (installed in the project's `node_modules` rather than globally).

---

## [1.0.1](https://github.com/kucherenko/jscpd/compare/v1.0.0...v1.0.1) — 2018-11-27

### Bug Fixes

- Added support for trailing-slash patterns in `.gitignore`-style ignore files.

---

## [1.0.0](https://github.com/kucherenko/jscpd/compare/v1.0.0-rc.6...v1.0.0) — 2018-11-21

First stable release of the fully rewritten jscpd. The tool was migrated from CoffeeScript to TypeScript, the tokenizer was redesigned from scratch, and a new pluggable reporter system was introduced.

---

## Earlier Pre-releases (1.0.0-rc.x, 1.0.0-alpha.x) — 2018

These releases established the current architecture during active development:

- **1.0.0-rc.6** — HTML reporter added.
- **1.0.0-rc.4** — CLI supports multiple path arguments; hooks system introduced; reporter interface redesigned.
- **1.0.0-rc.1** — Execution-timer reporter added.
- **1.0.0-alpha.2** — Configuration file name finalised.
- **1.0.0-alpha.1** — CLI binary script added.
- **1.0.0-alpha.0** — Initial TypeScript rewrite: new tokenizer, XML/JSON/statistic/threshold/silent reporters, YAML language support, cache for detection results, and a `--debug` option.
