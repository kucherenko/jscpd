# jscpd

[![npm version](https://img.shields.io/npm/v/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![skills.sh installs](https://skills.sh/b/kucherenko/jscpd)](https://skills.sh/kucherenko/jscpd)
[![Crates.io Version](https://img.shields.io/crates/v/jscpd?color=green)](https://crates.io/crates/jscpd)
![NPM License](https://img.shields.io/npm/l/jscpd)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml)
[![Socket Badge](https://badge.socket.dev/npm/package/jscpd/latest)](https://socket.dev/npm/package/jscpd)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kucherenko/jscpd/badge)](https://scorecard.dev/viewer/?uri=github.com/kucherenko/jscpd)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14188/badge)](https://www.bestpractices.dev/projects/14188)

> Duplicate code detector for 220+ languages — plus dead code, complexity hotspots, duplication trends over git history, and one health score for the whole codebase. Rust engine, self-contained binary, AI-ready with an MCP server and a token-efficient reporter.

**Documentation:** https://jscpd.dev

jscpd tokenizes each of its 224 supported formats the way that language defines it — its own comment and string rules, not generic text — then finds duplicated token sequences across files with a rolling [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) hash. Opt-in passes catch copies that differ only in names or values (Type-2) or that have a few edited lines (Type-3), and an experimental one uses a code embedding model for functions that do the same thing written differently (Type-4). See [How detection works](docs/rust.md#how-detection-works) for the full mechanism, and [Supported formats](FORMATS.md) for the full list.

Beyond duplicates, jscpd also finds dead code (`--dead-code`), ranks files by complexity (`--complexity`), tracks duplication over git history (`--history`), and rolls it all into one health score (`--health`) — see [Features](#features) below.

## Quick Start

```bash
# macOS / Linux
curl -fsSL https://jscpd.dev/install.sh | bash

# Windows (PowerShell)
irm https://jscpd.dev/install.ps1 | iex

# No install — run once with npx (Node.js)
npx jscpd .
```

Then scan a project:

```bash
jscpd /path/to/code
```

### Other install methods

| Method | Command | Notes |
|--------|---------|-------|
| npm | `npm install -g jscpd` | Installs the `jscpd` command; prebuilt binary, no Node.js at runtime |
| npm (`cpd` command) | `npm install -g cpd` | Same binary, exposed as `cpd` |
| PyPI | `pip install jscpd` | Platform wheels with both commands; also `pipx install jscpd`, `uv tool install jscpd`, or `uvx jscpd .` to run without installing |
| Cargo | `cargo install jscpd` | Builds from crates.io; installs both `jscpd` and `cpd` |
| Homebrew | `brew install jscpd` | macOS / Linux |
| Nix | `nix run github:kucherenko/jscpd -- /path/to/code` | Or `nix profile install github:kucherenko/jscpd` |
| Docker | `docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd .` | Multi-arch image built from the release binaries |

### GitHub Action

```yaml
- uses: kucherenko/jscpd@v5
  with:
    threshold: 5
```

Uploads SARIF results to GitHub Code Scanning by default. See [CI & Pre-Commit Hooks](docs/ci-and-hooks.md) for all inputs and outputs.

## Documentation

| Document | Description |
|----------|-------------|
| [Rust engine](docs/rust.md) | Installation, CLI reference, reporters, baseline, summary, complexity, dashboard, blame, config file |
| [AI-Ready](docs/ai-ready.md) | AI reporter, agent skills, MCP server |
| [Editors](docs/editors.md) | `jscpd --lsp`, the language server: setup for Neovim, Helix, Sublime Text, Emacs and JetBrains IDEs |
| [Programming API](docs/api.md) | Rust API (`cpd-finder` crate) |
| [CI & Pre-Commit Hooks](docs/ci-and-hooks.md) | GitHub Action, Docker image, pre-commit hooks |
| [Packages](docs/packages.md) | npm packages and crates that make up a release |
| [Supported formats](FORMATS.md) | All 224 formats with their file extensions |
| [Runnable demos](fixtures) | One `fixtures/<feature>-demo/` directory per feature, each README lists the commands with their expected output |

## Features

jscpd v5 is a Rust engine that ships as a self-contained binary — no runtime required — under two npm names ([`jscpd`](https://www.npmjs.com/package/jscpd) installs the `jscpd` command, [`cpd`](https://www.npmjs.com/package/cpd) installs `cpd`), on [PyPI](https://pypi.org/project/jscpd/), [crates.io](https://crates.io/crates/jscpd), Homebrew, Nix, Docker, and as a GitHub Action.

### Duplicate detection

- **Language-aware tokenization** — per-format comment and string syntax for all 224 formats, the oxc parser for JavaScript/TypeScript/JSX/TSX, embedded-language extraction for Vue, Svelte, Astro, Markdown and Razor, and keyword/identifier/literal classification, so a clone is a repeated sequence of *language tokens*, never a repeated run of text (see [How detection works](docs/rust.md#how-detection-works))
- **224 language formats**, with cross-format detection (Vue SFC, Svelte, Astro, Markdown) and `--cross-formats` groups to match clones across JavaScript and TypeScript
- **Type-2 clones** — `--ignore-identifiers`, `--ignore-literals` and `--ignore-annotations` find blocks that differ only in names, literal values or annotations, reported as `renamed` (see [docs](docs/rust.md#type-2-clones-renamed-identifiers-literals-and-annotations))
- **Type-3 near-miss clones** — `--max-gap-lines N` merges a copy with a few inserted or changed lines into one `similar` clone with a similarity score; `--similarity 0.85` compares whole JavaScript, TypeScript and Python functions by syntax-tree structure, and Python classes, variables and type aliases too, in code blocks of Markdown and components as well, catching renames and scattered edits; `--similarity-identifiers role-aware` lowers the score of functions that call different methods, `--similarity-literals` decides whether literal values count, and `--similarity-decorators` whether Python decorators do (see [docs](docs/rust.md#type-3-clones-near-miss-merging-with---max-gap-lines))
- **Type-4 semantic clones (experimental)** — `--semantic` embeds the functions of JavaScript, TypeScript, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift files with a code embedding model, either one jscpd runs itself (after `jscpd --semantic-download` once) or any OpenAI-compatible API, and reports functions that do the same thing written differently: the same feature implemented twice in one language, or a rule your Rust backend enforces and your Svelte frontend repeats (see [Semantic clones](#semantic-clones-experimental) below)
- **Port progress and parity (experimental)**: `jscpd --compare python/ typescript/` pairs the functions of two folders with the same model and lists which functions of each have a counterpart in the other: what is left to port to another language or platform, or what the Android version of an app has that the iOS one lacks (see [Comparing two codebases](#comparing-two-codebases-experimental) below)
- **Clone kinds everywhere** — `exact`, `renamed` or `similar` in the console, JSON (`kind`, `similarity`, `method`), XML, HTML, Xcode, SARIF (`jscpd/duplicate-code`, `jscpd/renamed-code`, `jscpd/similar-code`, `jscpd/similar-function`, `jscpd/semantic-code`) and Code Climate output; default runs still report only `exact` clones
- **`--kind`** — keep only the clone kinds you care about: `--kind renamed`, or `--kind gap,ast` for near-miss clones only. Statistics and `--threshold` follow the filter; a kind whose detector is off warns, an unknown kind errors (see [docs](docs/rust.md#filtering-by-kind-with---kind))
- **15 reporters**: `console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`, `sarif`, `codeclimate`, `openmetrics`, `ai`, `xcode`, `threshold`, `silent`
- **Clone baseline** — gate CI on *new* duplication only. `--baseline .jscpd-baseline.json --fail-on-new-clones[=N]` tolerates legacy clones and fails the build on regressions; `--baseline-from-ref origin/main` does the same without a committed file (see [docs](docs/rust.md#baseline))
- **Exit codes you can gate on** — an unknown `--format`, a missing scan path or a reporter that can't write its file now exit 1 instead of passing with an empty report; `--fail-on-empty` fails a scan that analyzed no files (see [Exit codes](docs/rust.md#exit-codes))
- **GitLab-ready reporters** — `codeclimate` (`gl-code-quality-report.json`) and `openmetrics` (`jscpd-metrics.txt`) plug into `artifacts:reports`
- **Git blame** with side-by-side author comparison (`--blame --reporters console-full`)
- **`--skip-local`** — report only clones that cross the scan roots: `jscpd packages/api packages/web --skip-local` drops pairs inside either tree, keeping only api-to-web duplication
- **`--skip-isolated`** — ignore duplication between monorepo folders owned by different teams (`--skip-isolated "packages/team-a|packages/team-b"`)

### Beyond duplication

- **`--history`** — duplication trend over git history: `jscpd src --history v5.0.0..HEAD` scans every commit in the range and prints a sparkline, a per-commit table with the change between points, the overall trend, and how far `--threshold` could be tightened (see [docs](docs/rust.md#history))
- **`--dead-code`** — find code nothing runs, not just code written twice: unused files, exports, declarations and imports across JavaScript, TypeScript and Python. Builds the import graph from your entry points (`package.json`, `pyproject.toml`, framework conventions) and walks it, so dead code cascades — a helper whose only caller is dead gets reported too, each finding with a confidence score and, below 100, why it might be wrong. Also ships standalone as [`basta`](rust/crates/basta) (see [docs](docs/rust.md#dead-code-detection---dead-code))
- **`--summary`** — refactoring hotspots straight from the scan: top files and folders by tokens, lines, size, and a complexity estimate (see [docs](docs/rust.md#summary))
- **`--complexity`** — the complexity ranking alone, without clone detection: most complex files and folders from one tokenizing pass, in the console, `ai` or `json` (see [docs](docs/rust.md#complexity-only))
- **`--health`** — one 0-100 score with a grade, from the share of code that's duplicated, dead, or concentrated in complex files; size-aware, calibrated on 42 open-source projects, extensible with coverage, test or security metrics via `--health-input`. Console badge, JSON, SVG badge (see [docs](docs/rust.md#health-score))
- **`--dashboard`** — the whole picture on one screen, under the health badge: project size with the largest code files, duplication by clone kind with the most duplicated files, the most complex files, and dead code by category for JavaScript, TypeScript and Python (see [docs](docs/rust.md#dashboard))

### AI and operations

- **`--mcp`** — built-in MCP server over stdio with fully described tools: point your AI assistant at the binary and it can check a snippet against your codebase for all four types of clone (exact, renamed, near-miss and semantic), list a file's clones, and compare two folders function by function (see [docs](docs/ai-ready.md#stdio-transport-rust-v5))
- **`--lsp`**: a language server over stdio. Clones, similar functions, semantic clones, dead code and complexity show up as diagnostics in the files you edit and follow the text as you type, each analysis with a switch of its own (see [Editors](#editors) below)
- **AI reporter** — token-efficient output for LLM pipelines (~79% fewer tokens than console)
- **Prebuilt for 8 platforms** — macOS arm64/x64, Linux arm64/x64 (glibc and musl), Windows arm64/x64
- **`--workers`** — control parallelism for file tokenization and detection (default: all CPU cores)
- **Config discovery** — `.jscpd.json`, `.config/jscpd.json`, or the `jscpd` key in `package.json`
- **Symbolic links are skipped unless `--follow-symlinks`** — v4 followed them by default. With the flag, a file reached through a link is reported by the path it was found at, and a file reachable through several paths is counted once
- **Quiet in pipelines** — tips and sponsor lines print only on an interactive terminal; `--no-tips`, `CI` or `JSCPD_NO_TIPS` switch them off everywhere

See the [Rust docs](docs/rust.md) for the full CLI reference and [`rust/CHANGELOG.md`](rust/CHANGELOG.md) for release notes.

### Looking for v4?

jscpd v4 (TypeScript engine, Node.js API, LevelDB/Redis stores) is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published as `jscpd@4` / the `latest-4` dist-tag. [README-v4.md](README-v4.md) describes it in one page (install, CLI, API, packages, maintenance policy); the same content is at https://jscpd.dev/getting-started/v4.

## Packages

| Package | Registry | Description |
|---------|----------|-------------|
| [jscpd](rust/jscpd) | [npm](https://www.npmjs.com/package/jscpd) | Installs the `jscpd` command (prebuilt binary via platform packages) |
| [cpd](rust) | [npm](https://www.npmjs.com/package/cpd) | Installs the `cpd` command (same binary) |
| [jscpd-\<platform\>](rust/npm) | npm | Platform binary packages pulled in as optional dependencies: `jscpd-darwin-arm64`, `jscpd-darwin-x64`, `jscpd-linux-x64-gnu`, `jscpd-linux-arm64-gnu`, `jscpd-linux-x64-musl`, `jscpd-linux-arm64-musl`, `jscpd-windows-x64-msvc`, `jscpd-windows-arm64-msvc` |
| [jscpd](rust/scripts/build-pypi-wheels.py) | [PyPI](https://pypi.org/project/jscpd/) | Platform wheels repacked from the release binaries; installs both `jscpd` and `cpd` commands |
| [jscpd](rust/crates/cpd) | [crates.io](https://crates.io/crates/jscpd) | CLI crate; installs both `jscpd` and `cpd` binaries |
| [cpd-core](rust/crates/cpd-core) | [crates.io](https://crates.io/crates/cpd-core) | Detection algorithm (Rabin-Karp rolling hash), data models |
| [cpd-tokenizer](rust/crates/cpd-tokenizer) | [crates.io](https://crates.io/crates/cpd-tokenizer) | Source code tokenization (224 formats) |
| [cpd-similarity](rust/crates/cpd-similarity) | [crates.io](https://crates.io/crates/cpd-similarity) | Structural similarity (`--similarity`): functions, classes and other units from syntax trees, their summaries, the MinHash index and the pairing |
| [cpd-finder](rust/crates/cpd-finder) | [crates.io](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame — the library entry point |
| [cpd-reporter](rust/crates/cpd-reporter) | [crates.io](https://crates.io/crates/cpd-reporter) | Output formatting (15 reporters, duplication and dead code) |
| [cpd-semantic](rust/crates/cpd-semantic) | [crates.io](https://crates.io/crates/cpd-semantic) | Semantic clones (`--semantic`, experimental): function extraction, code embeddings from a model run in-process or an API, pairing |
| [basta](rust/crates/basta) | npm / crates.io | Dead code detection — unused files, exports, symbols and imports for JavaScript, TypeScript and Python. Installs the `basta` command; the same engine backs `jscpd --dead-code` |

## Who Uses jscpd

The `jscpd` npm package is downloaded **10M+ times per month**, and [~5,000 repositories](https://github.com/kucherenko/jscpd/network/dependents) declare it on GitHub's dependents graph.

**Bundled by analysis platforms:**

- [GitHub Super Linter](https://github.com/super-linter/super-linter) — official GitHub linter aggregator, bundles jscpd as its copy/paste detector and runs it by default; 15,500+ workflow files on GitHub reference Super Linter (as of Sep 2026)
- [MegaLinter](https://github.com/oxsecurity/megalinter) — open-source linter aggregator for CI, ships jscpd in every flavor including `ci_light`
- [Codacy](https://www.codacy.com/) — automated code analysis platform, jscpd powers the duplication engine

**Explicitly enabled in Super Linter** (`VALIDATE_JSCPD: true`) **by dozens of public repositories, including:**

- [A2A](https://github.com/a2aproject/A2A) — Google's Agent2Agent protocol (25k+ stars)
- [RimSort](https://github.com/RimSort/RimSort) — mod manager for RimWorld (1.2k+ stars); also runs jscpd directly with its own `.jscpd.json`
- [Contact Center AI samples](https://github.com/GoogleCloudPlatform/contact-center-ai-samples) — official Google Cloud samples, with a dedicated jscpd config
- [Drifty](https://github.com/SaptarshiSarkar12/Drifty) — open-source download manager

**Used in notable projects:**

- [OpenClaw](https://github.com/openclaw/openclaw) — personal AI assistant, runs jscpd as a duplication gate in its check scripts
- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) — DeepSeek's plugin harness, jscpd config in CI
- [degit](https://github.com/Rich-Harris/degit) — Rich Harris's project scaffolder
- [MEGA webclient](https://github.com/meganz/webclient) — the MEGA.nz web client
- [Microsoft TypeAgent](https://github.com/microsoft/TypeAgent)
- [Salesforce DX VS Code](https://github.com/forcedotcom/salesforcedx-vscode)
- [Alibaba AppWorks](https://github.com/apptools-lab/AppWorks) — embeds jscpd as a library
- [OVHcloud manager](https://github.com/ovh/manager) — OVHcloud's customer control panel
- [KiroCrew](https://github.com/kirodotdev/KiroCrew) — self-improving persistent development workspace

## Benchmark

Compared against other copy/paste detectors on the `fixtures/` corpus (547 files, 150+ formats), default thresholds, wall-clock time on Apple Silicon:

| Tool | Time | Files | Clones | Dup Lines |
|------|------|-------|--------|-----------|
| jscpd | 84ms | 347 | 212 | 9,133 |
| jscpd-rs | 111ms | 360 | 222 | 10,317 |
| Duplo | 162ms | 319 | 518 | 13,049 |
| Fallow dupes | 164ms | 34 | 10 | 3,137 |
| Simian | 964ms | 547 | 424 | 15,351 |
| PMD CPD | 35.980s | 71 | 56 | 2,267 |

Methodology, cross-format detection and AI-token-efficiency comparisons: [benchmark/BENCHMARK.md](benchmark/BENCHMARK.md). Re-run with [`benchmark/benchmark.sh`](benchmark/benchmark.sh).

## Semantic clones (experimental)

Token matching finds code that someone copied. It cannot find two functions that do the same job with different code, such as a validation rule that a Rust backend enforces and a Svelte frontend writes again, or two helpers that two people wrote for the same task. `--semantic` looks for these pairs with a code embedding model. jscpd turns every function into a vector and reports two functions as a `semantic` clone when each is the other's closest match and their vectors are similar enough.

```bash
jscpd --semantic-download                                    # once: CodeRankEmbed, 548 MB, into the jscpd cache
jscpd --semantic src/                                        # pairs within one language and across languages
jscpd --semantic --semantic-scope cross backend/ frontend/   # only pairs across languages
jscpd --semantic --kind semantic -r ai .                     # only semantic clones, one line each
```

The model runs inside jscpd on the CPU, so a scan makes no network call. The default model is [CodeRankEmbed](https://huggingface.co/nomic-ai/CodeRankEmbed) (MIT), and jscpd can also run jina-embeddings-v2-base-code. `jscpd --semantic-models` lists nine models with the thresholds jscpd calibrated for each one, and `--semantic-model` picks one of them. To use a model that jscpd does not run itself, point `--semantic-url` at an OpenAI-compatible embeddings API such as Ollama, LM Studio or `llama-server`. jscpd reads an API key only from the `JSCPD_SEMANTIC_API_KEY` environment variable.

jscpd compares the functions of JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift files. A pair within one language needs a higher similarity than a pair across languages, because code in one language resembles other code in that language whatever it does. With CodeRankEmbed the two thresholds are 0.4125 and 0.6375, and `--semantic-threshold` and `--semantic-same-threshold` change them. jscpd caches the vectors, so a second run embeds only the functions whose code changed.

The console, `ai`, JSON, SARIF and Code Climate reporters mark these clones as kind `semantic` with their similarity. The mode is experimental. Besides real duplicates it reports related code, such as a client function and the server endpoint it calls, so review a pair before you merge the two functions. The [docs](docs/rust.md#semantic-clones-with---semantic-experimental) describe the rules, [`fixtures/semantic-demo`](fixtures/semantic-demo/README.md) is a runnable example with a Rust backend and a SvelteKit frontend, and [Embedding Models](https://jscpd.dev/benchmarks/embedding-models) compares the nine models.

## Comparing two codebases (experimental)

`--compare` takes two folders and pairs their functions with the `--semantic` model. During a port, such as a library moving to another language or an iOS app moving to Android, it shows which functions of the source already have a version in the target and which are still to port. For two implementations of one app, such as the Android and the iOS one, it shows what both have and what only one of them has.

```bash
jscpd --compare python typescript        # progress of a port: the source first, the target second
jscpd --compare ios android -r console-full  # parity, with every pair and its similarity
```

```text
Code
 71% 5 of 7 functions in python have a counterpart in typescript
 80% 4 of 5 functions in typescript have a counterpart in python

Paired under other names (1):
  python                        typescript              similarity
  billing.py:28 tax_for_region  billing.ts:27 salesTax  0.87 high

Only in python (2):
  billing.py (1)
    46  due_date                6 lines
  shipping.py (1)
    18  estimate_delivery_days  8 lines

Tests
 75% 3 of 4 tests in python have a counterpart in typescript
100% 3 of 3 tests in typescript have a counterpart in python
```

A function pairs with its counterpart when the model finds them each other's closest match. A short function the model cannot place pairs by name when the names match once case and underscores are ignored (`encodeBinary`, `encode_binary`) and the code is similar enough. jscpd measures tests and code in two blocks and pairs a test only with a test. It tells a test by the conventions of its language, such as `*_test.go`, `test_*.py`, `*.test.ts`, `src/test/` or Rust's `#[cfg(test)]`. Every pair has its similarity and a level, `high`, `medium` or `low`, on the scale of the model, and the report lists the pairs under other names on their own, since nobody finds those by searching for a name. The console, JSON and Markdown reporters print the totals per side and per file with the mean similarity, the functions with no counterpart, and the pairs. `-r html` writes a migration map, a page that draws both sides as dependency graphs with the pairs bridging them and marks the functions ready to port, the ones whose callees all have a counterpart already. A second tab lists the same pairs as a table you can sort. The [docs](docs/rust.md#comparing-two-codebases-with---compare-experimental) describe the rules and how they did on two real codebases, and [`fixtures/compare-demo`](fixtures/compare-demo/README.md) is a runnable example.

## Editors

`jscpd --lsp` runs jscpd as a language server. An editor starts it for a workspace, and the files you edit get what jscpd finds as diagnostics, updated as you type: clones by default, and similar functions, semantic clones, dead code and complexity when `--lsp-analyses` or the `lsp` section of `.jscpd.json` turns them on. A clone comes with "Go to the other copy" and "Ignore this clone" actions. In Neovim 0.11:

```lua
vim.lsp.config('jscpd', { cmd = { 'jscpd', '--lsp' }, root_markers = { '.jscpd.json', '.git' } })
vim.lsp.enable('jscpd')
```

[Editors](docs/editors.md) has the setup for Helix, Sublime Text, Emacs and JetBrains IDEs too, and [`fixtures/lsp-demo`](fixtures/lsp-demo/README.md) is a project that shows each analysis. Clients for VS Code and Zed will follow in a repository of their own.

## AI-Ready Features

jscpd integrates into AI-powered workflows through three mechanisms:

### AI Reporter

Token-efficient output for LLM pipelines (~79% fewer tokens than the default console reporter):

```bash
jscpd --reporters ai /path/to/source              # compact clone list
jscpd --reporters ai --summary /path/to/source    # + compact codebase summary
jscpd --reporters ai --complexity /path/to/source # most complex files, no clone detection
```

### Agent Skills

Installable skills that teach AI coding assistants how to use jscpd, refactor detected duplications, and clean up a codebase more broadly:

| Skill | Purpose | Install |
|-------|---------|---------|
| [`jscpd`](skills/jscpd/SKILL.md) | Tool reference — CLI options, AI reporter format, config syntax | `npx skills add kucherenko/jscpd --skill jscpd` |
| [`dry-refactoring`](skills/dry-refactoring/SKILL.md) | Guided refactoring workflow — read clones, choose strategy, apply, verify | `npx skills add kucherenko/jscpd --skill dry-refactoring` |
| [`compare-codebases`](skills/compare-codebases/SKILL.md) | How `--compare` pairs the functions of two folders, and a step-by-step way to compare two folders and check the result | `npx skills add kucherenko/jscpd --skill compare-codebases` |
| [`code-migration`](skills/code-migration/SKILL.md) | Port a codebase to another language or framework, tests first and code second, with a coverage map binding functions to their tests and `--compare` as the progress measure; or check two implementations for parity | `npx skills add kucherenko/jscpd --skill code-migration` |
| [`codebase-refactoring`](skills/codebase-refactoring/SKILL.md) | Broader health pass — fix duplication, then remove/refactor dead code, then simplify the biggest/most complex files, prioritized from `--health` | `npx skills add kucherenko/jscpd --skill codebase-refactoring` |

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results — or "clean up this codebase" for the broader pass, or "port this Python library to Rust" for a migration.

### MCP Server

`jscpd --mcp /path/to/project` scans the project and serves the Model Context Protocol over stdio. An assistant can check a snippet against the codebase before writing it, list a file's clones, scan the working directory again and compare two folders function by function. The tools that find clones can look for exact (Type-1), renamed (Type-2), near-miss (Type-3) and semantic (Type-4) ones.

See [AI-Ready docs](docs/ai-ready.md) for full details.

## Citation

If jscpd is part of your research, cite it via the repository's [`CITATION.cff`](CITATION.cff) (GitHub's "Cite this repository" button produces BibTeX and APA) or with:

```bibtex
@software{jscpd,
  title        = {jscpd: copy/paste detector for programming source code},
  author       = {Kucherenko, Andrey},
  year         = {2026},
  version      = {5.3.0},
  license      = {MIT},
  url          = {https://github.com/kucherenko/jscpd},
}
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, test policy, and pull request requirements. In short:

```bash
cd rust
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Security issues go through the [security policy](SECURITY.md), not public issues.

## Backers

Thank you to all our backers! 🙏 [[Become a backer](https://opencollective.com/jscpd#backer)]

<a href="https://opencollective.com/jscpd#backers" target="_blank"><img src="https://opencollective.com/jscpd/backers.svg?width=890"></a>

## Sponsors

Support this project by becoming a sponsor. Your logo will show up here with a link to your website. [[Become a sponsor](https://opencollective.com/jscpd#sponsor)]

<a href="https://opencollective.com/jscpd/sponsor/0/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/0/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/1/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/1/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/2/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/2/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/3/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/3/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/4/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/4/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/5/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/5/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/6/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/6/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/7/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/7/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/8/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/8/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/9/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/9/avatar.svg"></a>


## License

[MIT](LICENSE) © Andrey Kucherenko
