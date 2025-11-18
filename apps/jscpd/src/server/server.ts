import express, { Express } from 'express';
import { JscpdServerService } from './service';
import { createRouter } from './routes';
import { errorHandler, notFoundHandler } from './middleware';
import { IOptions } from '@jscpd/core';
import { SERVER_DEFAULTS, API_INFO } from './constants';

export interface ServerOptions {
  port?: number;
  host?: string;
  jscpdOptions?: Partial<IOptions>;
}

export class JscpdServer {
  private app: Express;
  private service: JscpdServerService;
  private server: ReturnType<Express['listen']> | null = null;

  constructor(
    workingDirectory: string,
    private options: ServerOptions = {}
  ) {
    this.service = new JscpdServerService(workingDirectory);
    this.app = express();
    this.setupMiddleware();
    this.setupRoutes();
    this.setupErrorHandlers();
  }

  private setupMiddleware(): void {
    this.app.use(express.json({ limit: SERVER_DEFAULTS.BODY_SIZE_LIMIT }));
    this.app.use(express.urlencoded({ extended: true }));

    this.app.use((_req, res, next) => {
      res.header('Content-Type', 'application/json');
      next();
    });
  }

  private setupRoutes(): void {
    const router = createRouter(this.service);
    this.app.use('/api', router);

    this.app.get('/', (_req, res) => {
      res.json({
        name: API_INFO.NAME,
        version: API_INFO.VERSION,
        endpoints: {
          'POST /api/check': 'Check code snippet for duplications',
          'GET /api/stats': 'Get overall project statistics',
          'GET /api/health': 'Server health check',
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
    const port = this.options.port || SERVER_DEFAULTS.PORT;
    const host = this.options.host || SERVER_DEFAULTS.HOST;

    await this.service.initialize(this.options.jscpdOptions);

    return new Promise((resolve, reject) => {
      try {
        this.server = this.app.listen(port, host, () => {
          resolve();
        });

        this.server.on('error', (error) => {
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
  options: ServerOptions = {}
): Promise<JscpdServer> {
  const server = new JscpdServer(workingDirectory, options);
  await server.start();
  return server;
}
