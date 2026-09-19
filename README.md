# jscpd

[![npm version](https://img.shields.io/npm/v/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![Crates.io Version](https://img.shields.io/crates/v/jscpd?color=green)](https://crates.io/crates/jscpd)
![NPM License](https://img.shields.io/npm/l/jscpd)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml)
[![Socket Badge](https://socket.dev/api/badge/npm/package/jscpd)](https://socket.dev/npm/package/jscpd)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kucherenko/jscpd/badge)](https://scorecard.dev/viewer/?uri=github.com/kucherenko/jscpd)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14188/badge)](https://www.bestpractices.dev/projects/14188)

> Duplicate code detector for 220+ languages — plus dead code, complexity hotspots, duplication trends over git history, and one health score for the whole codebase. Rust engine, self-contained binary, AI-ready with an MCP server and a token-efficient reporter.

**Documentation:** https://jscpd.dev

jscpd tokenizes each of its 224 supported formats the way that language defines it — its own comment and string rules, not generic text — then finds duplicated token sequences across files with a rolling [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) hash. Opt-in passes catch copies that differ only in names or values (Type-2) or that have a few edited lines (Type-3). See [How detection works](docs/rust.md#how-detection-works) for the full mechanism, and [Supported formats](FORMATS.md) for the full list.

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
- **Type-3 near-miss clones** — `--max-gap-lines N` merges a copy with a few inserted or changed lines into one `similar` clone with a similarity score; `--similarity 0.85` compares whole JavaScript/TypeScript functions by syntax-tree structure, catching renames and scattered edits too (see [docs](docs/rust.md#type-3-clones-near-miss-merging-with---max-gap-lines))
- **Clone kinds everywhere** — `exact`, `renamed` or `similar` in the console, JSON (`kind`, `similarity`, `method`), XML, HTML, Xcode, SARIF (`jscpd/duplicate-code`, `jscpd/renamed-code`, `jscpd/similar-code`) and Code Climate output; default runs still report only `exact` clones
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
- **`--dashboard`** — the whole picture on one screen, under the health badge: project size, duplication by clone kind with the most duplicated files, the most complex files, and dead code by category for JavaScript, TypeScript and Python (see [docs](docs/rust.md#dashboard))

### AI and operations

- **`--mcp`** — built-in MCP server over stdio with fully described tools: point your AI assistant at the binary and it can check snippets for duplication against your codebase, or find structurally similar functions with a `similarity` argument (see [docs](docs/ai-ready.md#stdio-transport-rust-v5))
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
| [cpd-finder](rust/crates/cpd-finder) | [crates.io](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame — the library entry point |
| [cpd-reporter](rust/crates/cpd-reporter) | [crates.io](https://crates.io/crates/cpd-reporter) | Output formatting (15 reporters, duplication and dead code) |
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

### Recommended aggregator settings

Super Linter and MegaLinter both run jscpd with `threshold: 0` by default, which
fails a build on any duplication at all, including clones that were already
there. That is the most common reason projects end up setting
`VALIDATE_JSCPD: false`. Gate on *new* duplication instead:

```bash
jscpd . --baseline-from-ref origin/main --fail-on-new-clones 0
```

`--baseline-from-ref` scans the base ref's tree with the same configuration and
reports only the clones that are absent from it, so a pull request is judged on
what it adds rather than on the state of the repository. Note that jscpd needs
the whole tree to find clones, so `VALIDATE_ALL_CODEBASE: false` does not narrow
the scan; the baseline is what narrows the result.

When several linters write into one reports directory, give jscpd its own file
name so parallel runs cannot overwrite each other:

```bash
jscpd . --reporters json --output reports --report-name megalinter-jscpd
# -> reports/megalinter-jscpd.json
```

`--report-name` sets the base name for the `json`, `xml`, `csv`, `html`,
`markdown` and `sarif` reports, and defaults to `jscpd-report`. The badge,
OpenMetrics and CodeClimate outputs keep their own names, since GitLab expects
`gl-code-quality-report.json` at that exact path. It can also be set as
`reportName` in `.jscpd.json`.

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
| [`codebase-refactoring`](skills/codebase-refactoring/SKILL.md) | Broader health pass — fix duplication, then remove/refactor dead code, then simplify the biggest/most complex files, prioritized from `--health` | `npx skills add kucherenko/jscpd --skill codebase-refactoring` |

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results — or "clean up this codebase" for the broader pass.

### MCP Server

`jscpd --mcp /path/to/project` scans once and serves the Model Context Protocol over stdio, so an assistant can check any snippet for duplication against the codebase on demand, list a file's clones, re-scan the working directory, and look for structurally similar functions by passing `similarity`.

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
