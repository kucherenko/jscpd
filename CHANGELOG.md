# Changelog

All notable changes to **jscpd** are documented here. Releases follow [Semantic Versioning](https://semver.org/).

---

## 5.1.0

### New Features

- **Clone baseline (`--baseline`, `--update-baseline`, `--fail-on-new-clones`)** — gate CI on *new* duplication only: a committed baseline file records fingerprints of accepted clones, and only clones absent from it fail the build, so legacy duplication is tolerated while regressions are caught. ([#944](https://github.com/kucherenko/jscpd/issues/944))
- **Ephemeral baseline from a git ref (`--baseline-from-ref`)** — stateless PR gating without a committed file: scans the base ref in a temporary worktree and reports clones that don't exist there. ([#944](https://github.com/kucherenko/jscpd/issues/944))
- **OpenMetrics reporter (`--reporters openmetrics`)** — duplication gauges in the OpenMetrics text format, ready for GitLab CI `artifacts:reports:metrics`. ([#422](https://github.com/kucherenko/jscpd/issues/422))
- **CodeClimate / GitLab Code Quality reporter (`--reporters codeclimate`, alias `gitlab`)** — `gl-code-quality-report.json` for GitLab's `artifacts:reports:codequality`, so duplicates appear as code quality issues in merge requests. ([#958](https://github.com/kucherenko/jscpd/issues/958))
- **Windows on ARM support** — npm installs pick a native `aarch64-pc-windows-msvc` binary on Windows ARM64. ([#963](https://github.com/kucherenko/jscpd/pull/963))
- **Config discovery in `.config/`** — jscpd also finds its config at `.config/jscpd.json` per the [dot-config convention](https://dot-config.github.io/); a root `.jscpd.json` still wins. ([#979](https://github.com/kucherenko/jscpd/issues/979))

### Bug Fixes

- **Unknown `--format` values warn instead of silently scanning 0 files** — `--format cs` (or any typo) no longer looks identical to a clean scan in CI. ([#964](https://github.com/kucherenko/jscpd/issues/964))
- **Nix flake builds again** — the Rust toolchain is pinned to an exact patch version, so the manifest hash can no longer drift when a new patch release ships. ([#976](https://github.com/kucherenko/jscpd/issues/976))
- **Windows: `--baseline-from-ref` no longer reports every clone as new** — Windows verbatim paths (`\\?\C:\...`) were truncated by the `:format`-suffix stripper, silently breaking every snippet read behind the base scan's fingerprints. Clone fingerprints are also line-ending agnostic now, so committed baselines survive CRLF/LF differences between platforms.

### Thank You ❤️

- [@luchsamapparat](https://github.com/luchsamapparat) for contributing Windows on ARM support ([#963](https://github.com/kucherenko/jscpd/pull/963))
- [@dmromanov](https://github.com/dmromanov) for proposing the OpenMetrics reporter ([#422](https://github.com/kucherenko/jscpd/issues/422))
- [@beanaroo](https://github.com/beanaroo) for proposing the GitLab / CodeClimate Code Quality report format ([#958](https://github.com/kucherenko/jscpd/issues/958))
- [@MRDGH2821](https://github.com/MRDGH2821) for proposing config discovery from the `.config/` subfolder ([#979](https://github.com/kucherenko/jscpd/issues/979))
- [@zbcoding](https://github.com/zbcoding) for reporting the silent unknown-`--format` behavior ([#964](https://github.com/kucherenko/jscpd/issues/964))
- [@eaves-dropper](https://github.com/eaves-dropper) for reporting the Nix build failure ([#976](https://github.com/kucherenko/jscpd/issues/976))

---

## 5.0.16

### New Features

- **MCP server over stdio (`--mcp`)** — `cpd --mcp /path/to/project` serves the Model Context Protocol on stdin/stdout; the project is scanned once at startup and kept in memory, so `check_duplication` snippet checks answer in milliseconds. Tools: `check_duplication`, `get_file_clones`, `get_statistics`, `check_current_directory`. ([#891](https://github.com/kucherenko/jscpd/issues/891))
- **Codebase summary (`--summary`)** — opt-in refactoring-hotspot overview: top files and folders ranked by tokens, lines, size, or complexity, with each file's duplication share; `--summary-top` and `--summary-by` tune it. ([#934](https://github.com/kucherenko/jscpd/issues/934))
- **Isolated folder groups (`--skip-isolated`)** — skip duplication between monorepo folders owned by different teams (`--skip-isolated "packages/a|packages/b"`); clones inside one folder or against shared code are still reported. Config file: `"skipIsolated": [["packages/a", "packages/b"]]`. ([#628](https://github.com/kucherenko/jscpd/pull/628), [#942](https://github.com/kucherenko/jscpd/pull/942))

### Security

- **Supply-chain hardening (OpenSSF Scorecard)** — GitHub Actions pinned to commit SHAs, least-privilege workflow tokens, `SECURITY.md` with private vulnerability reporting, protected `master` branch

### Bug Fixes

- **GitHub "Latest" release badge stays on v5** — legacy v4 releases can no longer take the Latest badge from the v5 line

### Thank You ❤️

- [@hanzhangyu](https://github.com/hanzhangyu) for proposing isolated folder groups for monorepos and contributing the original `skipIsolated` implementation ([#628](https://github.com/kucherenko/jscpd/pull/628)), which this release ports to the Rust engine

---

## 5.0.15

### New Features

- **SARIF: size-based severity** — new `--sarif-error-tokens <N>` flag (also `sarifErrorTokens` in `.jscpd.json`): clones with at least N tokens are reported at level `error` while smaller ones stay `warning`. When overall duplication exceeds `--threshold`, **all** SARIF results are emitted as `error`. ([#908](https://github.com/kucherenko/jscpd/issues/908))
- **SARIF: clone fingerprints** — each result carries `token_count`, a `clone_hash`, and a `partialFingerprints` entry (`jscpdCloneHash/v1`) for cross-run result identity in consumers like GitHub code scanning. ([#909](https://github.com/kucherenko/jscpd/issues/909))
- **SARIF: related-location messages** — the duplicate's counterpart location now has a message linked from the primary message, so GitHub code scanning displays it. ([#911](https://github.com/kucherenko/jscpd/issues/911))
- **SARIF: richer rule metadata** — display name, full description, default configuration, and quality tags on the `jscpd/duplicate-code` rule. ([#914](https://github.com/kucherenko/jscpd/pull/914))

### Bug Fixes

- **Scan-root-relative report paths** — report paths are relative to the scanned directory again (as in 4.x) while reporters can still resolve source files; fixes empty snippets and unresolvable paths when scanning from outside the target directory, including multi-root scans. ([#872](https://github.com/kucherenko/jscpd/issues/872), [#892](https://github.com/kucherenko/jscpd/issues/892))
- **Report version stamping** — SARIF `tool.driver.version` and the HTML report version now match `--version`. ([#915](https://github.com/kucherenko/jscpd/issues/915))
- **Multi-root blame attribution** — git blame data is keyed by resolved path, so a second scan root no longer inherits the first root's authors
- **Git root discovery** — walking up from a relative scan path no longer terminates before reaching the repository root

### Thank You ❤️

- [@chrisc-onaorg](https://github.com/chrisc-onaorg) for the SARIF fingerprints, related-location messages, and rule metadata ([#910](https://github.com/kucherenko/jscpd/pull/910), [#912](https://github.com/kucherenko/jscpd/pull/912), [#914](https://github.com/kucherenko/jscpd/pull/914))
- [@darronz](https://github.com/darronz) for the scan-root-relative paths fix ([#913](https://github.com/kucherenko/jscpd/pull/913))
- [@nvuillam](https://github.com/nvuillam) for proposing size-based SARIF severity ([#908](https://github.com/kucherenko/jscpd/issues/908))

---

## 5.0.14

### New Features

- `--cross-formats` — detect clones across related formats via format equivalence groups sharing one comparison pool, e.g. `--cross-formats "javascript,typescript"` or the `js-ts` preset. When a group mixes TypeScript with JavaScript, TS files are compared with erasable type syntax stripped. Also configurable as `crossFormats` in `.jscpd.json` / `package.json`. ([#810](https://github.com/kucherenko/jscpd/issues/810))

### Bug Fixes

- **Prose-only Markdown files are now analyzed** — `.md` files without code fences previously produced zero tokens and were silently skipped; prose is now tokenized, while embedded code fences keep their own sub-format pools. ([#883](https://github.com/kucherenko/jscpd/issues/883))

---

## 5.0.13

npm-only release: republished the `cpd` package so its `optionalDependencies` point at the 5.0.12 platform binaries. No code changes.

---

## 5.0.12

### Dependencies

- Rust dependency updates (`askama` 0.16.0, `log` 0.4.33, `env_logger` 0.11.11, `rustc-hash` 2.1.3)

---

## 5.0.11

### New Features

- **Razor (.razor) support** — new tokenizer for Razor files in the Rust backend (thanks to [@chrisc-onaorg](https://github.com/chrisc-onaorg) in [#829](https://github.com/kucherenko/jscpd/pull/829))

### Dependencies

- `cpd-core` bumped to 0.1.6, `cpd-tokenizer` bumped to 0.1.7

---

## 5.0.10

### Bug Fixes

- Emit scan-root-relative paths in all reporters when `absolute: false`. Fixes [#827](https://github.com/kucherenko/jscpd/issues/827)
- Fix `--skip-local` to match jscpd v4 TypeScript semantics

### Refactoring

- DRY duplication in reporters: extract shared helpers into `cpd-reporter/src/shared.rs`
- Move blame enrichment from `gitoxide` to `git blame --porcelain`

---

## 5.0.9

### New Features

- GitHub Action for jscpd (Rust v5) — `jscpd-copy-paste-detector` action for GitHub Actions Marketplace. Scan your repo for copy/paste in CI with `uses: kucherenko/jscpd/.github/workflows/action.yml@v5`

### Bug Fixes

- Resolve platform binary resolution when `cpd` is installed as a nested dependency (e.g. in a project's `node_modules` via a parent package). The runner now correctly locates the platform-specific binary relative to the installed package rather than assuming a top-level install. Fixes [#816](https://github.com/kucherenko/jscpd/issues/816)

---

## 5.0.8

### Bug Fixes

- Prevent mmap exhaustion crashes when scanning repositories with more files than `vm.max_map_count` (default 131 072 on Linux). The walker previously held a live `Mmap` per discovered file; each rayon worker now opens and drops its mapping within the processing closure, capping concurrent mappings to the thread-pool size (typically 8–32). Fixes [#813](https://github.com/kucherenko/jscpd/issues/813)
- Fix `--pattern` not matching relative paths when the scan root is absolute (e.g. CWD). Patterns like `src/**/*.ts` now match correctly by comparing against both the relative path and the full absolute path, and bare patterns like `*.ts` gain a `**/` prefix to match at any depth. Fixes [#811](https://github.com/kucherenko/jscpd/issues/811)
- Fix trailing-newline off-by-one in line-count filter: files not ending with `\n` now count the final line correctly

---

## 5.0.7

### Bug Fixes

- Prevent stack overflow when scanning directories containing deeply-nested JS/TS files (e.g. Bun's `test/bundler` with 320K+ nested for-loops). OXC's recursive-descent parser allocates one stack frame per AST nesting level; pathological inputs now exceed the default 8 MiB thread stack. Fixed by building a local rayon `ThreadPool` with 64 MiB stacks instead of using the global pool (which silently fails on re-init)
- Default `--max-size` to `1mb` — files exceeding the limit are skipped at walk time, consistent with jscpd v4's `maxSize` behavior. This prevents OXC from ever seeing megabyte-scale generated files that would overflow the stack
- `--workers N` now correctly takes effect on every `run()` call (previously `build_global()` silently no-op'd after the first invocation)

---

## 5.0.6

### New Features

- v4 config backward compatibility — `.jscpd.json` fields `path`, `pattern`, `ignore`, and `ignorePattern` are now read and applied, matching jscpd v4 behavior
- `ignore` and `ignorePattern` are now distinct: `ignore` matches file-level globs, `ignorePattern` matches code-level regex patterns (previously conflated)
- `.jscpd.json` path config support — reads scan directories from the `path` field, resolving relative paths against the config file's directory
- `jscpd` npm wrapper package — publishes the same Rust binary under the `jscpd` name on npm with v5.x versioning
- `--exit-code` now matches v4 behavior: accepts optional integer value (`--exit-code` exits 1, `--exit-code 2` exits 2); `--threshold` and `--exit-code` are now independent
- Performance improvements: memory-mapped file I/O (via `memmap2`) eliminates heap copies of file contents; SIMD-accelerated line counting (via `memchr`); parallel detection pipeline uses `flat_map` to avoid intermediate allocations; JS tokenizer no longer clones source strings before parsing (thanks to [@auterium](https://github.com/auterium), [#808](https://github.com/kucherenko/jscpd/pull/808))

### Bug Fixes

- Fixed `--exit-code` to match jscpd v4's `--exitCode` behavior (was boolean, now optional integer)
- Fixed unique temp dir generation in reporter tests (added PID to prevent race conditions under parallel test runners)

---

## 5.0.4

### New Features

- CLI alignment with jscpd v4: new `--absolute`, `--ignore-case`, `--formats-exts`, `--formats-names` flags; fixed `--threshold`, improved `--max-size`
- Detection and statistics aligned with jscpd for consistent output across Rust and TypeScript versions
- Side-by-side blame comparison in console-full reporter
- Clone list display in console reporter

### Bug Fixes

- HTML reporter now outputs `jscpd-report.html` at the `output_dir` root
- Resolved all clippy warnings across workspace
- Fixed unique temp dir generation in tests (use `as_nanos()` instead of `subsec_nanos()`)

---

## 5.0.3

### New Features

- Rust-based cpd CLI with full feature parity to TypeScript jscpd
- Cross-platform binary distribution via npm platform packages (linux-x64-gnu, linux-arm64-gnu, linux-x64-musl, darwin-arm64, darwin-x64, windows-x64-msvc)
- 13 reporters: json, console, xml, csv, html, markdown, sarif, ai, badge, xcode, threshold, silent, console-full
- Time reporter for execution timing
- CLI short-form aliases matching TypeScript jscpd conventions
- ReportContext data structure for extensible reporter signatures
- Trusted Publishing support for crates.io via OIDC

---

## 5.0.2

### Bug Fixes

- Fixed Vue SFC tokenization to dispatch each block to its own sub-format
- Fixed entire-file duplicates silently dropped by RabinKarp store flush logic
- Fixed ReDoS hang on Lisp/Elisp files
- Fixed crash on malformed package.json when reading config

---

## 5.0.1

### New Features

- Initial Rust workspace with cpd-core, cpd-tokenizer, cpd-finder, cpd-reporter, and jscpd crates
- Cross-format detection for Vue SFC, Svelte, Astro, and Markdown files
- Shebang detection for extensionless scripts

---

## 5.0.0

### Breaking Changes

- First stable Rust release — replaces the TypeScript-based CLI with a native binary
- Reporter trait signature changed to use ReportContext instead of Statistics directly

---

## 4.3.0 — 2026-08-13

### New Features

- **Color auto-detection** — ANSI colors are disabled automatically when stdout is not a TTY (piped or redirected output), with new `--colors` / `--no-colors` flags and a `colors` config key to override. Precedence: explicit flag/config → `FORCE_COLOR` → `NO_COLOR` → TTY detection. The statistics table is covered too. ([#893](https://github.com/kucherenko/jscpd/issues/893), [#899](https://github.com/kucherenko/jscpd/pull/899))
- **jscpd-server: MCP protocol revision 2026-07-28** — migrated to the official MCP SDK v2 with the stateless 2026-07-28 revision and a legacy fallback for 2025-era clients, DNS-rebinding protection (Origin/Host allowlists with `--allowed-origin`/`--allowed-host`) on both `/mcp` and the REST API, a loopback default bind (`127.0.0.1`), and a hardened start/stop lifecycle. ([#902](https://github.com/kucherenko/jscpd/pull/902))

### Bug Fixes

- **consoleFull no longer prints clones twice** — the progress announcer is skipped for reporters that print every clone themselves (`ai`, `consoleFull`); jscpd-server shares the same wiring. ([#900](https://github.com/kucherenko/jscpd/pull/900))
- **Inclusive source line counts** — fixes off-by-one line counts in statistics and reports. ([#881](https://github.com/kucherenko/jscpd/pull/881))
- **jscpd-server log colors** — server output now respects `NO_COLOR`/`FORCE_COLOR` and TTY detection like the CLI.

### Security

- Resolved all 27 open Dependabot alerts on transitive dependencies (fast-uri, hono, js-yaml, brace-expansion, nanoid, postcss, ip-address, body-parser and others) by restoring the pnpm override mechanism — overrides now live in `pnpm-workspace.yaml`, where pnpm 10 actually reads them. CI installs use `--frozen-lockfile` so lockfile drift fails loudly.

### Thank You ❤️

This release was shaped by community contributions — huge thanks to:

- [@suzunn](https://github.com/suzunn) for the color auto-detection ([#899](https://github.com/kucherenko/jscpd/pull/899)), the consoleFull double-print fix ([#900](https://github.com/kucherenko/jscpd/pull/900)), and the lockfile repair ([#901](https://github.com/kucherenko/jscpd/pull/901))
- [@anxkhn](https://github.com/anxkhn) for the MCP 2026-07-28 server migration and its security hardening ([#902](https://github.com/kucherenko/jscpd/pull/902))
- [@9904099](https://github.com/9904099) for the inclusive line-count fix ([#881](https://github.com/kucherenko/jscpd/pull/881)) and repository metadata cleanups
- [@kfstorm](https://github.com/kfstorm) for reporting the non-TTY color issue ([#893](https://github.com/kucherenko/jscpd/issues/893)) and [@maxpatiiuk](https://github.com/maxpatiiuk) for the consoleFull improvement request ([#652](https://github.com/kucherenko/jscpd/issues/652))

---

## 4.2.5 — 2026-06-07

### Bug Fixes

- **JSON reporter duplicate token counts** — `tokens` was always reported as `0` in JSON output; now computed from token positions (`end.position - start.position`) (#801).
- **Gitignore parent-directory walk** — `.gitignore` files in parent directories up to the repo root are now read and combined with scan-directory `.gitignore` files. Also reads `.git/info/exclude` and the global `core.excludesFile` for full parity with Git's ignore resolution (#741).
- **Commander v15 migration** — CLI option parsing migrated from direct property access (`cli.minTokens`, etc.) to the `cli.opts()` API required by Commander v8+. The `--no-gitignore` / `--gitignore` flag handling was rewritten to use Commander's native negation support instead of `rawArgs` inspection.
- **Vitest 4.1.0** — bumped from 3.2.4 to address CVE-2026-47429.
- **Commander v15** — bumped from v5 to v15, enabling modern Node.js compatibility.
- **Pug 3.0.4, node-sarif-builder 4.1.0, nodemon 3.1.14** — dependency bumps for security and compatibility.

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
