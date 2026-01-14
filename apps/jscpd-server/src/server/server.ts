import express, { Express } from "express";
import morgan from "morgan";
import { randomUUID } from "node:crypto";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { createMcpServer } from "./mcp-server";
import { JscpdServerService } from "./service";
import { createRouter } from "./routes";
import { errorHandler, notFoundHandler } from "./middleware";
import { IOptions } from "@jscpd/core";
import { SERVER_DEFAULTS, API_INFO } from "./constants";

export interface ServerOptions {
  port?: number;
  host?: string;
  jscpdOptions?: Partial<IOptions>;
}

export class JscpdServer {
  private app: Express;
  private service: JscpdServerService;
  private server: ReturnType<Express["listen"]> | null = null;
  private transports: { [sessionId: string]: StreamableHTTPServerTransport } =
    {};

  constructor(
    workingDirectory: string,
    private options: ServerOptions = {},
  ) {
    this.service = new JscpdServerService(workingDirectory);
    this.app = express();
    this.setupMiddleware();
    this.setupRoutes();
    this.setupErrorHandlers();
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

    this.app.post("/mcp", async (req, res) => {
      try {
        const sessionId = req.headers["mcp-session-id"] as string | undefined;
        let transport: StreamableHTTPServerTransport;

        if (sessionId && this.transports[sessionId]) {
          transport = this.transports[sessionId];
        } else if (!sessionId && req.body?.method === "initialize") {
          transport = new StreamableHTTPServerTransport({
            sessionIdGenerator: () => randomUUID(),
            enableJsonResponse: true,
            onsessioninitialized: (sessionId) => {
              console.log(`Session initialized with ID: ${sessionId}`);
              this.transports[sessionId] = transport;
            },
          });

          const server = createMcpServer(this.service);
          await server.connect(transport);
          await transport.handleRequest(req, res, req.body);
          return;
        } else {
          res.status(400).json({
            jsonrpc: "2.0",
            error: {
              code: -32000,
              message: "Bad Request: No valid session ID provided",
            },
            id: null,
          });
          return;
        }

        await transport.handleRequest(req, res, req.body);
      } catch (error) {
        console.error("Error handling MCP request:", error);
        if (!res.headersSent) {
          res.status(500).json({
            jsonrpc: "2.0",
            error: {
              code: -32603,
              message: "Internal server error",
            },
            id: null,
          });
        }
      }
    });

    this.app.get("/mcp", async (_req, res) => {
      res
        .status(405)
        .set("Allow", "POST")
        .json({ error: "Method Not Allowed" });
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
          "POST /mcp": "MCP Protocol endpoint",
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
    const host = this.options.host || SERVER_DEFAULTS.HOST;

    await this.service.initialize(this.options.jscpdOptions);

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
