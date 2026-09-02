# jscpd v4

> This is the maintenance branch for **jscpd v4** (TypeScript engine). The current major version, v5 (Rust engine), lives on [`master`](https://github.com/kucherenko/jscpd) and is documented at https://jscpd.dev. Use v4 if you need the Node.js programming API, the LevelDB/Redis stores, or a pure-Node.js install.

[![npm version (latest-4)](https://img.shields.io/npm/v/jscpd/latest-4?color=brightgreen)](https://www.npmjs.com/package/jscpd/v/latest-4)
[![npm downloads](https://img.shields.io/npm/dm/jscpd?color=brightgreen)](https://www.npmjs.com/package/jscpd)
![NPM License](https://img.shields.io/npm/l/jscpd)
[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml/badge.svg?branch=master-v4)](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml?query=branch%3Amaster-v4)
[![Socket Badge](https://socket.dev/api/badge/npm/package/jscpd)](https://socket.dev/npm/package/jscpd)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kucherenko/jscpd/badge)](https://scorecard.dev/viewer/?uri=github.com/kucherenko/jscpd)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14188/badge)](https://www.bestpractices.dev/projects/14188)

Copy/paste detector for programming source code, written in TypeScript and running on Node.js. Supports [224 formats](FORMATS.md), ships 13 reporters (console, JSON, XML, CSV, Markdown, HTML, SVG badge, SARIF, AI, Xcode, …), a Node.js programming API, and LevelDB/Redis stores for very large codebases.

jscpd implements the [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm to find duplicated code blocks across files.

## Installation

```bash
# Global install
npm install -g jscpd@4

# No install — run once with npx
npx jscpd@4 .
```

`jscpd@4` resolves to the newest 4.x release (npm dist-tag `latest-4`). Plain `npm install -g jscpd` installs v5. Requires Node.js 20 or newer.

## Quick Start

```bash
# Scan a project
jscpd /path/to/code

# Fail when more than 5% of the code is duplicated, write a JSON + HTML report
jscpd --threshold 5 --reporters console,json,html --output report ./src
```

```
Clone found (typescript):
 - src/utils/auth.ts [10:1 - 25:2] (15 lines, 112 tokens)
   src/utils/helpers.ts [40:1 - 55:2]

┌────────────┬────────────────┬─────────────┬──────────────┬──────────────┬──────────────────┬───────────────────┐
│ Format     │ Files analyzed │ Total lines │ Total tokens │ Clones found │ Duplicated lines │ Duplicated tokens │
├────────────┼────────────────┼─────────────┼──────────────┼──────────────┼──────────────────┼───────────────────┤
│ typescript │ 42             │ 3812        │ 27540        │ 3            │ 48 (1.26%)       │ 336 (1.22%)       │
└────────────┴────────────────┴─────────────┴──────────────┴──────────────┴──────────────────┴───────────────────┘
```

## Documentation

| Document | Description |
|----------|-------------|
| [TypeScript CLI reference](docs/typescript.md) | All CLI options, reporters, config file, detection modes, formats |
| [Programming API](docs/api.md) | `jscpd()`, `detectClones()`, custom stores and reporters |
| [AI-Ready](docs/ai-ready.md) | `ai` reporter and the `jscpd-server` MCP server |
| [CI & Pre-Commit Hooks](docs/ci-and-hooks.md) | GitHub Actions, GitLab CI, pre-commit, Husky |
| [Packages](docs/packages.md) | Workspace package overview |
| [FORMATS.md](FORMATS.md) | The 224 supported formats |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## CLI

```bash
jscpd [options] <path ...>
```

The most used options — see the [full reference](docs/typescript.md#options) for all of them:

| Option | Description | Default |
|--------|-------------|---------|
| `-l, --min-lines` | Minimum lines in a clone | 5 |
| `-k, --min-tokens` | Minimum tokens in a clone | 50 |
| `-t, --threshold` | Duplication percentage threshold — exit 1 if exceeded | — |
| `-r, --reporters` | Comma-separated reporters | `time,console` |
| `-o, --output` | Output directory for file reporters | `./report/` |
| `-m, --mode` | Detection mode: `strict`, `mild`, `weak` | `mild` |
| `-f, --format` | Formats to check (comma-separated) | all detected |
| `-i, --ignore` | Glob patterns to exclude | — |
| `-p, --pattern` | Glob pattern for file search | — |
| `--gitignore` / `--no-gitignore` | Respect `.gitignore` files | on |
| `--store` | `leveldb` for large repositories | memory |
| `-b, --blame` | Enrich clones with git blame author data | off |
| `--skipLocal` | Skip clones within the same directory | off |
| `--exitCode` | Exit code when clones are detected | — |
| `--noTips` | Suppress tips (useful in CI) | off |
| `--list` | List all supported formats | — |

### Configuration

Create `.jscpd.json` in the project root (or put the same keys under `"jscpd"` in `package.json`):

```json
{
  "path": ["./src"],
  "reporters": ["console", "json", "html"],
  "minLines": 5,
  "minTokens": 50,
  "threshold": 5,
  "format": ["javascript", "typescript"],
  "ignore": ["**/node_modules/**", "**/dist/**"],
  "gitignore": true,
  "mode": "mild"
}
```

### Reporters

| Reporter | Output |
|----------|--------|
| `console` | Clone list with per-format statistics table |
| `consoleFull` | Full source snippets for each clone |
| `json` | `report/jscpd-report.json` |
| `xml` | `report/jscpd-report.xml` (PMD CPD format) |
| `csv` | `report/jscpd-report.csv` |
| `markdown` | `report/jscpd-report.md` |
| `html` | Interactive HTML report in `report/html/` |
| `badge` | SVG badge `report/jscpd-badge.svg` |
| `sarif` | `report/jscpd-sarif.json` for GitHub Code Scanning |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication exceeds `--threshold` |
| `silent` | No console output |

Third-party reporters are loaded by npm package name (e.g. `jscpd-full-reporter`).

### Detection modes

| Mode | Behavior |
|------|----------|
| `strict` | All tokens must match (including whitespace, newlines) |
| `mild` | Ignore empty and newline tokens |
| `weak` | Ignore comments, empty tokens, and newlines (`--skipComments` is an alias) |

### Formats

224 formats are recognised ([FORMATS.md](FORMATS.md), or `jscpd --list`). Vue SFC, Svelte, Astro and Markdown files are tokenized per block, so a `<script>` block in a `.vue` file can match a `.ts` file. Extensionless scripts are detected by shebang, and `--formats-exts` / `--formats-names` map custom extensions and filenames (e.g. `Makefile`, `Dockerfile`) to formats.

## Programming API

```typescript
import { detectClones } from 'jscpd';

const clones = await detectClones({
  path: ['./src'],
  silent: true,
  format: ['javascript', 'typescript'],
  minLines: 5,
  minTokens: 50,
});
```

```typescript
import { IClone } from '@jscpd/core';
import { jscpd } from 'jscpd';

// argv-style, same options as the CLI
const clones: IClone[] = await jscpd(['', '', './src', '-m', 'weak', '--silent']);
```

Pass a store as the second argument of `detectClones` to reuse token maps between runs (`MemoryStore` from `@jscpd/core`, `LevelDBStore` from `@jscpd/leveldb-store`). See the [API docs](docs/api.md) and [`examples/api`](examples/api).

## Packages

| Package | Description |
|---------|-------------|
| [jscpd](apps/jscpd) | CLI and Node.js API |
| [jscpd-server](apps/jscpd-server) | REST API + MCP server (Streamable HTTP) |
| [@jscpd/core](packages/core) | Core detection algorithm (Rabin-Karp), interfaces, `MemoryStore` |
| [@jscpd/finder](packages/finder) | File discovery, detection orchestration, built-in reporters |
| [@jscpd/tokenizer](packages/tokenizer) | Source code tokenization (224 formats) |
| [@jscpd/html-reporter](packages/html-reporter) | HTML report |
| [@jscpd/badge-reporter](packages/badge-reporter) | SVG badge |
| [jscpd-sarif-reporter](packages/sarif-reporter) | SARIF (GitHub Code Scanning) |
| [@jscpd/leveldb-store](packages/leveldb-store) | LevelDB persistent store |
| [@jscpd/redis-store](packages/redis-store) | Redis distributed store |

All packages are published to npm from this branch; see [docs/packages.md](docs/packages.md).

## CI and pre-commit

Run the CLI with `npx` in any CI system that has Node.js:

```yaml
# GitHub Actions
- uses: actions/setup-node@v4
  with:
    node-version: 22
- run: npx jscpd@4 --threshold 5 --reporters console,sarif --output report .
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: report/jscpd-sarif.json
```

The `kucherenko/jscpd@v5` GitHub Action and the Docker image install the v5 engine, not v4 — use the `npx jscpd@4` form above when you need the Node.js engine.

Pre-commit hook via the [pre-commit](https://pre-commit.com) framework:

```yaml
repos:
  - repo: local
    hooks:
      - id: jscpd
        name: jscpd - copy/paste detector
        entry: jscpd
        language: node
        additional_dependencies: ['jscpd@4']
        args: [--threshold, "5", --reporters, console,silent]
        pass_filenames: false
        always_run: true
```

Husky, plain git hooks, GitLab CI and PR-comment workflows are covered in [docs/ci-and-hooks.md](docs/ci-and-hooks.md).

## AI-ready

- `--reporters ai` prints a compact clone list (roughly 79% fewer tokens than the console reporter) for LLM pipelines and coding agents.
- [`jscpd-server`](apps/jscpd-server) serves the detector as MCP tools over Streamable HTTP plus a REST API, so an assistant can check a snippet against your codebase on demand.

See [docs/ai-ready.md](docs/ai-ready.md).

## Changes in 4.x

### 4.3.0

- **Color auto-detection** — ANSI colors are disabled when stdout is not a TTY, with `--colors` / `--no-colors` flags and a `colors` config key to override (`FORCE_COLOR` / `NO_COLOR` respected) (#893, #899)
- **jscpd-server: MCP protocol revision 2026-07-28** — official MCP SDK v2, Origin/Host allowlists (`--allowed-origin` / `--allowed-host`), loopback default bind (#902)
- **Bug fixes**: `consoleFull` printed clones twice (#900), off-by-one source line counts (#881), server log colors
- **Security**: all open Dependabot alerts on transitive dependencies resolved via `pnpm-workspace.yaml` overrides; CI installs with `--frozen-lockfile`

### 4.2.x

- **Custom tokenizer backend** — replaced `prismjs` with an own backend built on [reprism](https://github.com/tannerlinsley/reprism)
- **Cross-format detection** — Vue SFC, Svelte, Astro, and Markdown tokenized per-block, enabling detection across file types
- **New formats**: Apex, CFML/ColdFusion, GDScript, and 70+ additional formats (224 total, up from 152)
- **Shebang detection** — auto-detect language for extensionless scripts
- **`--store-path`** — configure LevelDB cache directory for parallel runs
- **`--skipComments`** — shorthand for `--mode weak`
- **`--formats-names`** — map filenames (e.g. `Makefile`, `Dockerfile`) to formats
- **`--noTips`** — suppress tip output in CI
- **Bug fixes**: entire-file duplicates silently dropped (#728), ReDoS on Lisp/Elisp files (#737), process crash on malformed `package.json` (#739), Vue SFC cross-file detection (#737), Vue SFC column numbers (#737), 50 dependency security vulnerabilities

The full history is in [CHANGELOG.md](CHANGELOG.md).

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

## Contributing

This branch receives security and critical fixes for the 4.x line; new features go to v5 on `master`. Pull requests for v4 must target `master-v4` — see [CONTRIBUTING.md](CONTRIBUTING.md). In short:

```bash
pnpm install && pnpm build && pnpm lint && pnpm test
node apps/jscpd/bin/jscpd ./fixtures --reporters console,json --output report   # smoke test
pnpm changeset                                                                  # describe your change
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
