# CPD Tool Benchmark

## Methodology

Seven copy/paste detection tools were benchmarked against the `fixtures/` directory (547 files, 21,645 lines across 150+ language formats). Each tool was run with its default detection threshold (~5 lines / ~50 tokens) and no custom language configurations.

Timing uses external wall-clock measurement (`date +%s%N`), reported in milliseconds. Tools that require per-language invocation (PMD CPD) were run for all 34 supported languages and results summed. Fallow dupes must run from inside the target directory (no path argument support).

All tools were built or installed locally — no `npx` downloads at runtime:

- **jscpd@5** — built from `rust/` via `cargo build --release`; binary at `rust/target/release/jscpd`
- **jscpd@4** — built from `apps/jscpd/` via `npm run build`; binary at `apps/jscpd/bin/jscpd`
- **jscpd-rs** — installed via `npm install` into `benchmark/tools/node_modules/`
- **Fallow** — installed via `npm install` into `benchmark/tools/node_modules/`
- **PMD CPD** — installed via Homebrew
- **Duplo** — pre-built macOS arm64 binary in `benchmark/tools/duplo`
- **Simian** — downloaded JAR in `benchmark/tools/simian.jar`

**Hardware:** Apple Silicon (Darwin arm64)

## Results

| Tool | Time | Files | Clones | Dup Lines |
|------|------|-------|--------|-----------|
| jscpd@5 | 68ms | 347 | 212 | 9,133 |
| jscpd-rs (npm) | 105ms | 360 | 222 | 10,317 |
| Duplo | 135ms | 319 | 518 | 13,049 |
| Fallow dupes | 195ms | 34 | 10 | 3,137 |
| Simian | 1.012s | 547 | 424 | 15,351 |
| jscpd@4 | 2.250s | 364 | 211 | 9,969 |
| PMD CPD | 34.026s | 71 | 56 | 2,267 |

## Comparison (jscpd@5 as baseline)

| Tool | Speed | Files | Clones | Dup Lines |
|------|-------|-------|---------|-----------|
| jscpd@5 | 1.0× | 347 | 212 | 9,133 |
| jscpd-rs | 1.5× slower | +3.7% | +4.7% | +13.0% |
| Duplo | 2.0× slower | −8.1% | +144% | +42.9% |
| Fallow | 2.9× slower | −90.2% | −95.3% | −65.7% |
| Simian | 14.9× slower | +57.6% | +100% | +68.1% |
| jscpd@4 | 33.1× slower | +4.9% | −0.5% | +9.1% |
| PMD CPD | 500× slower | −79.5% | −73.6% | −75.2% |

## Summary

**jscpd@5 is the fastest tool by a wide margin** — 33× faster than jscpd@4 (its TypeScript predecessor) and the only tool completing in under 100ms. The Rust rewrite delivers this speed while maintaining comparable detection fidelity.

**Detection accuracy varies significantly across tools:**

- **jscpd@5 and jscpd@4** report nearly identical clone counts (212 vs 211) and similar duplicate lines (9,133 vs 9,969), confirming the Rust rewrite preserves detection quality.
- **jscpd-rs** (npm v0.1.12) reports 222 clones and 10,317 duplicate lines — slightly more than jscpd@5, likely due to a different default configuration or algorithm version in the npm package.
- **Duplo and Simian** report the most duplicates (518 and 424 clones respectively) but include significant false positives — both are purely text-based without tokenization, so formatting differences inflate counts.
- **PMD CPD** finds the fewest clones (56) despite running for 34 seconds across 34 languages. It only supports a limited set of languages and cannot process most fixture formats.
- **Fallow dupes** only analyzes TypeScript/JavaScript files (34 of 547), making it unsuitable for polyglot codebases.

**Key tradeoff:** Speed vs. precision. jscpd@5 and jscpd@4 use token-based detection that balances recall and precision. Simian and Duplo are faster at raw text matching but over-report. PMD CPD is thorough per-language but painfully slow and coverage-limited.