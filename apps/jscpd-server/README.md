# JSCPD Server API Documentation

## Overview

The JSCPD Server provides a RESTful API for detecting code duplications. It scans a codebase on startup and allows clients to check code snippets for duplications against the scanned codebase.

## Starting the Server

### Command Line

```bash
# Start server in current directory
jscpd-server

# Start server in specific directory
jscpd-server /path/to/project

# Start server on specific port
jscpd-server . --port 8080

# Start server with custom host
jscpd-server . --host localhost --port 3000

# Start server with persistent storage (recommended for production)
jscpd-server . --store leveldb
```

### Server-Specific Options

- `-p, --port [number]` - Port to run the server on (Default: 3000)
- `-H, --host [string]` - Host to bind the server to (Default: 0.0.0.0)

### Common Options (Available for Server)

- `--store [string]` - Persistent store for codebase data (e.g., `leveldb` for disk persistence). Without this option, the server uses in-memory storage which is lost on restart
- `-c, --config [string]` - Path to config file (Default is .jscpd.json in <path>)
- `-f, --format [string]` - Format or formats separated by comma
- `-i, --ignore [string]` - Glob pattern for files to exclude
- `--ignore-pattern [string]` - Ignore code blocks matching regexp patterns
- `-l, --min-lines [number]` - Min size of duplication in code lines
- `-k, --min-tokens [number]` - Min size of duplication in code tokens
- `-x, --max-lines [number]` - Max size of source in lines
- `-z, --max-size [string]` - Max size of source in bytes
- `-m, --mode [string]` - Mode of quality of search (strict, mild, weak)
- `-a, --absolute` - Use absolute path in reports
- `-n, --noSymlinks` - Don't use symlinks for detection
- `--ignoreCase` - Ignore case of symbols in code
- `-g, --gitignore` - Ignore all files from .gitignore file
- `--skipLocal` - Skip duplicates in local folders

## Base URL

When running locally: `http://localhost:3000/api`

## Content Type

All API responses return `Content-Type: application/json`. The server accepts both `application/json` and `application/x-www-form-urlencoded` request bodies.

## Request Body Size Limit

The server accepts request bodies up to 10MB in size.

## Authentication

Currently, no authentication is required. Future versions may add authentication support.

## Endpoints

### 1. Check Code Snippet

Check a code snippet for duplications against the scanned codebase.

**Endpoint:** `POST /api/check`

**Request Headers:**
```http
Content-Type: application/json
```

**Request Body:**

```json
{
  "code": "string (required)",
  "format": "string (required)"
}
```

**Parameters:**

- `code` (required, string): The code snippet to check for duplications. This field cannot be empty or whitespace-only.
- `format` (required, string): Programming language/format of the snippet (e.g., "javascript", "python", "java")

**Example Request:**

```bash
curl -X POST http://localhost:3000/api/check \
  -H "Content-Type: application/json" \
  -d '{
    "code": "function hello() {\n  console.log(\"Hello, World!\");\n}",
    "format": "javascript"
  }'
```

**Success Response (200 OK):**

