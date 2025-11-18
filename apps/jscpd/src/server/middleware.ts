import { Request, Response, NextFunction } from 'express';
import { ErrorResponse } from './types';
import { ERROR_MESSAGES, HTTP_STATUS } from './constants';

interface FieldValidation {
  name: string;
  type: 'string' | 'number' | 'boolean';
  required: boolean;
  allowEmpty?: boolean;
}

function sendValidationError(res: Response, message: string): void {
  const error: ErrorResponse = {
    error: 'ValidationError',
    message,
    statusCode: HTTP_STATUS.BAD_REQUEST,
  };
  res.status(HTTP_STATUS.BAD_REQUEST).json(error);
}

function validateField(
  value: unknown,
  validation: FieldValidation
): string | null {
  if (validation.required && (value === undefined || value === null)) {
    return ERROR_MESSAGES.MISSING_REQUIRED_FIELD(validation.name);
  }

  if (value !== undefined && value !== null) {
    if (typeof value !== validation.type) {
      return ERROR_MESSAGES.INVALID_FIELD_TYPE(validation.name, validation.type);
    }

    if (validation.type === 'string' && !validation.allowEmpty) {
      if ((value as string).trim().length === 0) {
        return ERROR_MESSAGES.FIELD_CANNOT_BE_EMPTY(validation.name);
      }
    }
  }

  return null;
}

export function validateCheckRequest(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  const validations: FieldValidation[] = [
    { name: 'code', type: 'string', required: true, allowEmpty: false },
    { name: 'format', type: 'string', required: true, allowEmpty: false },
  ];

  for (const validation of validations) {
    const error = validateField(req.body[validation.name], validation);
    if (error) {
      return sendValidationError(res, error);
    }
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
