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
cpd --reporters ai --summary --no-tips /path/to/source
```

See [rust.md](rust.md#summary) for the metric definitions and `--summary-top` / `--summary-by` options.

## Agent Skills

jscpd ships two AI agent skills that teach coding assistants how to use jscpd and refactor detected duplications.

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

After installation, ask your agent to "find and fix code duplication" and it will invoke jscpd with the right options and act on the results.

## MCP Server

jscpd speaks the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), exposing detection capabilities as tools that AI assistants can call directly from the editor. Start the server once against your codebase, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed.

### stdio transport (Rust v5)

The `jscpd`/`cpd` binary serves MCP over stdio directly (`jscpd --mcp` or `cpd --mcp`) — the transport most MCP clients spawn-and-manage themselves, with no port and no network policy. The project is scanned once at startup (log line on stderr); snippet checks run against in-memory token hashes, so they answer without a rescan.

```bash
cpd --mcp /path/to/project
# All detection options apply to the scan and to snippet checks:
cpd --mcp --min-tokens 30 --format javascript,typescript /path/to/project
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

The server implements MCP protocol revision `2025-06-18` (also accepting `2025-03-26` and `2024-11-05` clients) and exposes four tools:

- `check_duplication(code, format, limit?)` — check a snippet against the scanned project; `format` accepts format names (`javascript`) or file extensions (`js`)
- `get_file_clones(path, limit?)` — clones involving one file, for file-scoped refactoring; `path` is scan-root-relative (as shown in results) or absolute
- `get_statistics()` — totals and per-format statistics from the last scan
- `check_current_directory(limit?)` — re-scan the configured paths and return updated counts plus the clone list

Tool results are compact JSON in a text content block. Every clone/match list is sorted biggest-first (by tokens) and capped by the optional `limit` argument (default 100) — the accompanying `clones`/`count` field always reports the untruncated total, and truncation is flagged with a `note`.

### HTTP transport

There is no HTTP transport in v5. `jscpd-server` — MCP over Streamable HTTP plus a REST API, for several clients sharing one long-lived server — is part of jscpd v4 and is maintained on the [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4/apps/jscpd-server) branch (`npm install -g jscpd-server`).