```json
{
  "duplications": [
    {
      "snippetLocation": {
        "startLine": 1,
        "endLine": 5,
        "startColumn": 0,
        "endColumn": 20
      },
      "codebaseLocation": {
        "file": "src/utils/helper.js",
        "startLine": 10,
        "endLine": 14,
        "startColumn": 0,
        "endColumn": 20,
        "fragment": "function hello() {\n  console.log(\"Hello, World!\");\n}"
      },
      "linesCount": 4
    }
  ],
  "statistics": {
    "totalDuplications": 1,
    "duplicatedLines": 4,
    "totalLines": 5,
    "percentageDuplicated": 80.0
  }
}
```

  "message": "Missing required field: code",
  "statusCode": 400
}
```

**400 Bad Request** - Validation error (wrong type):
```json
{
  "error": "ValidationError",
  "message": "Field \"code\" must be a string",
  "statusCode": 400
}
```

**400 Bad Request** - Validation error (empty code):
```json
{
  "error": "ValidationError",
  "message": "Field \"code\" cannot be empty",
  "statusCode": 400
}
```

**400 Bad Request** - Server not initialized:
```json
{
  "error": "Error",
  "message": "Server not initialized. Please wait for initial scan to complete.",
  "statusCode": 400
}
```

**400 Bad Request** - Processing error:
```json
{
  "error": "CheckError",
  "message": "Error details...",
  "statusCode": 400
}
```

#### Snippet Isolation and Memory Management

The `/api/check` endpoint implements **request isolation** to ensure:

1. **No Cross-Request Contamination**: Snippet tokens are isolated per request and never stored in the shared project store. Each request compares only against the scanned codebase, not against tokens from previous snippet checks.

2. **Automatic Cleanup**: Snippet token data is automatically discarded when the request completes (via finally block), preventing unbounded memory growth.

3. **Concurrent Request Safety**: Multiple concurrent snippet checks are isolated from each other. The same snippet checked simultaneously will produce identical results without interference.

4. **Consistent Results**: Checking the same snippet multiple times will always produce identical duplication reports, as snippet tokens don't persist between requests.

**Implementation Details**:

The server uses an ephemeral hybrid store for each snippet check:
- **Reads** are delegated to the shared project store (to detect duplications against the codebase)
- **Writes** (snippet tokens) go to a temporary in-memory store
- The temporary store is discarded after the request completes

This architecture ensures snippet detection remains stateless and memory-safe while still detecting duplications against the full project.

### 2. Get Project Statistics

Get overall duplication statistics for the scanned codebase.

**Endpoint:** `GET /api/stats`

**Request Headers:** None required

**Example Request:**

```bash
curl http://localhost:3000/api/stats
```

**Success Response (200 OK):**

The response includes the full statistics object from the initial scan. The structure follows the standard jscpd statistics format.

```json
{
  "statistics": {
    "detectionDate": "2025-11-17T10:30:00.000Z",
    "total": {
      "lines": 10000,
      "tokens": 50000,
      "sources": 50,
      "duplicatedLines": 500,
      "duplicatedTokens": 2500,
      "clones": 10,
      "percentage": 5.0,
      "percentageTokens": 5.0,
      "newDuplicatedLines": 0,
      "newClones": 0
    },
    "formats": {
      "javascript": {
        "total": {
          "lines": 5000,
          "tokens": 25000,
          "sources": 30,
          "duplicatedLines": 300,
          "duplicatedTokens": 1500,
          "clones": 6,
          "percentage": 6.0,
          "percentageTokens": 6.0,
          "newDuplicatedLines": 0,
          "newClones": 0
        },
        "sources": {
          "src/file1.js": {
            "lines": 100,
            "tokens": 500,
            "sources": 1,
            "duplicatedLines": 10,
            "duplicatedTokens": 50,
            "clones": 1,
            "percentage": 10.0,
            "percentageTokens": 10.0,
            "newDuplicatedLines": 0,
            "newClones": 0
          }
        }
      }
    }
  },
  "timestamp": "2025-11-17T10:30:00.000Z"
}
```

**Error Responses:**

**503 Service Unavailable** - Statistics not ready:
```json
{
  "error": "NotReady",
  "message": "Statistics not available yet. Server is still initializing.",
  "statusCode": 503
}
```

### 4. Health Check

Check server health and initialization status.

**Endpoint:** `GET /api/health`

**Request Headers:** None required

**Example Request:**

```bash
curl http://localhost:3000/api/health
```

**Success Response (200 OK):**

```json
{
  "status": "ready",
  "workingDirectory": "/path/to/project",
  "lastScanTime": "2025-11-17T10:30:00.000Z"
}
```

**Status Values:**
- `initializing` - Server is scanning the codebase
- `ready` - Server is ready to accept requests

### 5. API Information

Get information about the API and available endpoints.

**Endpoint:** `GET /`

**Request Headers:** None required

**Example Request:**

```bash
curl http://localhost:3000/
```

**Success Response (200 OK):**

```json
{
  "name": "jscpd-server",
  "version": "1.0.0",
  "endpoints": {
    "POST /api/check": "Check code snippet for duplications",
    "GET /api/stats": "Get overall project statistics",
    "GET /api/health": "Server health check"
  },
  "documentation": "https://github.com/kucherenko/jscpd"
}
```

## MCP Server

The server also supports the [Model Context Protocol (MCP)](https://github.com/modelcontextprotocol), allowing it to fit into LLM-based workflows.

### MCP Endpoint

**Endpoint:** `POST /mcp`

The server handles MCP requests via the `/mcp` endpoint using the `StreamableHTTPServerTransport`.

### MCP Tools

#### `check_duplication`
Check code snippet for duplications against the codebase.
- **Input**:
  - `code` (string): Source code snippet.
  - `format` (string): Format/language of the code.

#### `get_statistics`
Get overall project duplication statistics.
- **Input**: None

#### `check_current_directory`
Trigger a re-scan of the current working directory for duplications.
- **Input**: None

## Response Schemas

### CheckSnippetResponse

```typescript
{
  duplications: Array<{
    snippetLocation: {
      startLine: number;
      endLine: number;
      startColumn: number;
      endColumn: number;
    };
    codebaseLocation: {
      file: string;
      startLine: number;
      endLine: number;
      startColumn: number;
      endColumn: number;
      fragment?: string;
    };
    linesCount: number;
  }>;
  statistics: {
    totalDuplications: number;
    duplicatedLines: number;
    totalLines: number;
    percentageDuplicated: number;
  };
}
```

### ErrorResponse

```typescript
{
  error: string;
  message: string;
  statusCode: number;
}
```

## Supported Languages

The server supports all languages that jscpd supports. When checking a snippet, specify the language using the `format` parameter with the language name or file extension.

Common format identifiers:
- JavaScript: `javascript`, `js`
- TypeScript: `typescript`, `ts`
- Python: `python`, `py`
- Java: `java`
- C/C++: `c`, `cpp`
- C#: `csharp`, `cs`
- PHP: `php`
- Ruby: `ruby`, `rb`
- Go: `go`
- Rust: `rust`, `rs`

For a complete list, see the [supported formats documentation](../../supported_formats.md).

## Error Handling

All errors follow a consistent format with an HTTP status code and JSON body:

```json
{
  "error": "ErrorType",
  "message": "Human-readable error message",
  "statusCode": 400
}
```

### Common Error Types

- `ValidationError` (400) - Invalid request parameters (missing fields, wrong types, empty values)
- `CheckError` (400) - Error processing the check request
- `Error` (400) - Server not initialized or general errors
- `NotReady` (503) - Statistics not available yet (server still initializing)
- `NotFound` (404) - Endpoint not found
- `StatsError` (500) - Error retrieving statistics
- `InternalServerError` (500) - Unexpected server error

## Rate Limiting

Currently, no rate limiting is implemented. Consider implementing rate limiting in production environments.

## Persistent Storage with LevelDB

### Why Use Persistent Storage?

By default, the server uses in-memory storage (MemoryStore) for the codebase scan results. This means:
- ✅ **Fast** - All data is in memory
- ❌ **Volatile** - Data is lost when the server restarts
- ❌ **Memory-intensive** - Large codebases consume significant RAM

With `--store leveldb`, the server uses disk-based storage (LevelDB):
- ✅ **Persistent** - Data survives server restarts
- ✅ **Memory-efficient** - Data is stored on disk
- ✅ **No re-scanning** - Server starts immediately with cached data
- ⚠️ **Slightly slower** - Disk I/O overhead (minimal impact)

### Installation

```bash
# Install the LevelDB store package
npm install @jscpd/leveldb-store
```

### Usage

```bash
# Start server with LevelDB persistence
jscpd server /path/to/project --store leveldb --port 3000

