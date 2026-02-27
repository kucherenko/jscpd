import { Request, Response, Router } from 'express';
import { JscpdServerService } from './service';
import { validateCheckRequest } from './middleware';
import { CheckSnippetRequest, ErrorResponse } from './types';
import { HTTP_STATUS } from './constants';

function handleRouteError(
  res: Response,
  err: unknown,
  defaultErrorName: string,
  statusCode: number = HTTP_STATUS.BAD_REQUEST
): void {
  const error = err instanceof Error
    ? err
    : new Error(typeof err === 'string' ? err : 'An unexpected error occurred');

  const response: ErrorResponse = {
    error: error.name || defaultErrorName,
    message: error.message,
    statusCode,
  };
  res.status(response.statusCode).json(response);
}

export function createRouter(service: JscpdServerService): Router {
  const router = Router();

  router.post('/check', validateCheckRequest, async (req: Request, res: Response) => {
    try {
      const request: CheckSnippetRequest = req.body;
      const result = await service.checkSnippet(request);
      res.json(result);
    } catch (err) {
      handleRouteError(res, err, 'CheckError');
    }
  });
 
  router.post('/recheck', async (_req: Request, res: Response) => {
    try {
      await service.recheck();
      res.json({ message: 'Recheck started' });
    } catch (err) {
      handleRouteError(res, err, 'RecheckError');
    }
  });

  router.get('/stats', (_req: Request, res: Response) => {
    try {
      const result = service.getStatistics();

      if (!result.statistics) {
        const error: ErrorResponse = {
          error: 'NotReady',
          message: 'Statistics not available yet. Server is still initializing.',
          statusCode: HTTP_STATUS.SERVICE_UNAVAILABLE,
        };
        res.status(HTTP_STATUS.SERVICE_UNAVAILABLE).json(error);
        return;
      }

      res.json(result);
    } catch (err) {
      handleRouteError(res, err, 'StatsError', HTTP_STATUS.INTERNAL_SERVER_ERROR);
    }
  });

  router.get('/health', (_req: Request, res: Response) => {
    const state = service.getState();
    res.json({
      status: state.isScanning ? 'initializing' : 'ready',
      workingDirectory: state.workingDirectory,
      lastScanTime: state.lastScanTime,
    });
  });

  return router;
}

