# jscpd

[![npm version](https://img.shields.io/npm/v/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![npm downloads](https://img.shields.io/npm/dm/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
[![Crates.io Version](https://img.shields.io/crates/v/jscpd?color=green)](https://crates.io/crates/jscpd)
![NPM License](https://img.shields.io/npm/l/jscpd)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/rust.yml)
[![Socket Badge](https://socket.dev/api/badge/npm/package/jscpd)](https://socket.dev/npm/package/jscpd)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kucherenko/jscpd/badge)](https://scorecard.dev/viewer/?uri=github.com/kucherenko/jscpd)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14188/badge)](https://www.bestpractices.dev/projects/14188)

> Copy/paste detector for programming source code. 220+ formats, Rust engine, self-contained binary, AI-ready with MCP server and token-efficient reporter.

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
| npm | `npm install -g jscpd` | Installs the `jscpd` command; prebuilt binary, no Node.js at runtime |
| npm (`cpd` command) | `npm install -g cpd` | Same binary, exposed as `cpd` |
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
| [Rust engine](docs/rust.md) | Installation, CLI reference, reporters, baseline, summary, blame, config file |
| [AI-Ready](docs/ai-ready.md) | AI reporter, agent skills, MCP server |
| [Programming API](docs/api.md) | Rust API (`cpd-finder` crate) |
| [CI & Pre-Commit Hooks](docs/ci-and-hooks.md) | GitHub Action, Docker image, pre-commit hooks |
| [Packages](docs/packages.md) | npm packages and crates that make up a release |
| [Supported formats](FORMATS.md) | All 224 formats with their file extensions |

## Features

jscpd v5 is a Rust engine that ships as a self-contained binary — no runtime required — under two npm names ([`jscpd`](https://www.npmjs.com/package/jscpd) installs the `jscpd` command, [`cpd`](https://www.npmjs.com/package/cpd) installs `cpd`), on [crates.io](https://crates.io/crates/jscpd), Homebrew, Nix, Docker, and as a GitHub Action.

- **224 language formats** with cross-format detection (Vue SFC, Svelte, Astro, Markdown) and `--cross-formats` groups to match clones across JavaScript and TypeScript
- **Prebuilt for 8 platforms** — macOS arm64/x64, Linux arm64/x64 (glibc and musl), Windows arm64/x64
- **15 reporters**: `console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`, `sarif`, `codeclimate`, `openmetrics`, `ai`, `xcode`, `threshold`, `silent`
- **Clone baseline** — gate CI on *new* duplication only. `--baseline .jscpd-baseline.json` with `--fail-on-new-clones[=N]` tolerates legacy clones and fails the build on regressions; `--baseline-from-ref origin/main` does the same without a committed file (see [docs](docs/rust.md#baseline))
- **GitLab-ready reporters** — `codeclimate` (`gl-code-quality-report.json`) and `openmetrics` (`jscpd-metrics.txt`) plug into `artifacts:reports`
- **Git blame** with side-by-side author comparison (`--blame --reporters console-full`)
- **`--summary`** — codebase summary: top files and folders by tokens, lines, size, and a complexity estimate — refactoring hotspots straight from the scan (see [docs](docs/rust.md#summary))
- **`--mcp`** — built-in MCP server over stdio: point your AI assistant at the binary and it can check snippets for duplication against your codebase (see [docs](docs/ai-ready.md#stdio-transport-rust-v5))
- **AI reporter** — token-efficient output for LLM pipelines (~79% fewer tokens than console)
- **`--skip-isolated`** — ignore duplication between monorepo folders owned by different teams
- **`--workers`** — control parallelism for file tokenization and detection (default: all CPU cores)
- **Config discovery** — `.jscpd.json`, `.config/jscpd.json`, or the `jscpd` key in `package.json`

See the [Rust docs](docs/rust.md) for the full CLI reference and [`rust/CHANGELOG.md`](rust/CHANGELOG.md) for release notes.

### Looking for v4?

jscpd v4 (TypeScript engine, Node.js API, LevelDB/Redis stores) is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published as `jscpd@4` / the `latest-4` dist-tag. [README-v4.md](README-v4.md) describes it in one page (install, CLI, API, packages, maintenance policy); the same content is at https://jscpd.dev/getting-started/v4.

## Packages

| Package | Registry | Description |
|---------|----------|-------------|
| [jscpd](rust/jscpd) | [npm](https://www.npmjs.com/package/jscpd) | Installs the `jscpd` command (prebuilt binary via platform packages) |
| [cpd](rust) | [npm](https://www.npmjs.com/package/cpd) | Installs the `cpd` command (same binary) |
| [jscpd-\<platform\>](rust/npm) | npm | Platform binary packages pulled in as optional dependencies: `jscpd-darwin-arm64`, `jscpd-darwin-x64`, `jscpd-linux-x64-gnu`, `jscpd-linux-arm64-gnu`, `jscpd-linux-x64-musl`, `jscpd-linux-arm64-musl`, `jscpd-windows-x64-msvc`, `jscpd-windows-arm64-msvc` |
| [jscpd](rust/crates/cpd) | [crates.io](https://crates.io/crates/jscpd) | CLI crate; installs both `jscpd` and `cpd` binaries |
| [cpd-core](rust/crates/cpd-core) | [crates.io](https://crates.io/crates/cpd-core) | Detection algorithm (Rabin-Karp rolling hash), data models |
| [cpd-tokenizer](rust/crates/cpd-tokenizer) | [crates.io](https://crates.io/crates/cpd-tokenizer) | Source code tokenization (224 formats) |
| [cpd-finder](rust/crates/cpd-finder) | [crates.io](https://crates.io/crates/cpd-finder) | File walking, orchestration, git blame — the library entry point |
| [cpd-reporter](rust/crates/cpd-reporter) | [crates.io](https://crates.io/crates/cpd-reporter) | Output formatting (15 reporters) |

## Who Uses jscpd

The `jscpd` npm package is downloaded **10M+ times per month**, and [~5,000 repositories](https://github.com/kucherenko/jscpd/network/dependents) declare it on GitHub's dependents graph.

**Bundled by analysis platforms:**

- [GitHub Super Linter](https://github.com/super-linter/super-linter) — official GitHub linter aggregator, bundles jscpd as its copy/paste detector and runs it by default; 15,500+ workflow files on GitHub reference Super Linter (as of Sep 2026)
- [MegaLinter](https://github.com/oxsecurity/megalinter) — open-source linter aggregator for CI, ships jscpd in every flavor including `ci_light`
- [Codacy](https://www.codacy.com/) — automated code analysis platform, jscpd powers the duplication engine

**Explicitly enabled in Super Linter** (`VALIDATE_JSCPD: true`) **by ~70 public repositories, including:**

- [A2A](https://github.com/a2aproject/A2A) — Google's Agent2Agent protocol
- [Contact Center AI samples](https://github.com/GoogleCloudPlatform/contact-center-ai-samples) — official Google Cloud samples
- [erobs](https://github.com/NSLS2/erobs) — Brookhaven National Laboratory (NSLS-II)
- [HEAL example analyses](https://github.com/uc-cdis/heal-example-analyses) — NIH HEAL data commons
- [Z-Rad](https://github.com/medical-physics-usz/z-rad) — University Hospital Zürich medical physics
- [Qubership Airflow](https://github.com/Netcracker/qubership-airflow) — Netcracker's platform tooling
- [RimSort](https://github.com/RimSort/RimSort), [Drifty](https://github.com/SaptarshiSarkar12/Drifty), [trice](https://github.com/rokath/trice) — community OSS

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

## AI-Ready Features

jscpd integrates into AI-powered workflows through three mechanisms:

### AI Reporter

Token-efficient output for LLM pipelines (~79% fewer tokens than the default console reporter):

```bash
jscpd --reporters ai /path/to/source              # compact clone list
jscpd --reporters ai --summary /path/to/source    # + compact codebase summary
```

### Agent Skills

Two installable skills that teach AI coding assistants how to use jscpd and refactor detected duplications:

| Skill | Purpose | Install |
|-------|---------|---------|
| [`jscpd`](skills/jscpd/SKILL.md) | Tool reference — CLI options, AI reporter format, config syntax | `npx skills add kucherenko/jscpd --skill jscpd` |
| [`dry-refactoring`](skills/dry-refactoring/SKILL.md) | Guided refactoring workflow — read clones, choose strategy, apply, verify | `npx skills add kucherenko/jscpd --skill dry-refactoring` |

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results.

### MCP Server

`jscpd --mcp /path/to/project` scans once and serves the Model Context Protocol over stdio, so an assistant can check any snippet for duplication against the codebase on demand.

See [AI-Ready docs](docs/ai-ready.md) for full details.

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
