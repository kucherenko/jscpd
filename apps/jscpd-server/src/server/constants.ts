import { readPackageJson } from "../setup";

const packageJson = readPackageJson();

export const SERVER_DEFAULTS = {
  PORT: 3000,
  HOST: "127.0.0.1",
  BODY_SIZE_LIMIT: "10mb",
} as const;

export const ERROR_MESSAGES = {
  SCAN_IN_PROGRESS: "Please wait for initial scan to complete",
  NOT_INITIALIZED:
    "Server not initialized. Please wait for initial scan to complete.",
  SOURCE_STORE_NOT_INITIALIZED: "Source store not initialized",
  EMPTY_CODE: "Code snippet cannot be empty",
  MCP_NOT_STARTED:
    "Service Unavailable: the MCP endpoint is closed. Start the server first.",
  MISSING_REQUIRED_FIELD: (field: string) => `Missing required field: ${field}`,
  INVALID_FIELD_TYPE: (field: string, expectedType: string) =>
    `Field "${field}" must be a ${expectedType}`,
  FIELD_CANNOT_BE_EMPTY: (field: string) => `Field "${field}" cannot be empty`,
} as const;

export const API_INFO = {
  NAME: "jscpd-server",
  VERSION: packageJson.version,
  DOCUMENTATION_URL: "https://github.com/kucherenko/jscpd",
} as const;

export const HTTP_STATUS = {
  OK: 200,
  BAD_REQUEST: 400,
  NOT_FOUND: 404,
  INTERNAL_SERVER_ERROR: 500,
  SERVICE_UNAVAILABLE: 503,
} as const;

export const MCP_ENDPOINT = "/mcp";

/** JSON-RPC code the SDK uses for transport-level server errors. */
export const MCP_SERVER_ERROR_CODE = -32000;

/**
 * The modern MCP revision served by this endpoint. Modern requests carry it in
 * the per-request `_meta` envelope and in the `MCP-Protocol-Version` header;
 * there is no `initialize` handshake and no `Mcp-Session-Id` for this era.
 */
export const MCP_MODERN_PROTOCOL_VERSION = "2026-07-28";

const STATIC_LISTING_TTL_MS = 300_000;
const STATISTICS_TTL_MS = 5_000;

/**
 * Cache fields (`ttlMs`/`cacheScope`) the 2026-07-28 revision requires on
 * cacheable results. Registrations and listings are static for the lifetime of
 * a process, while duplication statistics change on every rescan.
 */
export const MCP_CACHE_HINTS = {
  "server/discover": { ttlMs: STATIC_LISTING_TTL_MS, cacheScope: "public" },
  "tools/list": { ttlMs: STATIC_LISTING_TTL_MS, cacheScope: "public" },
  "resources/list": { ttlMs: STATIC_LISTING_TTL_MS, cacheScope: "public" },
  "resources/templates/list": {
    ttlMs: STATIC_LISTING_TTL_MS,
    cacheScope: "public",
  },
  "resources/read": { ttlMs: STATISTICS_TTL_MS, cacheScope: "private" },
} as const;
