import express, { Express } from "express";
import morgan from "morgan";
import { createMcpEndpoint, createNetworkGuard, McpEndpoint } from "./mcp-http";
import { JscpdServerService } from "./service";
import { createRouter } from "./routes";
import { errorHandler, notFoundHandler } from "./middleware";
import { getRequestState, setAppState } from "./app-state";
import { resolveAllowedHosts, resolveAllowedOrigins } from "./network-policy";
import { IOptions } from "@jscpd/core";
import {
  SERVER_DEFAULTS,
  API_INFO,
  ERROR_MESSAGES,
  HTTP_STATUS,
  MCP_ENDPOINT,
  MCP_MODERN_PROTOCOL_VERSION,
  MCP_SERVER_ERROR_CODE,
} from "./constants";

export interface ServerOptions {
  port?: number;
  host?: string;
  /** Extra Origin header hostnames accepted by the MCP and REST endpoints. */
  allowedOrigins?: string[];
  /** Host header hostnames the MCP and REST endpoints answer on. */
  allowedHosts?: string[];
  jscpdOptions?: Partial<IOptions>;
}

/** Runs a teardown step to completion, returning its failure instead of throwing. */
function settle(step: Promise<unknown>): Promise<Error | undefined> {
  return step.then(
    () => undefined,
    (error: unknown) =>
      error instanceof Error ? error : new Error(String(error)),
  );
}

export class JscpdServer {
  private app: Express;
  private service: JscpdServerService;
  private mcp: McpEndpoint | null = null;
  private server: ReturnType<Express["listen"]> | null = null;

  constructor(
    workingDirectory: string,
    private options: ServerOptions = {},
  ) {
    this.service = new JscpdServerService(workingDirectory);
    this.app = express();
    this.publishAppState();
    this.setupMiddleware();
    this.setupRoutes();
    this.setupErrorHandlers();
  }

  private publishAppState(): void {
    setAppState(this.app, { service: this.service, mcp: this.mcp });
  }

  private bindHost(): string {
    return this.options.host || SERVER_DEFAULTS.HOST;
  }

  private networkPolicy() {
    const bindHost = this.bindHost();
    return {
      allowedOrigins: resolveAllowedOrigins(
        bindHost,
        this.options.allowedOrigins,
      ),
      allowedHosts: resolveAllowedHosts(bindHost, this.options.allowedHosts),
    };
  }

  /**
   * Opens a fresh MCP endpoint for a server run. `createMcpHandler` is closed
   * for good by `stop()`, so every run needs its own handler.
   */
  private async openMcpEndpoint(): Promise<void> {
    await this.closeMcpEndpoint();

    this.mcp = createMcpEndpoint(this.service, this.networkPolicy());
    this.publishAppState();
  }

  private async closeMcpEndpoint(): Promise<void> {
    const mcp = this.mcp;
    if (!mcp) {
      return;
    }

    this.mcp = null;
    this.publishAppState();
    await mcp.close();
  }

  private setupMiddleware(): void {
    this.app.use(morgan("dev"));
    this.app.use(express.json({ limit: SERVER_DEFAULTS.BODY_SIZE_LIMIT }));
    this.app.use(express.urlencoded({ extended: true }));

    this.app.use("/api", (_req, res, next) => {
      res.header("Content-Type", "application/json");
      next();
    });
  }

