# Changelog

All notable changes to **cpd (Rust)** are documented here. Releases follow [Semantic Versioning](https://semver.org).

---

## 5.0.10

### Bug Fixes

- Emit scan-root-relative paths in all reporters when `absolute: false` (or the default). Previously, `jscpd /abs/path` from a different CWD left absolute paths in SARIF/JSON/XML/HTML/CSV/Markdown/console output, and Windows/macOS path canonicalization could leave `\\?\` or `./` prefixes. Paths are now normalized against the canonicalized scan root (with CWD fallback) and stripped of any leading `./` or `.\\` component. Fixes [#827](https://github.com/kucherenko/jscpd/issues/827)
- Fix `--skip-local` to match jscpd v4 TypeScript semantics: it now filters clones where both fragments are under the same scan root, instead of only skipping clones in the same parent directory

### Refactoring

- DRY duplication in reporters: extract shared helpers (`print_clone_header`, `print_clone_locations`, `print_snippet`, `write_report_file`, report statistics, test fixtures, etc.) into `cpd-reporter/src/shared.rs`. Console, console-full, CSV, JSON, HTML, Markdown, silent, XML, and SARIF reporters now reuse the same implementation, reducing the monorepo's reported duplication ratio from 5.0% to 0.56% and fixing a latent `--absolute` path relativization bug in the same pass
- Move blame enrichment from `gitoxide` to `git blame --porcelain`; capture elapsed time after blame so timing includes blame work
- Resolve `needless_borrow` clippy warnings in CSV and Markdown reporters

### Documentation

- Add Nix and Homebrew install instructions to Rust READMEs. [#818](https://github.com/kucherenko/jscpd/issues/818)
- Update project homepage URLs to `https://jscpd.dev` in all `Cargo.toml` and npm `package.json` files, add curl install method to READMEs, clean up outdated badges
- Remove defunct Universal Analytics tracking pixels from all READMEs

---

## 5.0.9

### New Features

- GitHub Action for jscpd (Rust v5) — `jscpd-copy-paste-detector` action for GitHub Actions Marketplace. Scan your repo for copy/paste in CI with `uses: kucherenko/jscpd/.github/workflows/action.yml@v5`

### Bug Fixes

- Resolve platform binary resolution when `cpd` is installed as a nested dependency (e.g. in a project's `node_modules` via a parent package). The runner now correctly locates the platform-specific binary relative to the installed package rather than assuming a top-level install. Fixes [#816](https://github.com/kucherenko/jscpd/issues/816)

---

## 5.0.8

### Bug Fixes

- Prevent mmap exhaustion crashes when scanning repositories with more files than `vm.max_map_count` (default 131 072 on Linux). The walker previously held a live `Mmap` per discovered file; each rayon worker now opens and drops its mapping within the processing closure, capping concurrent mappings to the thread-pool size (typically 8–32). Fixes [#813](https://github.com/kucherenko/jscpd/issues/813)
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