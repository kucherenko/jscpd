# AI-Ready Integrations

jscpd integrates into AI-powered development workflows through three complementary mechanisms: the AI reporter, agent skills, and an MCP server.

## AI Reporter

The `ai` reporter produces compact, token-efficient output designed to be piped directly into an LLM prompt or agentic pipeline. It uses common-path-prefix compression and omits code fragments and colors — just the clone locations and a summary.

```bash
jscpd --reporters ai /path/to/source
```

### Example Output

```
src/utils/ auth.ts:10-25 ~ helpers.ts:40-55
src/utils/auth.ts 30-45 ~ 80-95
src/ utils/auth.ts:10-25 ~ api/routes.ts:5-20
---
23 clones · 4.2% duplication
```

### Token Efficiency

Benchmarked on the `fixtures/` directory (212 clones, 347 files):

| Reporter | Output size | Estimated tokens |
|----------|-------------|------------------|
| `console` (default) | ~21,800 chars | ~5,400 |
| `ai` | ~4,500 chars | ~1,100 |

~79% fewer tokens than the default console reporter.

### Codebase Summary

Add `--summary` for a compact refactoring-hotspot overview — top files and folders by tokens, lines, size, and a complexity estimate. In the `ai` reporter each entry is one line with all metrics inline, so an agent gets the full picture for a handful of tokens:

```
Summary by tokens (321 files, 129 folders):
files (tokens/lines/size/cx/dup%):
src/core/files.ts 2052/363/11.4K/80/0.0%
...
folders (files/tokens/lines/size):
src/core 8/5264/843/26.5K
...
```

```bash
jscpd --reporters ai --summary --no-tips /path/to/source
```

See [rust.md](rust.md#summary) for the metric definitions and `--summary-top` / `--summary-by` options.

When an agent only needs the complexity ranking, `--complexity` skips clone detection and prints the same compact rows without the duplication column:

```
Complexity by complexity (6 files, 3 folders):
files (tokens/lines/size/cx):
src/rates.ts 182/35/757/11
...
folders (files/tokens/lines/size/mean cx):
src 4/557/83/2.2K/5
...
```

```bash
jscpd --reporters ai --complexity --no-tips /path/to/source
```

To hand an agent one kind of clone, combine `--reporters ai` with `--kind`, e.g. `--ignore-identifiers --kind renamed` for copies that differ only in names.

## Agent Skills

jscpd ships AI agent skills that teach coding assistants how to use jscpd, refactor detected duplications, and clean up a codebase more broadly.

### jscpd — Tool Reference Skill

Covers all CLI options, the AI reporter output format, and configuration file syntax.

```bash
npx skills add kucherenko/jscpd --skill jscpd
```

### dry-refactoring — Refactoring Workflow Skill

A guided process for reading clone output, choosing the right extraction strategy, applying the refactor, and verifying the clone is eliminated.

```bash
npx skills add kucherenko/jscpd --skill dry-refactoring
```

### codebase-refactoring — Codebase Health Workflow Skill

A broader pass for "clean up this codebase" requests: fix duplication first (delegates to dry-refactoring), then find and remove or refactor dead code (`--dead-code`), then find and simplify the largest/most complex files (`--complexity`) — prioritized from `--health`/`--dashboard` and re-measured at the end.

```bash
npx skills add kucherenko/jscpd --skill codebase-refactoring
```

### compare-codebases — Folder Comparison Skill

Explains how `jscpd --compare` pairs the functions of two folders, in one language or across two, and what the similarity levels mean. It gives the agent a way to run a comparison: pick the folders, read the overview, check a sample of pairs and every `low` one, search for the counterparts of unmatched functions before calling them missing, and report the result.

```bash
npx skills add kucherenko/jscpd --skill compare-codebases
```

### code-migration — Migration and Parity Workflow Skill

For porting a codebase to another language or framework, and for checking two implementations of one app (Android and iOS) for parity. The agent binds each source function to the tests that exercise it with a per-test coverage report, ports the tests first and the code second, and measures both with `jscpd --compare SOURCE TARGET`. It ports the missing functions in dependency order, turns on each function's ported tests as it lands, checks with coverage that they reach the new code, and records the functions it decided not to port.

```bash
npx skills add kucherenko/jscpd --skill code-migration
```

After installation, ask your agent to "find and fix code duplication" for the focused pass, "clean up this codebase" for the broader one, or "port the iOS app to Android" for a migration, and it will invoke jscpd with the right options and act on the results.

## MCP Server

jscpd speaks the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), exposing detection capabilities as tools that AI assistants can call directly from the editor. Start the server once against your codebase, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed.

### stdio transport (Rust v5)

The `jscpd`/`cpd` binary serves MCP over stdio directly (`jscpd --mcp` or `cpd --mcp`), the transport most MCP clients start and manage themselves, with no port and no network policy. The server scans the project in the background as it starts, so the client connects at once and the first tool call waits for the scan (a log line on stderr says when it is done). Snippet checks run against the tokens kept in memory, so they answer without a rescan.

```bash
jscpd --mcp /path/to/project
# The detection options apply to the scan and to snippet checks:
jscpd --mcp --min-tokens 30 --format javascript,typescript /path/to/project
# Semantic clones too by default (the model first: jscpd --semantic-download)
jscpd --mcp --semantic /path/to/project
```

