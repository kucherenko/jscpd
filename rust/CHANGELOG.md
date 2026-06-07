# Changelog

All notable changes to **cpd (Rust)** are documented here. Releases follow [Semantic Versioning](https://semver.org/).

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