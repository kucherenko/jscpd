# jscpd

![stand with Ukraine](https://badgen.net/badge/support/UKRAINE/?color=0057B8&labelColor=FFD700)

[![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
![jscpd](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/jscpd-badge.svg?sanitize=true)
[![license](https://img.shields.io/github/license/kucherenko/jscpd.svg?style=flat-square)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![npm](https://img.shields.io/npm/dw/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)


[![jscpd CI](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml/badge.svg)](https://github.com/kucherenko/jscpd/actions/workflows/nodejs.yml)
[![codecov](https://codecov.io/gh/kucherenko/jscpd/branch/master/graph/badge.svg)](https://codecov.io/gh/kucherenko/jscpd)
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd?ref=badge_shield)
[![Backers on Open Collective](https://opencollective.com/jscpd/backers/badge.svg)](#backers)
[![Sponsors on Open Collective](https://opencollective.com/jscpd/sponsors/badge.svg)](#sponsors)

[![NPM](https://nodei.co/npm/jscpd.svg)](https://nodei.co/npm/jscpd/)

> Copy/paste detector for programming source code, supports 225 formats. AI-ready with AI skills, MCP server and token-efficient reporter. Now with a Rust-powered engine.

Copy/paste is a common technical debt on a lot of projects. The jscpd gives the ability to find duplicated blocks implemented on more than 225 programming languages and digital formats of documents.
The jscpd tool implements [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm for searching duplications.

## Packages of jscpd

| name                 | version  |  description  |
|----------------------|----------|---------------|
| [jscpd](apps/jscpd) | [![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd) | main package for jscpd (cli and API for detections included) |
| [jscpd-server](apps/jscpd-server) | [![npm](https://img.shields.io/npm/v/jscpd-server.svg?style=flat-square)](https://www.npmjs.com/package/jscpd-server) | jscpd server application |
| [@jscpd/core](packages/core) | [![npm](https://img.shields.io/npm/v/@jscpd/core.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/core) |core detection algorithm, can be used for detect duplication in different environments, one dependency to eventemitter3 |
| [@jscpd/finder](packages/finder) | [![npm](https://img.shields.io/npm/v/@jscpd/finder.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/finder) | detector of duplication in files  |
| [@jscpd/tokenizer](packages/tokenizer) | [![npm](https://img.shields.io/npm/v/@jscpd/tokenizer.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/tokenizer) | tool for tokenize programming source code |
| [@jscpd/leveldb-store](packages/leveldb-store) | [![npm](https://img.shields.io/npm/v/@jscpd/leveldb-store.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/leveldb-store) | LevelDB store, used for big repositories, slower than default store |
| [@jscpd/html-reporter](packages/html-reporter) | [![npm](https://img.shields.io/npm/v/@jscpd/html-reporter.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/html-reporter) | Html reporter for jscpd |
| [@jscpd/badge-reporter](packages/badge-reporter) | [![npm](https://img.shields.io/npm/v/@jscpd/badge-reporter.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/badge-reporter) | Badge reporter for jscpd |
| [jscpd-sarif-reporter](packages/sarif-reporter) | [![npm](https://img.shields.io/npm/v/jscpd-sarif-reporter.svg?style=flat-square)](https://www.npmjs.com/package/jscpd-sarif-reporter) | SARIF reporter for jscpd (GitHub Code Scanning compatible) |
| [cpd](rust) (Rust engine) | [![npm](https://img.shields.io/npm/v/cpd.svg?style=flat-square)](https://www.npmjs.com/package/cpd) | Rust-powered engine: 10-30x faster detection, 225+ formats, git blame, 13 reporters |

## AI-Ready

jscpd integrates into AI-powered development workflows through three complementary mechanisms.

### AI Reporter

The `ai` reporter produces compact, token-efficient output designed to be piped directly into an LLM prompt or agentic pipeline. It uses common-path-prefix compression and omits code fragments and colors — just the clone locations and a summary.

```bash
jscpd --reporters ai /path/to/source
```

Example output:
```
src/utils/ auth.ts:10-25 ~ helpers.ts:40-55
src/utils/auth.ts 30-45 ~ 80-95
src/ utils/auth.ts:10-25 ~ api/routes.ts:5-20
---
23 clones · 4.2% duplication
```

Benchmarked on the `fixtures/` directory (91 clones, 132 files):

| Reporter | Output size | Estimated tokens |
|----------|-------------|------------------|
| default (console) | ~21,800 chars | ~5,400 |
| `ai` | ~4,500 chars | ~1,100 |

~79% fewer tokens than the default console reporter.

### Agent Skills

jscpd ships two AI agent skills that teach coding assistants how to use jscpd and refactor detected duplications:

**jscpd** — tool reference skill. Covers all CLI options, the AI reporter output format, and configuration file syntax. Install with:
```bash
npx skills add kucherenko/jscpd --skill jscpd
```

**dry-refactoring** — refactoring workflow skill. A guided process for reading clone output, choosing the right extraction strategy, applying the refactor, and verifying the clone is eliminated. Install with:
```bash
npx skills add kucherenko/jscpd --skill dry-refactoring
```

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results.

### MCP Server

[jscpd-server](apps/jscpd-server) implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), exposing jscpd's detection capabilities as tools that AI assistants can call directly from the editor. Start the server against your codebase once, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed.

```bash
npm install -g jscpd-server
jscpd-server /path/to/project
```

Add to your MCP client config (e.g. Claude Desktop):

```json
{
  "mcpServers": {
    "jscpd": {
      "type": "streamable-http",
      "url": "http://localhost:3000/mcp"
    }
  }
}
```

Available MCP tools: `check_duplication`, `get_statistics`, `check_current_directory`. Full API docs at [apps/jscpd-server](apps/jscpd-server).

## What's New

**v5.0.x**

- **Rust engine** — ground-up rewrite in Rust, published as the [`cpd`](https://www.npmjs.com/package/cpd) npm package and `jscpd` crate. 10-30x faster than the Node.js version on real projects.
- **225+ formats** — expanded format support (up from 223), including new tokenizer backends for Go (oxc-based), TypeScript/JSX, and Markdown embedded code blocks.
- **Git blame with side-by-side comparison** — `--blame --reporters console-full` shows per-line author attribution with `==` (same author) and `<=` (different author) markers, matching jscpd v4's `consoleFull` format.
- **`--skip-local`** — skip clones where both fragments are in the same directory.
- **Statistics table in all console reporters** — both `console` and `console-full` now show the per-format statistics table.
- **Self-contained binary** — the `cpd` npm package ships prebuilt binaries for 6 platforms (no Node.js runtime required).

**v4.2.x**

- **Custom tokenizer backend** — replaced the `prismjs` npm package with an own backend built on the [reprism](https://github.com/tannerlinsley/reprism) grammar engine. ~11.5% faster tokenization on real projects (avg 1126ms → 997ms on a 548-file, 223-format scan).
- **Cross-format detection** — Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are now tokenized per-block/per-section, enabling duplicate detection across file types (e.g. a `<script>` block in a `.vue` file vs a `.ts` file).
- **New formats**: Apex, CFML/ColdFusion, GDScript, and 70+ additional formats (223 total, up from 152)
- **Shebang detection**: auto-detect language for extensionless executable scripts
- **`--store-path`**: configure LevelDB cache directory for parallel runs
- **`--skipComments`**: shorthand flag for `--mode weak`
- **`--formats-names`**: map specific filenames (e.g. `Makefile`, `Dockerfile`) to a format
- **`--noTips`**: suppress tip output in CI environments

### Bug Fixes

- **Entire-file duplicates silently dropped** — RabinKarp flushed the pending clone on a store *hit* at end-of-file instead of on a *miss*, causing files that are complete copies of each other to go undetected. Fixed in `@jscpd/core` (#728).
- **ReDoS hang on Lisp/Elisp files** — the Lisp string regex `/"(?:[^"\\]*|\\.)*"/` could catastrophically backtrack (O(2ⁿ)) on unterminated strings. Replaced with a linear alternative. Fixed in `@jscpd/tokenizer` (#737).
- **Process crash on malformed `package.json`** — when jscpd was run in a directory containing invalid JSON in `package.json`, `readJSONSync` threw an unhandled `SyntaxError` that killed the process. Now emits a warning and continues with an empty config (#739).
- **Vue SFC cross-file detection broken** — the detector used the file-level format (`vue`) as the store namespace for all SFC blocks, so a `<script>` block in one `.vue` file could never match a `<script>` block in another. The namespace now reflects each block's resolved sub-format (`javascript`, `typescript`, `scss`, etc.).
- **Vue SFC incorrect column numbers** — tokens on the first line of a block carried block-relative column 1 instead of the file-absolute column. Fixed in `@jscpd/tokenizer`.
- **50 dependency security vulnerabilities** remediated across the monorepo (Dependabot batches #DR-43 and #DR-7).

## Rust Engine (v5.x)

jscpd v5 introduces a ground-up Rust rewrite of the detection engine, available as the [`cpd`](https://www.npmjs.com/package/cpd) npm package. It is a drop-in replacement for the Node.js CLI — same algorithm, same reporters, same `.jscpd.json` config — but 10-30x faster.

### Performance

Benchmarks on the jscpd repository (release build, Apple M-series):

| Codebase | Files | `jscpd` (Node.js) | `cpd` (Rust) | Speedup |
|----------|-------|--------------------|--------------|---------|
| `fixtures/` (130 formats) | 353 | 1.59s | 0.45s | **3.5x** |
| `rust/crates/` (Rust sources) | 46 | 0.87s | 0.03s | **29x** |

Larger and more homogeneous codebases (fewer format switches) see the biggest gains.

### Install

```bash
npm install -g cpd
```

The correct platform-specific binary is selected automatically (macOS arm64/x64, Linux arm64/x64 glibc/musl, Windows x64). No Node.js runtime is needed — the binary is self-contained.

Or install from crates.io:

```bash
cargo install jscpd
```

### Usage

The `cpd` binary accepts the same options as `jscpd`:

```bash
# Drop-in replacement
cpd /path/to/source

# Same flags
cpd /path/to/source --min-tokens 30 --min-lines 3 --reporters console,json,html

# Git blame with side-by-side author comparison
cpd /path/to/source --blame --reporters console-full

# List supported formats
cpd --list
```

### Reporters

13 built-in reporters (same names as jscpd v4):

| Reporter | Output |
|----------|--------|
| `console` | Clone list + statistics table (default) |
| `console-full` | Clone list with source snippets; with `--blame` shows side-by-side author comparison |
| `json` | `report/cpd.json` |
| `xml` | `report/cpd.xml` |
| `csv` | `report/cpd.csv` |
| `html` | `report/cpd.html` |
| `markdown` | `report/cpd.md` |
| `badge` | `report/cpd-badge.svg` |
| `sarif` | `report/cpd.sarif.json` (GitHub Code Scanning) |
| `ai` | Token-efficient output for LLM pipelines |
| `xcode` | Xcode-compatible warnings |
| `threshold` | Exit 1 if duplication % exceeds `--threshold` |
| `silent` | No console output |

### Blame Output

With `--blame --reporters console-full`, clones are displayed with a side-by-side author comparison:

```
176 │ Andrii Kucherenko │ <= │ 196 │ Josh Soref │ ## TODO
177 │ Andrii Kucherenko │ <= │ 197 │ Josh Soref │
180 │ Andrii Kucherenko │ == │ 200 │ Andrii Kucherenko │ ## License
```

`==` means both lines were written by the same author; `<=` means different authors (potential copy).

### Known Differences from jscpd v4

| Feature | jscpd v4 (Node.js) | cpd v5 (Rust) |
|---------|--------------------|-----------------|
| `--blame` in `console-full` | Shows per-line side-by-side author comparison | Same — `==` / `<=` markers |
| `--store` (LevelDB) | Persistent store for large repos | Not supported. Use jscpd v4.x for external stores. |
| `--formatsExts` | Custom format-to-extension mapping | Not supported. Use `--format` to specify formats. |
| Programming API | `jscpd()` Promise API, `detectClones()` | Rust API via `cpd-finder` crate; no Node.js API |
| Config file | `.jscpd.json` with camelCase keys | Same — `.jscpd.json` with camelCase keys |
| Cross-format detection | Vue SFC, Svelte, Astro, Markdown | Same — per-block tokenization for embedded formats |
| Token counts | Varies slightly due to tokenizer differences | May differ by 1-2% due to Rust tokenizer; clone detection matches |
| `--reporters` | All v4 reporters | All v4 reporters except `full` (use `console-full`) |
| `--noGitignore` | Default respects `.gitignore` | Same |

### Architecture

```
cpd (CLI binary)
 ├── cpd-core      — Detection algorithm (Rabin-Karp rolling hash)
 ├── cpd-tokenizer — Language tokenization (225+ formats)
 ├── cpd-finder    — File walking, orchestration, git blame
 └── cpd-reporter  — Output formatting (13 reporters)
```

## Installation
```bash
$ npm install -g jscpd
```
## Usage
```bash
$ npx jscpd /path/to/source
```
or

```bash
$ jscpd /path/to/code
```
or

```bash
$ jscpd --pattern "src/**/*.js"
```

### CLI Aliases

jscpd supports short-form aliases for common options (matching TypeScript compiler conventions):

| Long Form | Short Form | Description |
|-----------|------------|-------------|
| `--min-lines` | `-l` | Minimum number of lines to detect |
| `--min-tokens` | `-k` | Minimum number of tokens to detect |
| `--max-lines` | `-x` | Maximum number of lines to detect |
| `--max-size` | `-z` | Maximum file size to check |
| `--threshold` | `-t` | Duplication threshold percentage |
| `--formatsExts` | `-e` | Custom format extensions |
| `--config` | `-c` | Path to config file |
| `--ignore` | `-i` | Ignore patterns |
| `--reporters` | `-r` | Output reporters |

Example with short forms:
```bash
$ jscpd -l 5 -k 50 -t 0.1 -r console,html /path/to/code
```

More information about cli [here](apps/jscpd).

## JSCPD Server

JSCPD Server is a standalone application that provides an API for detecting code duplication. It can be used to integrate duplication detection into your services or tools.

### Installation

```bash
$ npm install -g jscpd-server
```

### Usage

Start the server:

```bash
$ jscpd-server
```

Check code for duplication:

```bash
$ curl -X POST http://localhost:3000/api/check \
  -H "Content-Type: application/json" \
  -d '{
    "code": "console.log(\"hello\");\nconsole.log(\"world\");",
    "format": "javascript"
  }'
```

More information about server [here](apps/jscpd-server).

## Programming API

For integration copy/paste detection to your application you can use programming API:

`jscpd` Promise API
```typescript
import {IClone} from '@jscpd/core';
import {jscpd} from 'jscpd';

const clones: Promise<IClone[]> = jscpd(process.argv);
```

`jscpd` async/await API
```typescript
import {IClone} from '@jscpd/core';
import {jscpd} from 'jscpd';
(async () => {
  const clones: IClone[] = await jscpd(['', '', __dirname + '/../fixtures', '-m', 'weak', '--silent']);
  console.log(clones);
})();

```

`detectClones` API
```typescript
import {detectClones} from "jscpd";

(async () => {
  const clones = await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
    silent: true
  });
  console.log(clones);
})()
```

`detectClones` with persist store
```typescript
import {detectClones} from "jscpd";
import {IMapFrame, MemoryStore} from "@jscpd/core";

(async () => {
  const store = new MemoryStore<IMapFrame>();

  await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
  }, store);

  await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
    silent: true
  }, store);
})()
```

In case of deep customisation of detection process you can build your own tool with `@jscpd/core`, `@jscpd/finder` and `@jscpd/tokenizer` (Node.js) or `cpd-core`, `cpd-tokenizer`, `cpd-finder` and `cpd-reporter` (Rust).

**Rust API:**
```rust
use cpd_finder::orchestrate::{RunConfig, run};

let config = RunConfig {
    paths: vec!["./src".into()],
    min_tokens: 50,
    ..Default::default()
};

let result = run(&config).unwrap();
println!("Found {} clones", result.clones.len());
println!("Analyzed {} files", result.statistics.total.sources);
```

## Start contribution

 - Fork the repo [kucherenko/jscpd](https://github.com/kucherenko/jscpd/)
 - Clone forked version (`git clone https://github.com/{your-id}/jscpd`)
 - Install dependencies (`pnpm install`)
 - Run the project in dev mode: `pnpm dev` (watch changes and rebuild the packages)
 - Add your changes
 - Add tests and check it with `pnpm test`
 - Build your project `pnpm build`
 - Create PR

## Who uses jscpd
 - [GitHub Super Linter](https://github.com/github/super-linter) is combination of multiple linters to install as a GitHub Action
 - [Code-Inspector](https://www.code-inspector.com/) is a code analysis and technical debt management service.
 - [Mega-Linter](https://nvuillam.github.io/mega-linter/) is a 100% open-source linters aggregator for CI (GitHub Action & other CI tools) or to run locally
 - [Codacy](http://docs.codacy.com/getting-started/supported-languages-and-tools/) automatically analyzes your source code and identifies issues as you go, helping you develop software more efficiently with fewer issues down the line.
 - [Natural](https://github.com/NaturalNode/natural) is a general natural language facility for nodejs. It offers a broad range of functionalities for natural language processing.
 - [OpenClaw](https://github.com/openclaw/openclaw) is a personal AI assistant that runs on your own devices, supporting 20+ messaging channels and multi-platform companion apps.


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

## Star History

[![Star History Chart](https://api.star-history.com/chart?repos=kucherenko/jscpd&type=date&legend=top-left)](https://www.star-history.com/?repos=kucherenko%2Fjscpd&type=date&legend=top-left)

## License

[MIT](LICENSE) © Andrey Kucherenko
