# AI-Ready Integrations

jscpd v4 integrates into AI-powered development workflows through two mechanisms: the token-efficient `ai` reporter and the `jscpd-server` MCP server.

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

Each line is one clone pair:

- **Same file**: `path/file.ts 10-25 ~ 45-60` (shared path shown once)
- **Same directory**: `shared/prefix/ file-a.ts:10-25 ~ file-b.ts:42-57` (common prefix factored out)
- **Different paths**: `path/a.ts:10-25 ~ path/b.ts:42-57`

### Token Efficiency

Measured on the `fixtures/` directory in this repository:

| Reporter | Output size | Estimated tokens |
|----------|-------------|------------------|
| `console` (default) | ~21,800 chars | ~5,400 |
| `ai` | ~4,500 chars | ~1,100 |

Roughly 79% fewer tokens than the default console reporter.

### Agent skills

The installable agent skills (`npx skills add kucherenko/jscpd --skill jscpd` / `--skill dry-refactoring`) are maintained on [`master`](https://github.com/kucherenko/jscpd) and document the v5 CLI. Several options they reference (`--summary`, `--cross-formats`, `--min-similarity`, `--no-tips`) do not exist in v4 — with `jscpd@4`, point your agent at the [CLI reference](typescript.md) instead, and use `--reporters ai --noTips`.

## MCP Server

[jscpd-server](../apps/jscpd-server) exposes detection as [Model Context Protocol (MCP)](https://modelcontextprotocol.io) tools over Streamable HTTP, plus a REST API. Start a server once against your codebase, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed. Use it when several clients share one long-lived server or you need the LevelDB store.

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

Add to your MCP client config (e.g. Claude Desktop, Claude Code, Cursor):

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

### stdio transport

A built-in stdio MCP server (`jscpd --mcp`) is a v5 feature; it is not available in v4. See the [v5 documentation](https://jscpd.dev) if you need it.