Client configuration (Claude Desktop, Claude Code, Cursor, APM, ...):

```json
{
  "mcpServers": {
    "jscpd": {
      "command": "cpd",
      "args": ["--mcp", "/path/to/project"]
    }
  }
}
```

### Clone types

The tools find the four types of clone:

| Kind | Type | What it is | How the server finds it |
|------|------|------------|-------------------------|
| `exact` | Type-1 | The same tokens; layout and comments may differ | The token passes of the scan |
| `renamed` | Type-2 | The same code with identifiers or literals changed | The normalization the options configure (`--ignore-identifiers`, `--ignore-literals`, `--ignore-annotations`); with none configured, a second scan that normalizes identifiers and literals |
| `similar` | Type-3 | A copy with lines added, removed or changed | `gap`: clones of one file pair merged across up to `--max-gap-lines` unmatched lines (2 when the option is not set). `ast`: JavaScript and TypeScript functions with the same syntax-tree shape, at `--similarity` (0.85 when not set) |
| `semantic` | Type-4 | Functions that do the same job, written differently or in another language | The `--semantic` embedding model |

See [`fixtures/mcp-demo`](../fixtures/mcp-demo/README.md) for a runnable example of each kind and of `compare_folders`.

Every clone tool takes `kinds`: a list of kind names, where `gap` and `ast` stand for one of the two ways to find similar clones and `type1` to `type4` name the types. Without it, the tools report what `jscpd` reports with the server's options: exact clones, plus the kinds the options switch on (renamed with `--ignore-identifiers`, `--ignore-literals` or `--ignore-annotations`, similar with `--max-gap-lines` or `--similarity`, semantic with `--semantic` or the config's `semantic.enabled`). `--kind` sets other defaults: `jscpd --mcp --kind exact,renamed,similar` looks for the first three types in every call. A request without `renamed` reads the scan made with the configured options, so its exact clones are the ones `jscpd` reports with the same options. The server finds each kind when a request first asks for it and keeps the result until `check_current_directory` scans again. A scan reads the syntax trees and functions that ast and semantic clones need from the same text as its tokens, so the first request for one of them may scan again; every result then describes the same files.

Semantic clones need the embedding model on this machine (`jscpd --semantic-download`, 548 MB) or an embeddings API (`--semantic-url`). The server never downloads the model itself: when it cannot look for semantic clones, the result lists `semantic` under `unavailable` with the reason, so the assistant can ask you first. The first semantic request embeds every function of the project and can take minutes; later ones reuse the vectors cached for the project, and a server started on the same folder as `jscpd --semantic` shares that cache.

### Tools

- `check_duplication(code, format, kinds?, limit?, similarity?)`: compare a snippet with the scanned project, to find code to reuse before writing it again. Each match has its `kind`, the file and lines of the project's copy and of the snippet's, and `tokens`; similar and semantic matches add `similarity` (and `method`, `gap` or `ast`), and function matches add `name` (the project's function) and `snippetName`. Exact matches come first, then renamed, similar and semantic ones. `similarity` (a ratio in `(0, 1]`) sets the threshold of ast matches for this call, and `1` turns them off. A request without `kinds` gets its ast matches under `similar`, with `similarCount` (and `similarNote` for a language without syntax trees), as earlier versions answered; with `kinds`, every match is in `duplications`. `format` takes a format name (`javascript`) or a file extension (`js`)
- `get_file_clones(path, kinds?, limit?)`: the clones of one file, for file-scoped refactoring; `path` is relative to the scan root, as results show paths, or absolute
- `get_statistics(kinds?)`: totals and per-format statistics of the last scan for the kinds asked, and the number of clones of each kind (`byKind`)
- `check_current_directory(kinds?, limit?)`: scan the configured paths again and return the fresh counts and clone list
- `compare_folders(left, right, limit?)`: compare two folders function by function, as [`--compare`](rust.md#comparing-two-codebases-with---compare-experimental) does, in one language or across languages: a port and its original, or the iOS and Android versions of an app. It returns the `--compare` JSON report with the model's name. Both folders must lie inside the paths the server scans, and a relative path is taken from one of them; to compare two projects, start the server with both. Like semantic clones, it needs the embedding model

Tool results are JSON, as `structuredContent` and as text for clients of older revisions. Clone lists come biggest first (the matches of `check_duplication` in the order above), and the optional `limit` argument caps every list (default 100). The count next to a list (`clones`, `count`) is always the full one, and a cut list gets a `note`. When a tool cannot use its arguments (a missing `code`, an unknown format or kind, a `similarity` out of range), the server answers with a tool error (`isError: true`) that says what to fix, so the assistant can retry. An unknown tool is a JSON-RPC error.

### Protocol revisions

The server speaks MCP 2026-07-28, the stateless revision: each request names its version in `_meta`, and `server/discover` describes the server. It also speaks the revisions with an `initialize` handshake, 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05, so clients of either kind can use one process.

### HTTP transport

There is no HTTP transport in v5. `jscpd-server` — MCP over Streamable HTTP plus a REST API, for several clients sharing one long-lived server — is part of jscpd v4 and is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4/apps/jscpd-server) branch (`npm install -g jscpd-server`).
