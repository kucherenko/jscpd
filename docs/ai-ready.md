# AI-Ready Integrations

jscpd integrates into AI-powered development workflows through three complementary mechanisms: the AI reporter, agent skills, and an MCP server.

## AI Reporter

The `ai` reporter produces compact, token-efficient output designed to be piped directly into an LLM prompt or agentic pipeline. It uses common-path-prefix compression and omits code fragments and colors — just the clone locations and a summary.

### TypeScript (v4)

```bash
jscpd --reporters ai /path/to/source
```

### Rust (v5)

```bash
cpd --reporters ai /path/to/source
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

### Codebase Summary (v5)

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

jscpd speaks the [Model Context Protocol (MCP)](https://modelcontextprotocol.io) over two transports, exposing detection capabilities as tools that AI assistants can call directly from the editor. Start a server once against your codebase, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed.

| Transport | Command | Engine |
|-----------|---------|--------|
| stdio | `cpd --mcp /path/to/project` (or `jscpd --mcp`) | Rust v5 |
| Streamable HTTP | `jscpd-server /path/to/project` | Node.js v4 |

### stdio transport (Rust v5)

The `cpd`/`jscpd` v5 binary serves MCP over stdio directly — the transport most MCP clients spawn-and-manage themselves, with no port, no network policy, and the Rust engine's scan speed. The project is scanned once at startup (log line on stderr); snippet checks run against in-memory token hashes, so they answer in milliseconds even on large codebases.

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

### Streamable HTTP transport (jscpd-server, Node.js v4)

[jscpd-server](../apps/jscpd-server) serves MCP over HTTP plus a REST API — use it when several clients share one long-lived server or you need the LevelDB store.

### Installation

```bash
npm install -g jscpd-server
```

### Usage

Start the server:

```bash
jscpd-server /path/to/project
```

Options:
- `--port` — Port number (default: 3000)
- `--host` — Host to bind (default: 127.0.0.1)
- `--allowed-origin` — Extra `Origin` hostname accepted by the MCP and REST endpoints (repeatable)
- `--allowed-host` — `Host` hostname the MCP and REST endpoints answer on (repeatable)
- `--store leveldb` — Use LevelDB persistent storage
- Plus all standard jscpd detection options

### MCP Configuration

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

The endpoint serves protocol revision `2026-07-28`: requests are direct and stateless, carrying their protocol version and client capabilities in the per-request `_meta` envelope, so there is no `initialize` handshake and no `Mcp-Session-Id`. Clients discover the server with `server/discover`. 2025-era clients keep working through the SDK's stateless legacy fallback.

`/mcp`, `POST /api/check`, `POST /api/recheck`, and `GET /api/stats` validate the `Origin` and `Host` headers, as the transport specification requires. Loopback origins and hosts are allowed by default; add `--allowed-origin` for a browser client served under another name, and `--allowed-host` to pin extra hostnames a reachable deployment answers on. A concrete `--host` is always included in the Host allowlist.

### REST API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/check` | Check a code snippet for duplications. Body: `{"code": "...", "format": "javascript"}` |
| `POST` | `/api/recheck` | Trigger a re-scan of the directory |
| `GET` | `/api/stats` | Get overall project duplication statistics |
| `GET` | `/api/health` | Health check — returns `{ status, workingDirectory, lastScanTime }` |
| `GET` | `/` | API info with endpoint listing |

### MCP Tools

Available MCP tools exposed via the `/mcp` endpoint:

- `check_duplication` — Check a code snippet for duplications (inputs: `code`, `format`)
- `get_statistics` — Get project stats (no inputs)
- `check_current_directory` — Re-scan the working directory (no inputs)

Snippet checking uses an ephemeral in-memory store per request for isolation — no cross-request contamination, automatic cleanup, concurrent-request safe.