# jscpd

[![npm version](https://img.shields.io/npm/v/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![Crates.io Version](https://img.shields.io/crates/v/jscpd?color=green)](https://crates.io/crates/jscpd)
![NPM License](https://img.shields.io/npm/l/jscpd)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml)
[![Socket Badge](https://socket.dev/api/badge/npm/package/jscpd)](https://socket.dev/npm/package/jscpd)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kucherenko/jscpd/badge)](https://scorecard.dev/viewer/?uri=github.com/kucherenko/jscpd)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14188/badge)](https://www.bestpractices.dev/projects/14188)

> Copy/paste detector for programming source code. Supports 220+ formats. AI-ready with MCP server and token-efficient reporter. Now with a Rust-powered engine — 24-37x faster.

**Documentation:** https://jscpd.dev

jscpd implements the [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm to find duplicated code blocks across files.

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
| npm (Rust engine) | `npm install -g jscpd@5` | Installs the `jscpd` command; prebuilt binary, no Node.js at runtime |
| npm (`cpd` command) | `npm install -g cpd` | Same Rust binary, exposed as `cpd` |
| Cargo | `cargo install jscpd` | Builds from crates.io; installs both `jscpd` and `cpd` |
| Homebrew | `brew install jscpd` | macOS / Linux |
| Nix | `nix run github:kucherenko/jscpd -- /path/to/code` | Or `nix profile install github:kucherenko/jscpd` |
| Docker | `docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd .` | Multi-arch image from GitHub Releases; available from the next release |
| npm (TypeScript engine) | `npm install -g jscpd@4` | Node.js engine (v4.x) — needed for the Node.js API and LevelDB/Redis stores |

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
| [TypeScript (v4.x)](docs/typescript.md) | Node.js engine — CLI, reporters, config, detection modes |
| [Rust (v5.x)](docs/rust.md) | Rust engine — installation, CLI, reporters, blame, Rust API |
| [AI-Ready](docs/ai-ready.md) | AI reporter, agent skills, MCP server |
| [Programming API](docs/api.md) | TypeScript and Rust programmatic APIs |
| [CI & Pre-Commit Hooks](docs/ci-and-hooks.md) | GitHub Action, pre-commit hooks |
| [Packages](docs/packages.md) | Monorepo package and crate overview |

## Two Engines

| | TypeScript (v4) | Rust (v5) |
|---|---|---|
| **npm package** | [`jscpd@4`](https://www.npmjs.com/package/jscpd) | [`jscpd@5`](https://www.npmjs.com/package/jscpd) or [`cpd`](https://www.npmjs.com/package/cpd) |
| **CLI command** | `jscpd` | `jscpd` (from `jscpd@5`) or `cpd` (from `cpd`) |
| **Speed** | Baseline | 24-37x faster |
| **Formats** | 224 | 224 |
| **Node.js required** | Yes | No (self-contained binary) |
| **Programming API** | TypeScript (`jscpd()`, `detectClones()`) | Rust (`cpd-finder` crate) |
| **LevelDB store** | Yes | No |
| **Reporters** | 13 | 15 |

`jscpd@5` installs the `jscpd` command. The `cpd` npm package installs the `cpd` command. Both contain the same Rust binary. For both command names from a single install, use [crates.io](https://crates.io/crates/jscpd): `cargo install jscpd`.

## What's New

### v5.x — Rust Engine

jscpd v5 is a ground-up Rust rewrite that ships as [`jscpd@5`](https://www.npmjs.com/package/jscpd) (installs the `jscpd` command) or [`cpd`](https://www.npmjs.com/package/cpd) (installs the `cpd` command). Self-contained binary — no Node.js runtime required.

**Same interface, 24-37x faster:**

- All CLI options from v4 are preserved — drop-in replacement: `jscpd` → `jscpd@5`
- Same `.jscpd.json` config file, same detection algorithm, same reporters
- 224 language formats with cross-format detection (Vue SFC, Svelte, Astro, Markdown)

**New in 5.1:**

- **Clone baseline** — gate CI on *new* duplication only. `--baseline .jscpd-baseline.json` with `--fail-on-new-clones[=N]` tolerates legacy clones and fails the build on regressions; `--update-baseline` rewrites the file. `--baseline-from-ref origin/main` does the same without a committed file by scanning the base ref in a temporary worktree (see [docs](docs/rust.md#baseline))
- **OpenMetrics reporter** (`--reporters openmetrics`) — `jscpd-metrics.txt` for GitLab `artifacts:reports:metrics`
- **CodeClimate / GitLab Code Quality reporter** (`--reporters codeclimate`, alias `gitlab`) — `gl-code-quality-report.json` for GitLab `artifacts:reports:codequality`
- **Windows on ARM** — native `windows-arm64-msvc` binary
- **Config discovery in `.config/`** — `.config/jscpd.json` per the dot-config convention
- **Unknown `--format` values warn** instead of silently scanning nothing

**New in 5.0:**

- **24-37x faster** detection on real projects (see [benchmark](docs/performance-comparison.md))
  - Small codebases (548 files): 34x faster
  - Medium codebases (9K files): 37x faster
  - Large codebases (17K files, 900 MB): 24x faster
- **Git blame** with side-by-side author comparison (`--blame --reporters console-full`)
- **`--workers`** — control parallelism for file tokenization and detection (default: auto, uses all CPU cores; not available in v4)
- **15 reporters**: `console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`, `sarif`, `codeclimate`, `openmetrics`, `ai`, `xcode`, `threshold`, `silent`
- **AI reporter** — token-efficient output for LLM pipelines (~79% fewer tokens than console)
- **`--mcp`** — built-in MCP server over stdio: point your AI assistant at the binary and it can check snippets for duplication against your codebase (see [docs](docs/ai-ready.md#stdio-transport-rust-v5))
- **`--summary`** — codebase summary: top files and folders by tokens, lines, size, and a complexity estimate — refactoring hotspots straight from the scan (see [docs](docs/rust.md#summary))
- **`--cross-formats`** and **`--skip-isolated`** — detect clones across JS/TS, or ignore duplication between monorepo folders owned by different teams
- **Self-contained binary** — prebuilt for 8 platforms: macOS arm64/x64, Linux arm64/x64 (glibc and musl), Windows arm64/x64

**Not yet in v5** (use v4 for these):

- LevelDB/Redis stores (`--store leveldb`)
- Node.js programming API (`jscpd()`, `detectClones()`)

See [Rust docs](docs/rust.md) for the full CLI reference and differences from v4.

### v4.2.x — TypeScript Engine

- **Custom tokenizer backend** — replaced `prismjs` with own backend built on [reprism](https://github.com/tannerlinsley/reprism). ~11.5% faster tokenization on real projects
- **Cross-format detection** — Vue SFC, Svelte, Astro, and Markdown tokenized per-block, enabling detection across file types
- **New formats**: Apex, CFML/ColdFusion, GDScript, and 70+ additional formats (224 total, up from 152)
- **Shebang detection** — auto-detect language for extensionless scripts
- **`--store-path`** — configure LevelDB cache directory for parallel runs
- **`--skipComments`** — shorthand for `--mode weak`
- **`--formats-names`** — map filenames (e.g. `Makefile`, `Dockerfile`) to formats
- **`--noTips`** — suppress tip output in CI
- **Bug fixes**: entire-file duplicates silently dropped (#728), ReDoS on Lisp/Elisp files (#737), process crash on malformed `package.json` (#739), Vue SFC cross-file detection (#737), Vue SFC column numbers (#737), 50 dependency security vulnerabilities

See [TypeScript docs](docs/typescript.md) for the full CLI reference.

## Packages

| Package | Description |
|---------|-------------|
| [jscpd](apps/jscpd) | CLI and Node.js API (v4.x) |
| [jscpd-server](apps/jscpd-server) | REST API + MCP server |
| [@jscpd/core](packages/core) | Core detection algorithm |
| [@jscpd/finder](packages/finder) | File detection, reporters |
| [@jscpd/tokenizer](packages/tokenizer) | Source code tokenization |
| [@jscpd/html-reporter](packages/html-reporter) | HTML report |
| [@jscpd/badge-reporter](packages/badge-reporter) | SVG badge |
| [jscpd-sarif-reporter](packages/sarif-reporter) | SARIF (GitHub Code Scanning) |
| [@jscpd/leveldb-store](packages/leveldb-store) | LevelDB persistent store |
| [@jscpd/redis-store](packages/redis-store) | Redis distributed store |
| [cpd](rust) (Rust engine) | Rust-powered engine (v5.x) — also available as `jscpd@5` |

## Who Uses jscpd

The `jscpd` npm package is downloaded **10M+ times per month**, and [~5,000 repositories](https://github.com/kucherenko/jscpd/network/dependents) declare it on GitHub's dependents graph.

**Bundled by analysis platforms:**

- [GitHub Super Linter](https://github.com/super-linter/super-linter) — official GitHub linter aggregator, bundles jscpd as its copy/paste detector
- [MegaLinter](https://github.com/oxsecurity/megalinter) — open-source linter aggregator for CI, ships jscpd in every flavor including `ci_light`
- [Codacy](https://www.codacy.com/) — automated code analysis platform, jscpd powers the duplication engine

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

## Performance

Benchmarked on macOS (Apple Silicon), 10 runs per target (3 for CopilotKit). v4 ran with `--no-gitignore -i "node_modules"` to ensure comparable file scanning.

| Target | Files | Size | jscpd v4 | jscpd v5 | Speedup |
|--------|-------|------|----------|----------|---------|
| fixtures | 548 | 1.5 MB | 1.03s | 0.03s | **34.3x** |
| svelte | 9K | 38 MB | 15.80s | 0.43s | **36.9x** |
| CopilotKit | 17K | 159 MB | 82.89s | 3.44s | **24.1x** |

See [performance-comparison.md](docs/performance-comparison.md) for full methodology and raw data.

## AI-Ready Features

jscpd integrates into AI-powered workflows through three mechanisms:

### AI Reporter

Token-efficient output for LLM pipelines (~79% fewer tokens than the default console reporter):

```bash
jscpd --reporters ai /path/to/source              # v4
cpd --reporters ai /path/to/source                # v5
cpd --reporters ai --summary /path/to/source      # v5: + compact codebase summary
```

### Agent Skills

Two installable skills that teach AI coding assistants how to use jscpd and refactor detected duplications:

| Skill | Purpose | Install |
|-------|---------|---------|
| `jscpd` | Tool reference — CLI options, AI reporter format, config syntax | `npx skills add kucherenko/jscpd --skill jscpd` |
| `dry-refactoring` | Guided refactoring workflow — read clones, choose strategy, apply, verify | `npx skills add kucherenko/jscpd --skill dry-refactoring` |

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results.

See [AI-Ready docs](docs/ai-ready.md) for full details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, test policy, and pull request requirements. In short:

```bash
# Rust engine (v5, active development)
cd rust && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check

# TypeScript packages (v4, maintenance)
pnpm install && pnpm build && pnpm test
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
