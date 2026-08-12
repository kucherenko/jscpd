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

[jscpd-server](../apps/jscpd-server) implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io), exposing jscpd's detection capabilities as tools that AI assistants can call directly from the editor. Start the server once against your codebase, then let your AI assistant check any snippet for duplication on demand — no CLI invocation needed.

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
- `--host` — Host to bind (default: 0.0.0.0)
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

With [Autohand Code](https://github.com/autohandai/code-cli/), register the running HTTP endpoint from the CLI:

```bash
autohand mcp add --transport http jscpd http://localhost:3000/mcp
```

Add `--scope project` to keep the registration in the current workspace.

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
