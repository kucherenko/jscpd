import { Request, Response, NextFunction } from 'express';
import { ErrorResponse } from './types';
import { ERROR_MESSAGES, HTTP_STATUS } from './constants';

function sendValidationError(res: Response, message: string): void {
  const error: ErrorResponse = {
    error: 'ValidationError',
    message,
    statusCode: HTTP_STATUS.BAD_REQUEST,
  };
  res.status(HTTP_STATUS.BAD_REQUEST).json(error);
}

export function validateCheckRequest(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  const { code, format } = req.body;

  if (!code) {
    return sendValidationError(res, ERROR_MESSAGES.MISSING_REQUIRED_FIELD('code'));
  }

  if (typeof code !== 'string') {
    return sendValidationError(res, ERROR_MESSAGES.INVALID_FIELD_TYPE('code', 'string'));
  }

  if (code.trim().length === 0) {
    return sendValidationError(res, ERROR_MESSAGES.FIELD_CANNOT_BE_EMPTY('code'));
  }

  if (!format) {
    return sendValidationError(res, ERROR_MESSAGES.MISSING_REQUIRED_FIELD('format'));
  }

  if (typeof format !== 'string') {
    return sendValidationError(res, ERROR_MESSAGES.INVALID_FIELD_TYPE('format', 'string'));
  }

  if (format.trim().length === 0) {
    return sendValidationError(res, ERROR_MESSAGES.FIELD_CANNOT_BE_EMPTY('format'));
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
    statusCode: HTTP_STATUS.INTERNAL_SERVER_ERROR,
  };

  res.status(error.statusCode).json(error);
}

export function notFoundHandler(req: Request, res: Response): void {
  const error: ErrorResponse = {
    error: 'NotFound',
    message: `Route ${req.method} ${req.path} not found`,
    statusCode: HTTP_STATUS.NOT_FOUND,
  };
  res.status(HTTP_STATUS.NOT_FOUND).json(error);
}
