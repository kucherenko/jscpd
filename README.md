# jscpd

[![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
[![license](https://img.shields.io/github/license/kucherenko/jscpd.svg?style=flat-square)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml)

[![NPM](https://nodei.co/npm/jscpd.svg)](https://nodei.co/npm/jscpd/)

> Copy/paste detector for programming source code. Supports 224+ formats. AI-ready with MCP server and token-efficient reporter. Now with a Rust-powered engine — 24-37x faster.

jscpd implements the [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm to find duplicated code blocks across files.

## Quick Start

```bash
# Install (all platforms — installs both jscpd and cpd commands)
curl -fsSL https://jscpd.dev/install.sh | bash

# TypeScript engine (Node.js, v4.x)
npm install -g jscpd@4
jscpd /path/to/code
# or use without installing
npx jscpd@4 /path/to/code

# Rust engine (v5.x, 24-37x faster) — both jscpd and cpd commands
npm install -g jscpd@5
jscpd /path/to/code
cpd /path/to/code

# Rust engine — cpd command only
npm install -g cpd
cpd /path/to/code

# Rust-native install (exposes both jscpd and cpd)
cargo install jscpd
```

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
| **CLI command** | `jscpd` | `jscpd` and `cpd` (both available) |
| **Speed** | Baseline | 24-37x faster |
| **Formats** | 224 | 223 |
| **Node.js required** | Yes | No (self-contained binary) |
| **Programming API** | TypeScript (`jscpd()`, `detectClones()`) | Rust (`cpd-finder` crate) |
| **LevelDB store** | Yes | No |
| **Reporters** | 13 | 13 |

`jscpd@5` installs both `jscpd` and `cpd` commands. The `cpd` npm package installs only the `cpd` command. Both contain the same Rust binary.

## What's New

### v5.0.x — Rust Engine

jscpd v5 is a ground-up Rust rewrite that ships as [`jscpd@5`](https://www.npmjs.com/package/jscpd) (installs both `jscpd` and `cpd` commands) or [`cpd`](https://www.npmjs.com/package/cpd) (installs the `cpd` command only). Self-contained binary — no Node.js runtime required.

**Same interface, 24-37x faster:**

- All CLI options from v4 are preserved — drop-in replacement: `jscpd` → `jscpd@5`
- Same `.jscpd.json` config file, same detection algorithm, same reporters
- 223 language formats with cross-format detection (Vue SFC, Svelte, Astro, Markdown)

**New in v5:**

- **24-37x faster** detection on real projects (see [benchmark](docs/performance-comparison.md))
  - Small codebases (548 files): 34x faster
  - Medium codebases (9K files): 37x faster
  - Large codebases (17K files, 900 MB): 24x faster
- **Git blame** with side-by-side author comparison (`--blame --reporters console-full`) — uses [gitoxide](https://github.com/GitoxideLabs/gitoxide) instead of shelling out to `git`, making blame significantly faster
- **`--workers`** — control parallelism for file tokenization and detection (default: auto, uses all CPU cores; not available in v4)
- **13 reporters**: `console`, `console-full`, `json`, `xml`, `csv`, `html`, `markdown`, `badge`, `sarif`, `ai`, `xcode`, `threshold`, `silent`
- **AI reporter** — token-efficient output for LLM pipelines (~79% fewer tokens than console)
- **Self-contained binary** — prebuilt for 6 platforms (macOS arm64/x64, Linux arm64/x64, Windows x64)

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

- [GitHub Super Linter](https://github.com/super-linter/super-linter) — official GitHub linter aggregator, bundles jscpd as its copy/paste detector
- [Codacy](https://www.codacy.com/) — automated code analysis platform, jscpd powers the duplication engine
- [MegaLinter](https://github.com/oxsecurity/megalinter) — 100% open-source linter aggregator for CI, integrates jscpd
- [OpenClaw](https://github.com/openclaw/openclaw) — personal AI assistant for self-hosted devices
- [Natural](https://github.com/NaturalNode/natural) — NLP library for Node.js, uses jscpd for code quality
- [Nixpkgs](https://github.com/NixOS/nixpkgs) — 140K+ package repo for NixOS, packages jscpd

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
jscpd --reporters ai /path/to/source   # v4
cpd --reporters ai /path/to/source      # v5
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

1. Fork the repo [kucherenko/jscpd](https://github.com/kucherenko/jscpd/)
2. Clone forked version (`git clone https://github.com/{your-id}/jscpd`)
3. Install dependencies (`pnpm install`)
4. Run in dev mode: `pnpm dev`
5. Add your changes
6. Add tests and check: `pnpm test`
7. Build: `pnpm build`
8. Create PR

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

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
