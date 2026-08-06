import type { Application, Request } from "express";
import type { McpEndpoint } from "./mcp-http";
import { JscpdServerService } from "./service";

/**
 * Everything the request handlers need, held explicitly on the express app
 * instead of in handler closures or an implicit per-session transport map.
 * Modern MCP requests are stateless, so this is the only server-side state.
 *
 * `mcp` is bound to a single server run: it is opened by `start()` and closed
 * and released by `stop()`, so a restarted server never serves from a handler
 * that was closed by the previous run.
 */
export interface JscpdAppState {
  service: JscpdServerService;
  mcp: McpEndpoint | null;
}

const APP_STATE_KEY = "jscpdState";

export function setAppState(app: Application, state: JscpdAppState): void {
  app.locals[APP_STATE_KEY] = state;
}

export function getAppState(app: Application): JscpdAppState {
  const state = app.locals[APP_STATE_KEY] as JscpdAppState | undefined;
  if (!state) {
    throw new Error("Application state is not initialized");
  }
  return state;
}

export function getRequestState(req: Request): JscpdAppState {
  return getAppState(req.app);
}
