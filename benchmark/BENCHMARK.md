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

## Cross-Format Detection (Svelte, Astro, Vue, Markdown)

Multi-file component formats like `.svelte`, `.astro`, and `.vue` embed template, script, and style blocks in a single file. Markdown files (`.md`) can contain embedded code blocks (YAML frontmatter, TypeScript, Python, etc.). Effective CPD tools must parse these formats to detect duplicates both **within** the same format and **across** formats (e.g., shared CSS between `.svelte` and `.astro`, or matching TypeScript code blocks inside `.md` files).

The fixtures include:
- **Svelte/Astro components** that share ~60 lines of identical CSS and matching template structure (same class names, same layout)
- **Markdown files** with embedded YAML frontmatter, TypeScript, and Python code blocks where `file3.md` and `file4.md` share identical code despite different prose

### Within-Format Clones

| Tool | Svelte→Svelte | Astro→Astro | Vue→Vue | Markdown | Total |
|------|---------------|-------------|---------|----------|-------|
| jscpd@5 | 3 (410 lines) | 3 (124 lines) | 6 (274 lines) | 7 (356 lines) | 19 (1,164 lines) |
| jscpd@4 | 3 (410 lines) | 3 (124 lines) | 4 (224 lines) | 7 (343 lines) | 17 (1,101 lines) |
| jscpd-rs | 3 (410 lines) | 5 (177 lines) | 6 (274 lines) | 8 (350 lines) | 22 (1,211 lines) |
| Duplo | 1 (165 lines) | 1 (112 lines) | 3 (219 lines) | 5 (195 lines) | 10 (691 lines) |
| Simian | 1 (231 lines) | 1 (162 lines) | 4 (148 lines) | 4 (219 lines) | 10 (760 lines) |
| PMD CPD | 0 | 0 | 2 (29 lines) | 0 | 2 (29 lines) |
| Fallow | 1 (58 lines) | 0 | 2 (167 lines) | 0 | 3 (225 lines) |

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

### Markdown Embedded-Code Detection

Markdown files `file3.md` and `file4.md` contain identical TypeScript and Python code blocks inside fenced code regions, plus matching YAML frontmatter. This tests whether tools can detect duplication inside embedded language blocks.

| Tool | Detected Embedded Languages | Clones | Lines |
|------|-----------------------------|--------|-------|
| jscpd@5 | TypeScript, Python, YAML, Markdown | 7 | 356 |
| jscpd@4 | TypeScript, Python, YAML, Markdown | 7 | 343 |
| jscpd-rs | TypeScript, Python, YAML, Markdown, Coffeescript | 8 | 350 |
| Duplo | (flat text only) | 5 | 195 |
| Simian | (flat text only) | 4 | 219 |
| PMD CPD | — | 0 | 0 |
| Fallow | — | 0 | 0 |

jscpd variants are the **only** tools that detect duplicates by embedded language — identifying matching TypeScript code blocks, Python code blocks, and YAML frontmatter separately within markdown. All other tools treat markdown as flat text, matching prose content rather than code.

### Key Findings

- **jscpd variants** are the only tools that detect cross-format clones **by language section** — finding that the same CSS block appears in both `.svelte` and `.astro` files, and that template markup overlaps. This is the token-based tokenizer at work: it parses component files into separate language sections and matches within each.
- **jscpd@5** is the most conservative cross-format detector, finding only the clear CSS (46 lines) and small markup (7 lines) matches. **jscpd@4** also detects a larger markup overlap (136 lines). **jscpd-rs** finds the most (4 clones, 235 lines), combining both patterns.
- **Markdown embedded-code detection** is unique to jscpd. It parses fenced code blocks and frontmatter as separate languages (TypeScript, Python, YAML), detecting duplication that text-based tools miss because they match prose content instead of embedded code. jscpd@5 identifies 7 markdown clones (356 lines) across 4 embedded languages; text-based tools find only 4–5 clones matching flat prose.
- **Duplo** finds 8 cross-format Svelte↔Astro clones but as raw text matches (5–30 lines each), without distinguishing which language section (CSS vs template vs script) is duplicated.
- **Simian** detects overlaps but reports them as aggregate text blocks, making it impossible to attribute which part of a multi-format component is duplicated.
- **PMD CPD** cannot detect cross-format duplicates at all — it processes each language independently and has no concept of component file structure.
- **Fallow** only analyzes TypeScript/JavaScript, so it misses all CSS, template, and markdown duplicates across formats.

## AI Token Efficiency

When CPD output is fed to an LLM (for automated refactoring, code review, or deduplication workflows), output size directly impacts cost, latency, and context window usage. This section measures the token efficiency of each tool's default output format.

### Output Format Comparison

| Tool | Output Format | Output Size | Est. Tokens | Clones | Tokens/Clone |
|------|--------------|-------------|-------------|--------|---------------|
| jscpd@5 AI | Plain text (compressed) | 11 KB | ~2,800 | 212 | 13 |
| Fallow | Plain text | 1.6 KB | ~400 | 10 | 40 |
| Simian | Plain text | 60 KB | ~15,000 | 424 | 35 |
| jscpd@5 console | Plain text (human) | 93 KB | ~23,000 | 212 | 110 |
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
| 1 | jscpd@5 AI | 13 | Yes — designed for it |
| 2 | Fallow | 40 | Partial — limited to JS/TS |
| 3 | Simian | 35 | Partial — no structured metadata |
| 4 | jscpd@5 console | 110 | Marginal — human-oriented |
| 5 | Duplo | 305 | No — large JSON |
| 6 | PMD CPD | 375 | No — spread across 34 files |

Beyond a few dozen clones, only the AI reporter remains practical for LLM consumption. The console reporter is human-readable but 8× more expensive in tokens.