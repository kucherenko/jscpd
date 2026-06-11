# Changelog

All notable changes to **cpd (Rust)** are documented here. Releases follow [Semantic Versioning](https://semver.org).

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