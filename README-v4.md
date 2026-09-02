# jscpd v4 (TypeScript engine)

> jscpd v4 is the Node.js version of the copy/paste detector. It is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and published to npm under the `latest-4` dist-tag. This branch (`master`) holds jscpd v5, the Rust engine documented at https://jscpd.dev. A web version of this page lives at https://jscpd.dev/getting-started/v4.

[![npm version (latest-4)](https://img.shields.io/npm/v/jscpd/latest-4?color=brightgreen)](https://www.npmjs.com/package/jscpd/v/latest-4)
[![jscpd v4 CI](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml/badge.svg?branch=master-v4)](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml?query=branch%3Amaster-v4)

Both versions read the same `.jscpd.json`, run the same Rabin-Karp detection algorithm, and produce the same report formats. They differ in how they are built and what surrounds the core:

| | v4 (TypeScript) | v5 (Rust) |
|---|---|---|
| Runtime | Node.js 20+ | Self-contained binary, no runtime |
| Install | `npm install -g jscpd@4` | install script, npm, cargo, Homebrew, Nix, Docker |
| Programming API | Node.js (`jscpd()`, `detectClones()`) | Rust crates |
| Token cache for large repositories | LevelDB / Redis stores | Not needed |
| MCP server | `jscpd-server` package (Streamable HTTP + REST) | built-in `jscpd --mcp` (stdio) |
| Reporters | 13 | 15 (adds `openmetrics`, `codeclimate`) |
| Baseline mode (`--fail-on-new-clones`) | No | Yes |
| GitHub Action `kucherenko/jscpd@v5` | No (use `npx jscpd@4`) | Yes |
| Formats | 224 | 224 |

## When to use v4

- You call jscpd from Node.js code through the programming API.
- You run `jscpd-server` as a REST or MCP endpoint.
- You rely on the LevelDB or Redis store to share token maps between runs.
- Your environment can run Node.js packages but cannot execute prebuilt native binaries.

If none of these apply, use v5. The [migration guide](https://jscpd.dev/getting-started/migration) lists every flag and config difference.

## Installation

```bash
# Global install (newest 4.x release)
npm install -g jscpd@4

# No install — run once with npx
npx jscpd@4 .
```

`jscpd@4` resolves to the newest 4.x release through the `latest-4` dist-tag. Plain `npm install -g jscpd` installs v5. Node.js 20 or newer is required.

## Quick start

```bash
# Scan a project
jscpd /path/to/code

# Fail when more than 5% of the code is duplicated, write JSON and HTML reports
jscpd --threshold 5 --reporters console,json,html --output report ./src
```

## CLI

```bash
jscpd [options] <path ...>
```

The most used options. The [full reference](https://github.com/kucherenko/jscpd/blob/master-v4/docs/typescript.md#options) on the `master-v4` branch lists all of them.

| Option | Description | Default |
|--------|-------------|---------|
| `-l, --min-lines` | Minimum lines in a clone | 5 |
| `-k, --min-tokens` | Minimum tokens in a clone | 50 |
| `-t, --threshold` | Duplication percentage threshold, exit 1 if exceeded | — |
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

Create `.jscpd.json` in the project root, or put the same keys under a `"jscpd"` key in `package.json`:

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
| `console` | Clone list with a per-format statistics table |
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

Third-party reporters are loaded by npm package name.

### Detection modes

| Mode | Behavior |
|------|----------|
| `strict` | All tokens must match, including whitespace and newlines |
| `mild` | Ignore empty and newline tokens |
| `weak` | Ignore comments, empty tokens, and newlines (`--skipComments` is an alias) |

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

Pass a store as the second argument of `detectClones` to reuse token maps between runs: `MemoryStore` from `@jscpd/core` or `LevelDBStore` from `@jscpd/leveldb-store`. The [API docs](https://github.com/kucherenko/jscpd/blob/master-v4/docs/api.md) and [`examples/api`](https://github.com/kucherenko/jscpd/tree/master-v4/examples/api) on the `master-v4` branch have complete examples.

## Packages

| Package | Description |
|---------|-------------|
| [jscpd](https://www.npmjs.com/package/jscpd) | CLI and Node.js API |
| [jscpd-server](https://www.npmjs.com/package/jscpd-server) | REST API and MCP server (Streamable HTTP) |
| [@jscpd/core](https://www.npmjs.com/package/@jscpd/core) | Core detection algorithm, interfaces, `MemoryStore` |
| [@jscpd/finder](https://www.npmjs.com/package/@jscpd/finder) | File discovery, detection orchestration, built-in reporters |
| [@jscpd/tokenizer](https://www.npmjs.com/package/@jscpd/tokenizer) | Source code tokenization (224 formats) |
| [@jscpd/html-reporter](https://www.npmjs.com/package/@jscpd/html-reporter) | HTML report |
| [@jscpd/badge-reporter](https://www.npmjs.com/package/@jscpd/badge-reporter) | SVG badge |
| [jscpd-sarif-reporter](https://www.npmjs.com/package/jscpd-sarif-reporter) | SARIF for GitHub Code Scanning |
| [@jscpd/leveldb-store](https://www.npmjs.com/package/@jscpd/leveldb-store) | LevelDB persistent store |
| [@jscpd/redis-store](https://www.npmjs.com/package/@jscpd/redis-store) | Redis distributed store |

All of them are published from the `master-v4` branch.

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

The `kucherenko/jscpd@v5` GitHub Action and the Docker image install the v5 engine. Use the `npx jscpd@4` form when you need the Node.js engine.

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

## AI tooling

- `--reporters ai` prints a compact clone list for LLM pipelines and coding agents.
- `jscpd-server` exposes the detector as MCP tools over Streamable HTTP plus a REST API, so an assistant can check a snippet against your codebase on demand.

## Maintenance policy

- v4 receives bug fixes and security fixes on `master-v4`. New features land in v5.
- Bug reports for v4 go to the shared [issue tracker](https://github.com/kucherenko/jscpd/issues); pick "v4 (TypeScript)" in the engine field.
- Pull requests for v4 must target the `master-v4` branch.
- Full v4 documentation: [README](https://github.com/kucherenko/jscpd/blob/master-v4/README.md), [CLI reference](https://github.com/kucherenko/jscpd/blob/master-v4/docs/typescript.md), [changelog](https://github.com/kucherenko/jscpd/blob/master-v4/CHANGELOG.md).