  private setupRoutes(): void {
    const guard = createNetworkGuard(this.networkPolicy());
    this.app.post("/api/check", guard);
    this.app.post("/api/recheck", guard);
    this.app.get("/api/stats", guard);

    const router = createRouter(this.service);
    this.app.use("/api", router);

    this.app.all(MCP_ENDPOINT, (req, res, next) => {
      const { mcp } = getRequestState(req);
      if (!mcp) {
        res.status(HTTP_STATUS.SERVICE_UNAVAILABLE).json({
          jsonrpc: "2.0",
          error: {
            code: MCP_SERVER_ERROR_CODE,
            message: ERROR_MESSAGES.MCP_NOT_STARTED,
          },
          id: null,
        });
        return;
      }
      mcp.handle(req, res).catch(next);
    });

    this.app.get("/", (_req, res) => {
      res.json({
        name: API_INFO.NAME,
        version: API_INFO.VERSION,
        endpoints: {
          "POST /api/check": "Check code snippet for duplications",
          "GET /api/stats": "Get overall project statistics",
          "GET /api/health": "Server health check",
          "POST /api/recheck": "Trigger recheck of the directory",
          [`POST ${MCP_ENDPOINT}`]: "MCP Protocol endpoint",
        },
        mcp: {
          protocolVersion: MCP_MODERN_PROTOCOL_VERSION,
          legacyCompatibility: true,
        },
        documentation: API_INFO.DOCUMENTATION_URL,
      });
    });
  }

  private setupErrorHandlers(): void {
    this.app.use(notFoundHandler);
    this.app.use(errorHandler);
  }

  async start(): Promise<void> {
    const port = this.options.port !== undefined ? this.options.port : SERVER_DEFAULTS.PORT;
    const host = this.bindHost();

    try {
      await this.service.initialize(this.options.jscpdOptions);
      await this.openMcpEndpoint();
      await this.listen(port, host);
    } catch (error) {
      // A partially started server must not stay half-open: release everything
      // this attempt claimed, without letting cleanup mask the original error.
      await this.rollbackStart();
      throw error;
    }
  }

  /**
   * Binds the HTTP listener. `listen` reports `EADDRINUSE` asynchronously, so
   * the failure arrives as an `error` event rather than a throw.
   */
  private listen(port: number, host: string): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;
      const server = this.app.listen(port, host);
      this.server = server;

      server.once("listening", () => {
        if (settled) {
          return;
        }
        settled = true;
        console.log(`JSCPD server running on http://${host}:${port}`);
        resolve();
      });

      server.once("error", (error) => {
        if (settled) {
          return;
        }
        settled = true;
        reject(error);
      });
    });
  }

  private async rollbackStart(): Promise<void> {
    await this.closeHttpServer();
    await settle(this.closeMcpEndpoint());
    await settle(this.service.close());
  }

  /**
   * Stops accepting connections and resolves once the open ones have drained.
   * Never rejects: a drain failure is returned so callers can finish tearing
   * the rest of the server down before surfacing it.
   */
  private closeHttpServer(): Promise<Error | undefined> {
    const server = this.server;
    this.server = null;

    if (!server) {
      return Promise.resolve(undefined);
    }
    if (!server.listening) {
      server.close();
      return Promise.resolve(undefined);
    }

    return new Promise((resolve) => {
      server.close((error) => resolve(error ?? undefined));
    });
  }

  /**
   * Shuts the server down. Every step runs even when an earlier one fails, so
   * a teardown failure can never leak the listener, the endpoint or the store.
   * The reported failure follows the order the steps run in: endpoint, then
   * HTTP drain, then service.
   */
  async stop(): Promise<void> {
    // Stop accepting new connections first, then abort the in-flight MCP
    // exchanges so the connections already open can finish and the drain can
    // complete instead of waiting on a stream that never ends.
    const drained = this.closeHttpServer();

    const endpointError = await settle(this.closeMcpEndpoint());
    const drainError = await drained;
    const serviceError = await settle(this.service.close());

    const failure = endpointError ?? drainError ?? serviceError;
    if (failure) {
      throw failure;
    }
  }

  getApp(): Express {
    return this.app;
  }

  getService(): JscpdServerService {
    return this.service;
  }
}

/**
 * Start jscpd server to check code snippets for duplications
 * @param workingDirectory - Base directory for codebase scanning
 * @param options - Server configuration options
 * @returns Promise resolving to the running server instance
 */
export async function startServer(
  workingDirectory: string,
  options: ServerOptions = {},
): Promise<JscpdServer> {
  const server = new JscpdServer(workingDirectory, options);
  await server.start();
  return server;
}
