import express, { Express } from "express";
import morgan from "morgan";
import { createMcpEndpoint, McpEndpoint } from "./mcp-http";
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
  /** Extra Origin header hostnames accepted by the MCP endpoint. */
  allowedOrigins?: string[];
  /** Host header hostnames the MCP endpoint answers on. */
  allowedHosts?: string[];
  jscpdOptions?: Partial<IOptions>;
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

  /**
   * Opens a fresh MCP endpoint for a server run. `createMcpHandler` is closed
   * for good by `stop()`, so every run needs its own handler.
   */
  private async openMcpEndpoint(): Promise<void> {
    await this.closeMcpEndpoint();

    const bindHost = this.bindHost();
    this.mcp = createMcpEndpoint(this.service, {
      allowedOrigins: resolveAllowedOrigins(
        bindHost,
        this.options.allowedOrigins,
      ),
      allowedHosts: resolveAllowedHosts(bindHost, this.options.allowedHosts),
    });
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

    await this.service.initialize(this.options.jscpdOptions);
    await this.openMcpEndpoint();

    return new Promise((resolve, reject) => {
      try {
        this.server = this.app.listen(port, host, () => {
          console.log(`JSCPD server running on http://${host}:${port}`);
          resolve();
        });

        this.server.on("error", (error) => {
          reject(error);
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  async stop(): Promise<void> {
    await this.closeMcpEndpoint();

    if (this.server) {
      return new Promise((resolve, reject) => {
        this.server!.close((err) => {
          if (err) {
            reject(err);
          } else {
            this.server = null;
            this.service.close().then(resolve).catch(reject);
          }
        });
      });
    }
    await this.service.close();
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
