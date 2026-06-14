# Benchmark Tools

This directory contains all third-party tools required by the benchmark script. Install each tool by following the instructions below, then run `./benchmark.sh` from the parent directory.

## Quick Install

```bash
cd tools/
npm install
```

This installs **jscpd-rs** and **Fallow** (the only tools distributed via npm). The remaining tools must be installed manually.

## Tools

### jscpd@5 (local build)

The jscpd v5 Rust binary, built from the `rust/` directory in this repository.

```bash
cd ../../rust/
cargo build --release
```

The binary is expected at `../../rust/target/release/jscpd`.

### jscpd@4 (Node.js)

The jscpd v4 TypeScript CLI, built from the `apps/jscpd/` directory in this repository.

```bash
cd ../../apps/jscpd/
npm install
npm run build
```

The binary is expected at `../../apps/jscpd/bin/jscpd`.

### jscpd-rs (npm)

Installed in `tools/node_modules/` via `npm install`. The npm package downloads a prebuilt Rust binary for the current platform.

```bash
npm install
```

Binary: `node_modules/.bin/jscpd-rs`

### Fallow dupes

Installed in `tools/node_modules/` via `npm install`. Fallow is a Rust-based code intelligence tool; the `dupes` subcommand performs structural clone detection for TypeScript/JavaScript projects.

```bash
npm install
```

Binary: `node_modules/.bin/fallow`

> **Note:** Fallow must run from inside the target directory (it has no path argument). The benchmark script `cd`s into the fixtures directory before invoking it.

### PMD CPD

Install via Homebrew (macOS) or download from [pmd.github.io](https://pmd.github.io).

```bash
brew install pmd
```

PMD requires a Java runtime (8+). The `pmd` command must be on `$PATH`.

> **Note:** The benchmark runs PMD CPD for all 34 supported languages individually and sums the results. This takes ~30 seconds.

### Duplo

A prebuilt macOS arm64 binary is included at `tools/duplo`. To replace it:

1. Download from [github.com/dlidstrom/Duplo/releases](https://github.com/dlidstrom/Duplo/releases)
2. Place the binary at `tools/duplo`
3. `chmod +x tools/duplo`

```bash
# Verify
./duplo -help
```

### Simian

A prebuilt JAR is included at `tools/simian.jar`. To replace it:

1. Download from [simian.quandarypeak.com](https://simian.quandarypeak.com)
2. Place the JAR at `tools/simian.jar`

Requires a Java runtime:

```bash
java -jar tools/simian.jar -help
```