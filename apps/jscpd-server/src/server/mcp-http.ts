import { createMcpHandler } from "@modelcontextprotocol/server";
import {
  hostHeaderValidation,
  originValidation,
  toNodeHandler,
} from "@modelcontextprotocol/node";
import type { NextFunction, Request, RequestHandler, Response } from "express";
import { createMcpServer } from "./mcp-server";
import { JscpdServerService } from "./service";

/**
 * DNS rebinding protection for the MCP endpoint, as required by the Streamable
 * HTTP transport: "Servers MUST validate the `Origin` header on all incoming
 * connections to prevent DNS rebinding attacks."
 */
export interface McpEndpointOptions {
  /**
   * Origin header hostnames a browser may use. A request without an `Origin`
   * header still passes — non-browser MCP clients do not send one — while a
   * present, unlisted origin is rejected with `403`.
   */
  allowedOrigins?: string[];
  /**
   * Host header hostnames the endpoint answers on. Omit it to apply no Host
   * restriction, which is what a deliberate external bind needs.
   */
  allowedHosts?: string[];
}

/**
 * The `/mcp` endpoint of the server, mounted on every HTTP method so that the
 * SDK owns the whole Streamable HTTP surface (modern direct requests, the
 * `405` answers for the 2025-era session operations, and the protocol error
 * responses).
 */
export interface McpEndpoint {
  /** Serves one HTTP exchange. Express has already parsed the JSON body. */
  handle(req: Request, res: Response): Promise<void>;
  /** Aborts in-flight modern exchanges and closes their per-request instances. */
  close(): Promise<void>;
}

function createHeaderValidators(options: McpEndpointOptions = {}) {
  const validateOrigin = originValidation(options.allowedOrigins ?? []);
  const validateHost = options.allowedHosts
    ? hostHeaderValidation(options.allowedHosts)
    : undefined;

  return (req: Request, res: Response): boolean => {
    if (!validateOrigin(req, res)) {
      return false;
    }
    if (validateHost && !validateHost(req, res)) {
      return false;
    }
    return true;
  };
}

/**
 * Origin and Host allowlist used by `/mcp` and the mutating REST routes.
 * A missing `Origin` still passes (non-browser clients omit it); a present,
 * unlisted Origin or Host is rejected with `403`.
 */
export function createNetworkGuard(
  options: McpEndpointOptions = {},
): RequestHandler {
  const allow = createHeaderValidators(options);
  return (req: Request, res: Response, next: NextFunction): void => {
    if (allow(req, res)) {
      next();
    }
  };
}

/**
 * Creates the MCP endpoint for a service.
 *
 * The handler serves protocol revision 2026-07-28: every request is a direct,
 * stateless exchange carrying its protocol version and client capabilities in
 * the per-request `_meta` envelope, `server/discover` replaces the `initialize`
 * handshake, and no `Mcp-Session-Id` is issued or required. 2025-era clients
 * keep working through the SDK's stateless legacy fallback, which is served by
 * the very same factory so the two eras can never drift apart.
 */
export function createMcpEndpoint(
  service: JscpdServerService,
  options: McpEndpointOptions = {},
): McpEndpoint {
  const onerror = (error: Error): void => {
    console.error("MCP request error:", error.message);
  };

  const handler = createMcpHandler(() => createMcpServer(service), {
    legacy: "stateless",
    onerror,
  });

  const nodeHandler = toNodeHandler(handler, { onerror });
  const allow = createHeaderValidators(options);

  return {
    handle: async (req, res) => {
      if (!allow(req, res)) {
        return;
      }
      await nodeHandler(req, res, req.body);
    },
    close: () => handler.close(),
  };
}

