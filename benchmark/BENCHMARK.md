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
| jscpd@5 | 84ms | 347 | 212 | 9,133 |
| jscpd-rs (npm) | 111ms | 360 | 222 | 10,317 |
| Duplo | 162ms | 319 | 518 | 13,049 |
| Fallow dupes | 164ms | 34 | 10 | 3,137 |
| Simian | 964ms | 547 | 424 | 15,351 |
| jscpd@4 | 2.680s | 364 | 211 | 9,969 |
| PMD CPD | 35.980s | 71 | 56 | 2,267 |

## Comparison (jscpd@5 as baseline)

| Tool | Speed | Files | Clones | Dup Lines |
|------|-------|-------|---------|-----------|
| jscpd@5 | 1.0× | 347 | 212 | 9,133 |
| jscpd-rs | 1.3× slower | +3.7% | +4.7% | +13.0% |
| Duplo | 1.9× slower | −8.1% | +144% | +42.9% |
| Fallow | 2.0× slower | −90.2% | −95.3% | −65.7% |
| Simian | 11.5× slower | +57.6% | +100% | +68.1% |
| jscpd@4 | 31.9× slower | +4.9% | −0.5% | +9.1% |
| PMD CPD | 428× slower | −79.5% | −73.6% | −75.2% |

## Summary

**jscpd@5 is the fastest tool by a wide margin** — 32× faster than jscpd@4 (its TypeScript predecessor) and the only tool completing in under 100ms. The Rust rewrite delivers this speed while maintaining comparable detection fidelity.

**Detection accuracy varies significantly across tools:**

- **jscpd@5 and jscpd@4** report nearly identical clone counts (212 vs 211) and similar duplicate lines (9,133 vs 9,969), confirming the Rust rewrite preserves detection quality.
- **jscpd-rs** (npm v0.1.12) reports 222 clones and 10,317 duplicate lines — slightly more than jscpd@5, likely due to a different default configuration or algorithm version in the npm package.
- **Duplo and Simian** report the most duplicates (518 and 424 clones respectively) but include significant false positives — both are purely text-based without tokenization, so formatting differences inflate counts.
- **PMD CPD** finds the fewest clones (56) despite running for 34 seconds across 34 languages. It only supports a limited set of languages and cannot process most fixture formats.
- **Fallow dupes** only analyzes TypeScript/JavaScript files (34 of 547), making it unsuitable for polyglot codebases.

**Key tradeoff:** Speed vs. precision. jscpd@5 and jscpd@4 use token-based detection that balances recall and precision. Simian and Duplo are faster at raw text matching but over-report. PMD CPD is thorough per-language but painfully slow and coverage-limited.

## Cross-Format Detection (Svelte, Astro, Vue)

Multi-file component formats like `.svelte`, `.astro`, and `.vue` embed template, script, and style blocks in a single file. Effective CPD tools must parse these formats to detect duplicates both **within** the same format (e.g., `component1.svelte` ↔ `component2.svelte`) and **across** formats (e.g., shared CSS between `.svelte` and `.astro`).

The fixtures include Svelte/Astro components that share ~60 lines of identical CSS and matching template structure (same class names, same layout) — a realistic cross-framework duplication scenario.

### Within-Format Clones

| Tool | Svelte→Svelte | Astro→Astro | Vue→Vue | Total |
|------|---------------|-------------|---------|-------|
| jscpd@5 | 3 (410 lines) | 3 (124 lines) | 6 (274 lines) | 12 (808 lines) |
| jscpd@4 | 3 (410 lines) | 3 (124 lines) | 4 (224 lines) | 10 (758 lines) |
| jscpd-rs | 3 (410 lines) | 5 (177 lines) | 6 (274 lines) | 14 (861 lines) |
| Duplo | 1 (165 lines) | 1 (112 lines) | 3 (219 lines) | 5 (496 lines) |
| Simian | 1 (231 lines) | 1 (162 lines) | 4 (148 lines) | 6 (541 lines) |
| PMD CPD | 0 | 0 | 2 (29 lines) | 2 (29 lines) |
| Fallow | 1 (58 lines) | 0 | 2 (167 lines) | 3 (225 lines) |

### Cross-Format Clones (Svelte ↔ Astro)

| Tool | Cross-Format Clones | Lines | Details |
|------|---------------------|-------|---------|
| jscpd-rs | 4 | 235 | CSS (46 lines) + markup (7, 46, 136 lines) |
| jscpd@4 | 3 | 189 | CSS (46 lines) + markup (7, 136 lines) + JS (28 lines) |
| jscpd@5 | 2 | 53 | CSS (46 lines) + markup (7 lines) |
| Duplo | 8 | 92 | Text matches: CSS (6, 5, 30, 5 lines) × 4 pairs |
| Simian | 1+ | — | Reports as aggregate blocks, not per-language |
| PMD CPD | 0 | 0 | Cannot detect cross-format duplicates |
| Fallow | 0 | 0 | Only processes JS/TS files |

### Key Findings

- **jscpd variants** are the only tools that detect cross-format clones **by language section** — finding that the same CSS block appears in both `.svelte` and `.astro` files, and that template markup overlaps. This is the token-based tokenizer at work: it parses component files into separate language sections and matches within each.
- **jscpd@5** is the most conservative, finding only the clear CSS (46 lines) and small markup (7 lines) cross-format matches. **jscpd@4** also detects a larger markup overlap (136 lines). **jscpd-rs** finds the most (4 clones, 235 lines), combining both patterns.
- **Duplo** finds 8 cross-format Svelte↔Astro clones but as raw text matches (5–30 lines each), without distinguishing which language section (CSS vs template vs script) is duplicated.
- **Simian** detects the overlaps but reports them as aggregate text blocks, making it impossible to attribute which part of a multi-format component is duplicated.
- **PMD CPD** cannot detect cross-format duplicates at all — it processes each language independently and has no concept of component file structure.
- **Fallow** only analyzes TypeScript/JavaScript, so it misses all CSS and template duplicates across formats.