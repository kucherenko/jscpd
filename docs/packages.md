# Packages

A jscpd release is one Rust workspace published under several names. Everything lives under [`rust/`](../rust); `rust/scripts/sync-version.mjs` keeps the versions below in step (the engine version comes from `rust/package.json`).

## Crates (crates.io)

### jscpd

**Path:** `rust/crates/cpd`
**crates.io:** [`jscpd`](https://crates.io/crates/jscpd)
**Version:** 5.1.2

The CLI. `cargo install jscpd` installs two identical binaries, `jscpd` and `cpd`. Its library target is internal (test helpers only); depend on the crates below for programmatic use. See [Rust docs](./rust.md).

### cpd-core

**Path:** `rust/crates/cpd-core`
**crates.io:** [`cpd-core`](https://crates.io/crates/cpd-core)
**Version:** 0.1.11

Core data models and the Rabin-Karp rolling hash implementation.

### cpd-tokenizer

**Path:** `rust/crates/cpd-tokenizer`
**crates.io:** [`cpd-tokenizer`](https://crates.io/crates/cpd-tokenizer)
**Version:** 0.1.13

Source code tokenizer (224 formats, listed in [FORMATS.md](../FORMATS.md)). Uses `oxc_parser` for JavaScript/TypeScript/JSX and per-block tokenization for Vue SFC, Svelte, Astro, and Markdown. Pure — no filesystem or network access (enforced in CI).

### cpd-finder

**Path:** `rust/crates/cpd-finder`
**crates.io:** [`cpd-finder`](https://crates.io/crates/cpd-finder)
**Version:** 0.1.14

File walking, orchestration, baseline handling, and git blame. Uses `rayon` for parallelism, `ignore` + `globset` for file matching. The entry point for the [Rust API](./api.md).

### cpd-reporter

**Path:** `rust/crates/cpd-reporter`
**crates.io:** [`cpd-reporter`](https://crates.io/crates/cpd-reporter)
**Version:** 0.1.12

Output format rendering for the 15 reporters.

## npm packages

All npm packages share the engine version (5.1.2). None of them needs a Node.js runtime to run jscpd — Node.js is only the delivery mechanism.

### jscpd

**Path:** `rust/jscpd`
**npm:** [`jscpd`](https://www.npmjs.com/package/jscpd)

Installs the `jscpd` command. A thin launcher that resolves the platform package below for the current OS/CPU and executes the binary.

### cpd

**Path:** `rust`
**npm:** [`cpd`](https://www.npmjs.com/package/cpd)

Installs the `cpd` command. Same launcher, same binary, shorter name.

### Platform packages

**Path:** `rust/npm/<package>`

Each contains one prebuilt binary and is pulled in as an optional dependency of `jscpd` and `cpd`; npm installs only the one matching the host.

| Package | Rust target |
|---------|-------------|
| `jscpd-darwin-arm64` | `aarch64-apple-darwin` |
| `jscpd-darwin-x64` | `x86_64-apple-darwin` |
| `jscpd-linux-arm64-gnu` | `aarch64-unknown-linux-gnu` |
| `jscpd-linux-arm64-musl` | `aarch64-unknown-linux-musl` |
| `jscpd-linux-x64-gnu` | `x86_64-unknown-linux-gnu` |
| `jscpd-linux-x64-musl` | `x86_64-unknown-linux-musl` |
| `jscpd-windows-arm64-msvc` | `aarch64-pc-windows-msvc` |
| `jscpd-windows-x64-msvc` | `x86_64-pc-windows-msvc` |

The same binaries are attached to every [GitHub Release](https://github.com/kucherenko/jscpd/releases) as `jscpd-<platform>.tar.gz` with checksums, Sigstore signatures and SLSA provenance, and packaged into the `ghcr.io/kucherenko/jscpd` Docker image.

## Other distribution channels

| Channel | Source |
|---------|--------|
| GitHub Action `kucherenko/jscpd@v5` | [`action.yml`](../action.yml) |
| Docker `ghcr.io/kucherenko/jscpd` | [`Dockerfile`](../Dockerfile) |
| Nix `github:kucherenko/jscpd` | [`flake.nix`](../flake.nix) |
| Homebrew `brew install jscpd` | homebrew-core formula |
| `install.sh` / `install.ps1` | https://jscpd.dev |

## jscpd v4 packages

The TypeScript packages (`jscpd@4`, `jscpd-server`, `@jscpd/core`, `@jscpd/finder`, `@jscpd/tokenizer`, `@jscpd/html-reporter`, `@jscpd/badge-reporter`, `jscpd-sarif-reporter`, `@jscpd/leveldb-store`, `@jscpd/redis-store`) are maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch.