# On first start, the server will:
# 1. Scan the codebase
# 2. Store results in .jscpd/ directory
# 3. Accept requests

# On subsequent restarts, the server will:
# 1. Load cached data from .jscpd/ directory
# 2. Accept requests immediately (no re-scan needed)
```

### Storage Location

LevelDB stores data in the `.jscpd/` directory relative to where the server is started. This directory contains:
- Token databases for each file format
- Duplication detection data

**Important**: Add `.jscpd/` to your `.gitignore` file:

```bash
echo ".jscpd/" >> .gitignore
```

### Cleanup

To clear cached data and force a fresh scan:

```bash
# Stop the server, then:
rm -rf .jscpd/
```

## Examples

### Example 1: Check JavaScript Code

```javascript
const axios = require('axios');

const checkCode = async () => {
  try {
    const response = await axios.post('http://localhost:3000/api/check', {
      code: `
function calculateSum(a, b) {
  return a + b;
}
      `,
      format: 'javascript'
    });

    console.log('Duplications found:', response.data.duplications.length);
    console.log('Percentage duplicated:', response.data.statistics.percentageDuplicated + '%');
  } catch (error) {
    console.error('Error:', error.response.data);
  }
};

checkCode();
```

### Example 2: Check Python Code

```python
import requests

def check_code():
    url = 'http://localhost:3000/api/check'
    payload = {
        'code': '''
def hello_world():
    print("Hello, World!")
        ''',
        'format': 'python'
    }

    response = requests.post(url, json=payload)

    if response.status_code == 200:
        data = response.json()
        print(f"Duplications found: {len(data['duplications'])}")
        print(f"Percentage duplicated: {data['statistics']['percentageDuplicated']}%")
    else:
        print(f"Error: {response.json()}")

