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

The `jscpd`/`cpd` binary serves MCP over stdio itself (`jscpd --mcp` or `cpd --mcp`). Most MCP clients start and manage such a server on their own, and stdio needs no port and no network policy. The server scans the project in the background as it starts, so the client connects at once. The first tool call waits for the scan, and a line on stderr says when it is done. Snippet checks compare against the tokens the server keeps in memory and need no new scan.

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
| `similar` | Type-3 | A copy with lines added, removed or changed | `gap`: clones of one file pair merged across up to `--max-gap-lines` unmatched lines (2 when the option is not set). `ast`: JavaScript, TypeScript and Python functions, classes, variables and type aliases with the same syntax-tree shape, at `--similarity` (0.85 when not set). With `--similarity-identifiers role-aware` on the server, the methods that calls invoke count too, so functions that call different methods score lower. The server's `--similarity-literals`, or `similarityLiterals` in its config, decides how literals count, its `--similarity-decorators` (`similarityDecorators`) how Python decorators do, and its `--similarity-candidates` and `--similarity-skip-tests` (`similarityCandidates`, `similaritySkipTests`) which units are compared |
| `semantic` | Type-4 | Functions that do the same job, written differently or in another language | The `--semantic` embedding model |

See [`fixtures/mcp-demo`](../fixtures/mcp-demo/README.md) for a runnable example of each kind and of `compare_folders`.

Every clone tool takes `kinds`, a list of kind names. `gap` and `ast` name one of the two ways to find similar clones, and `type1` to `type4` work as names too. Without `kinds`, a tool reports what `jscpd` reports with the server's options. By default that means exact clones, plus renamed ones when `--ignore-identifiers`, `--ignore-literals` or `--ignore-annotations` is on, similar ones with `--max-gap-lines` or `--similarity`, and semantic ones with `--semantic` or the config's `semantic.enabled`. `--kind` sets other defaults, so `jscpd --mcp --kind exact,renamed,similar` looks for the first three types in every call.

The server finds a kind the first time a request asks for it and keeps the result until `check_current_directory` scans again. A request without `renamed` reads the scan made with the configured options, so its exact clones match a `jscpd` run with the same options. Ast and semantic clones need the syntax trees and functions of the files, and a scan reads them from the same text as its tokens. The first request for one of these kinds may therefore scan again, and every result describes the same files.

Semantic clones need the embedding model on this machine (`jscpd --semantic-download`, 548 MB) or an embeddings API (`--semantic-url`). The server never downloads the model. When it cannot look for semantic clones, the result lists `semantic` under `unavailable` with the reason, and the assistant can ask you first. The first semantic request embeds every function of the project, which can take minutes. Later requests reuse the vectors cached for the project, and `jscpd --semantic` on the same folder uses that cache too.

### Tools

- `check_duplication(code, format, kinds?, limit?, similarity?)` compares a snippet with the scanned project, so you can reuse existing code instead of writing it again. A match has its `kind`, the file and lines of the project's copy and of the snippet's, and `tokens`. Similar and semantic matches add `similarity`, similar ones also `method` (`gap` or `ast`), ast ones `unit` (`function`, `class`, `variable` or `type`), and a match between two functions or two classes adds `name` (the project's) and `snippetName`. Exact matches come first, then renamed, similar and semantic ones. `similarity`, a ratio in `(0, 1]`, sets the threshold of ast matches for the call, and `1` turns them off. A request without `kinds` gets its ast matches under `similar` with `similarCount`, plus `similarNote` for a language without syntax trees, the way earlier versions answered. With `kinds`, every match is in `duplications`. `format` takes a format name (`javascript`) or a file extension (`js`).
- `get_file_clones(path, kinds?, limit?)` lists the clones of one file, for refactoring that file. `path` is relative to the scan root, as results show paths, or absolute.
- `get_statistics(kinds?)` returns the totals and per-format statistics of the last scan for the kinds asked, and the number of clones of each kind (`byKind`).
- `check_current_directory(kinds?, limit?)` scans the configured paths again and returns the new counts and clone list.
- `compare_folders(left, right, limit?)` compares two folders function by function, as [`--compare`](rust.md#comparing-two-codebases-with---compare-experimental) does, in one language or across two. Use it for a port and its original, or for the iOS and Android versions of an app. [Comparing two folders](#comparing-two-folders) says how it differs from `--compare`.

A tool answers with JSON, sent as `structuredContent` and again as text for clients of older revisions. Clone lists come biggest first, except the matches of `check_duplication`, which keep the order above. The optional `limit` argument caps every list, at 100 entries by default. The count next to a list (`clones`, `count`) always gives the full number, and a cut list carries a `note`. If a tool cannot use its arguments, such as a missing `code`, an unknown format or kind, or a `similarity` out of range, the server answers with a tool error (`isError: true`), a message that says what to fix, so the assistant can try again. An unknown tool gets a JSON-RPC error.

### Comparing two folders

`--mcp` and `--compare` do not go together on one command line, and `jscpd --mcp --compare A B` stops with `the argument '--mcp' cannot be used with '--compare'`. The server offers the comparison as the `compare_folders` tool instead, which runs the same code as `--compare`:

```json
{ "name": "compare_folders", "arguments": { "left": "python", "right": "typescript" } }
```

On [`fixtures/compare-demo`](../fixtures/compare-demo/README.md), the tool finds a counterpart for 5 of the 7 Python functions and 4 of the 5 TypeScript ones, as `jscpd --compare python typescript` does. It differs from the command line in a few ways:

- The tool returns the report of `--compare -r json`, with the `code` and `tests` sections, `pairs`, `unmatched` and `readyToPort`, plus the model's name. The console, Markdown and HTML reports, the migration map among them, stay on the command line.
- Both folders must lie inside the paths the server scans, and a relative path starts from one of them. To compare two separate projects, start the server with both: `jscpd --mcp ../python-lib ../rust-lib`.
- `limit` cuts each list of the report to 100 entries by default. The totals stay whole, and a `note` names the lists the tool cut.
- The tool needs the embedding model or an embeddings API (`--semantic-url`). The server never downloads the model, and without it the tool answers with an error that tells the assistant to ask the user first.
- The model and its thresholds come from the server's `--semantic-*` options and config file, as for `--compare`, and `--semantic-scope` has no effect. `--min-tokens` sets the size a function needs to count, 30 by default, as with `--compare`.
- The vectors are cached per pair of folders, and `jscpd --compare` on the same folders uses the same cache. A local model loads once per server process, so a second comparison does not read the weights again.

### Protocol revisions

The server speaks MCP 2026-07-28, the stateless revision, in which each request names its version in `_meta` and `server/discover` describes the server. It also speaks the revisions that open with an `initialize` handshake (2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05), and one process serves clients of both kinds.

### HTTP transport

There is no HTTP transport in v5. `jscpd-server` — MCP over Streamable HTTP plus a REST API, for several clients sharing one long-lived server — is part of jscpd v4 and is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4/apps/jscpd-server) branch (`npm install -g jscpd-server`).
