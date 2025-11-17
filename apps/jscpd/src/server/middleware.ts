import { Request, Response, NextFunction } from 'express';
import { ErrorResponse } from './types';

function sendValidationError(res: Response, message: string): void {
  const error: ErrorResponse = {
    error: 'ValidationError',
    message,
    statusCode: 400,
  };
  res.status(400).json(error);
}

export function validateCheckRequest(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  const { code, language, filename } = req.body;

  if (!code) {
    return sendValidationError(res, 'Missing required field: code');
  }

  if (typeof code !== 'string') {
    return sendValidationError(res, 'Field "code" must be a string');
  }

  if (code.trim().length === 0) {
    return sendValidationError(res, 'Field "code" cannot be empty');
  }

  if (language !== undefined && typeof language !== 'string') {
    return sendValidationError(res, 'Field "language" must be a string');
  }

  if (filename !== undefined && typeof filename !== 'string') {
    return sendValidationError(res, 'Field "filename" must be a string');
  }

  next();
}

export function errorHandler(
  err: Error,
  _req: Request,
  res: Response,
  _next: NextFunction
): void {
  console.error('Error:', err);

  const error: ErrorResponse = {
    error: err.name || 'InternalServerError',
    message: err.message || 'An unexpected error occurred',
    statusCode: 500,
  };

  res.status(error.statusCode).json(error);
}

export function notFoundHandler(req: Request, res: Response): void {
  const error: ErrorResponse = {
    error: 'NotFound',
    message: `Route ${req.method} ${req.path} not found`,
    statusCode: 404,
  };
  res.status(404).json(error);
}
