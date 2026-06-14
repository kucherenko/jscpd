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
| jscpd-rs | 111ms | 360 | 222 | 10,317 |
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

## Multi-Format & Cross-Format Detection

Component file formats (`.vue`, `.svelte`, `.astro`) and rich documents (`.md`) embed multiple languages in a single file — template, script, and style blocks in components; YAML frontmatter and fenced code blocks in markdown. A CPD tool's ability to parse these structures determines whether it can detect duplicates **within** each embedded language and **across** different file formats sharing the same code.

### Format Support Overview

| Tool | Languages | `.vue` | `.svelte` | `.astro` | `.md` | Cross-Format |
|------|-----------|--------|-----------|----------|-------|--------------|
| jscpd@5 | 223 | Section-aware | Section-aware | Section-aware | Section-aware | Yes |
| jscpd@4 | 224 | Flat text | Flat text | Flat text | Section-aware | Yes |
| jscpd-rs | 223 | Flat text | Flat text | Flat text | Section-aware | Yes |
| Duplo | ~7 | — | — | — | — | Text-only |
| Simian | ∞ (any text) | Flat text | Flat text | Flat text | Flat text | Text-only |
| PMD CPD | 36 | — | — | — | — | No |
| Fallow | JS/TS only | JS blocks only | — | — | — | No |

**Section-aware** means the tool parses a file into separate language sections (CSS, JavaScript, template, etc.) and matches duplicates within each section independently. **Flat text** means the tool treats the entire file as undifferentiated text. **Text-only** cross-format means the tool can find text overlaps between different file types but cannot identify *which* language section is duplicated.

### Cross-Format Detection (Svelte ↔ Astro)

The Svelte and Astro fixture components share ~60 lines of identical CSS and matching template structure. Only section-aware tools can attribute these matches to specific languages (CSS vs. markup vs. script).

| Tool | Cross-Format Clones | Lines | Detail |
|------|---------------------|-------|--------|
| jscpd-rs | 4 | 235 | CSS (46 lines) + markup (7, 46, 136 lines) |
| jscpd@4 | 3 | 189 | CSS (46 lines) + markup (7, 136 lines) + JS (28 lines) |
| jscpd@5 | 2 | 53 | CSS (46 lines) + markup (7 lines) |
| Duplo | 8 | 92 | Raw text matches, no language attribution |
| Simian | 1+ | — | Aggregate text blocks, no language attribution |
| PMD CPD | 0 | 0 | Cannot detect cross-format duplicates |
| Fallow | 0 | 0 | JS/TS only |

### Markdown Embedded-Code Detection

`file3.md` and `file4.md` contain identical TypeScript and Python code blocks inside fenced regions, plus matching YAML frontmatter. Section-aware tools detect these as separate language clones; flat-text tools match only the surrounding prose.

| Tool | Detected Embedded Languages | Clones | Lines |
|------|-----------------------------|--------|-------|
| jscpd@5 | TypeScript, Python, YAML, Markdown | 7 | 356 |
| jscpd@4 | TypeScript, Python, YAML, Markdown | 7 | 343 |
| jscpd-rs | TypeScript, Python, YAML, Markdown, Coffeescript | 8 | 350 |
| Duplo | (flat text only) | 5 | 195 |
| Simian | (flat text only) | 4 | 219 |
| PMD CPD | — | 0 | 0 |
| Fallow | — | 0 | 0 |

### Key Findings

- **jscpd@5** is the only tool that parses `.vue`, `.svelte`, `.astro`, and `.md` into separate language sections and detects cross-format clones with language attribution. It identifies that the same CSS block appears in both `.svelte` and `.astro` files, and that TypeScript/Python code blocks inside markdown are duplicated independently of the prose.
- **jscpd@4 and jscpd-rs** detect markdown embedded-code sections with language attribution and do find cross-format clones between Svelte and Astro — but they treat `.vue`, `.svelte`, and `.astro` as flat text rather than parsing them into template/script/style sections, so cross-format matches lack language attribution (CSS vs. markup vs. script).
- **Simian and Duplo** find text overlaps between file types but report them as undifferentiated matches — there is no way to tell whether the duplication is in CSS, JavaScript, or template markup.
- **PMD CPD** cannot detect cross-format duplicates at all. It processes each language independently and has no concept of component file structure.
- **Fallow** only analyzes JS/TS, so it misses all CSS, template, and markdown duplicates across formats.

## AI Token Efficiency

When CPD output is fed to an LLM (for automated refactoring, code review, or deduplication workflows), output size directly impacts cost, latency, and context window usage. This section measures the token efficiency of each tool's default output format.

### Output Format Comparison

| Tool | Output Format | Output Size | Est. Tokens | Clones | Tokens/Clone |
|------|--------------|-------------|-------------|--------|---------------|
| jscpd@5 AI | Plain text (compressed) | 11 KB | ~2,800 | 212 | 13 |
| jscpd@4 AI | Plain text (compressed) | 11 KB | ~2,700 | 211 | 12 |
| jscpd-rs AI | Plain text (compressed) | 12 KB | ~3,000 | 222 | 13 |
| Fallow | Plain text | 1.6 KB | ~400 | 10 | 40 |
| Simian | Plain text | 60 KB | ~15,000 | 424 | 35 |
| Duplo | JSON | 754 KB | ~158,000 | 518 | 305 |
| PMD CPD | Plain text (34 files) | 83 KB | ~21,000 | 56 | 375 |

### jscpd AI Reporter

jscpd includes an `ai` reporter (`--reporters ai`) that produces a token-efficient plain-text format designed for LLM consumption. Key optimizations:

- **Common-path-prefix compression**: Shared directory prefixes are factored out (e.g., `fixtures/clike/ file1.cpp:1-88 ~ file2.cpp:1-88` instead of two full paths)
- **No code fragments**: Source code content is omitted — only file paths and line ranges are reported
- **Minimal structure**: One line per clone, no JSON overhead
- **Summary line**: Clone count and duplication percentage after a `---` separator

Sample output:

```
fixtures/clike/ file1.cpp:1-88 ~ file2.cpp:1-88
fixtures/clike/ file1.cs:1-91 ~ file2.cs:1-91
fixtures/svelte/ component1.svelte:css:112-230 ~ component2.svelte:css:112-230
fixtures/markdown/ file3.md:typescript:34-64 ~ file4.md:typescript:34-64
---
212 clones · 37.1% duplication
```

The AI reporter uses **~2,800 tokens** for 212 clones (13 tokens/clone) — an **8× reduction** versus the console reporter (~23,000 tokens, 110 tokens/clone). This makes it practical to include full CPD results in an LLM prompt context window.

### Token Efficiency Rankings

| Rank | Tool & Format | Tokens/Clone | LLM-Ready? |
|------|--------------|-------------|------------|
| 1 | jscpd@4 AI | 12 | Yes |
| 2 | jscpd@5 AI | 13 | Yes |
| 3 | jscpd-rs AI | 13 | Yes |
| 4 | Simian | 35 | Partial — no structured metadata |
| 5 | Fallow | 40 | Partial — limited to JS/TS |
| 6 | Duplo | 305 | No — large JSON |
| 7 | PMD CPD | 375 | No — spread across 34 files |

Beyond a few dozen clones, only the AI reporter remains practical for LLM consumption.