check_code()
```

### Example 3: Get Project Statistics

```bash
#!/bin/bash

# Get statistics
curl -s http://localhost:3000/api/stats | jq '.statistics.total'

# Output:
# {
#   "lines": 10000,
#   "tokens": 50000,
#   "sources": 50,
#   "duplicatedLines": 500,
#   "duplicatedTokens": 2500,
#   "clones": 10,
#   "percentage": 5.0,
#   ...
# }
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Check Code Duplication

on: [push, pull_request]

jobs:
  check-duplication:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Setup Node.js
        uses: actions/setup-node@v2
        with:
          node-version: '18'

      - name: Install jscpd
        run: npm install -g jscpd

      - name: Start jscpd server
        run: |
          jscpd server . --port 3000 &
          sleep 10  # Wait for server to initialize

      - name: Check new code
        run: |
          # Check files changed in this PR
          for file in $(git diff --name-only HEAD~1); do
            if [[ -f "$file" ]]; then
              # Extract file extension to determine format
              ext="${file##*.}"
              curl -X POST http://localhost:3000/api/check \
                -H "Content-Type: application/json" \
                -d "{\"code\": \"$(cat $file | jq -Rs .)\", \"format\": \"$ext\"}" \
                | jq .
            fi
          done
```

## Best Practices

1. **Initialize Once**: The server scans the codebase on startup. For large codebases, this may take time. Check the `/api/health` endpoint to verify the server is ready before sending requests.

2. **Format Parameter**: Always provide the `format` parameter with a valid language identifier or file extension.

3. **Snippet Size**: Large snippets take longer to process. Maximum body size is 10MB.

4. **Error Handling**: Always handle errors appropriately, especially 400 errors when the server is not initialized or validation fails.

5. **Production Use**: For production use, consider:
   - Using persistent storage with `--store leveldb` to avoid rescanning on restarts
   - Adding authentication
   - Implementing rate limiting
   - Using a reverse proxy (nginx, Apache)
   - Monitoring and logging
   - Running behind HTTPS

## Troubleshooting

### Server won't start

- **Port already in use**: Try a different port with `--port`
- **Permission denied**: Use a port above 1024 or run with appropriate permissions

### No duplications found

- Ensure `format` is correctly specified with a valid language identifier
- Check that the codebase was successfully scanned (check server logs)
- Verify minimum thresholds (`--min-lines`, `--min-tokens`)

### 400 Bad Request - Server not initialized

- Wait for the initial scan to complete
- Check `/api/health` endpoint - status should be `ready` not `initializing`

### Validation errors

- Ensure `code` field is a non-empty string
- Ensure `format` field is provided and is a string
- Check that the request body is valid JSON

## Performance Considerations

- **Initial Scan Time**: Depends on codebase size. Large codebases may take several minutes.
- **Memory Usage**: The server keeps the scanned codebase in memory by default. Monitor memory usage for large projects. Snippet checks use ephemeral stores that are automatically garbage-collected, preventing unbounded memory growth from snippet tokens.
- **Check Response Time**: Typically < 1 second for small snippets, longer for larger snippets.
- **Concurrent Requests**: Snippet checks are isolated and thread-safe. Multiple concurrent requests do not interfere with each other.
- **Persistent Storage**: Use `--store leveldb` for disk-based persistence. This allows the server to survive restarts without rescanning the codebase. LevelDB is recommended for large repositories. To use LevelDB, ensure `@jscpd/leveldb-store` is installed: `npm install @jscpd/leveldb-store`

## Support

For issues, questions, or feature requests, please visit:
- GitHub Issues: [https://github.com/kucherenko/jscpd/issues](https://github.com/kucherenko/jscpd/issues)
- Documentation: [https://github.com/kucherenko/jscpd](https://github.com/kucherenko/jscpd)
