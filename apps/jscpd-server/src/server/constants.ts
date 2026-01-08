import { readPackageJson } from "../setup";

const packageJson = readPackageJson();

export const SERVER_DEFAULTS = {
  PORT: 3000,
  HOST: "0.0.0.0",
  BODY_SIZE_LIMIT: "10mb",
} as const;

export const ERROR_MESSAGES = {
  SCAN_IN_PROGRESS: "Please wait for initial scan to complete",
  NOT_INITIALIZED:
    "Server not initialized. Please wait for initial scan to complete.",
  SOURCE_STORE_NOT_INITIALIZED: "Source store not initialized",
  EMPTY_CODE: "Code snippet cannot be empty",
  MISSING_REQUIRED_FIELD: (field: string) => `Missing required field: ${field}`,
  INVALID_FIELD_TYPE: (field: string, expectedType: string) =>
    `Field "${field}" must be a ${expectedType}`,
  FIELD_CANNOT_BE_EMPTY: (field: string) => `Field "${field}" cannot be empty`,
} as const;

export const API_INFO = {
  NAME: "jscpd-server",
  VERSION: packageJson.version,
  DOCUMENTATION_URL: "https://github.com/kucherenko/jscpd",
} as const;

export const HTTP_STATUS = {
  OK: 200,
  BAD_REQUEST: 400,
  NOT_FOUND: 404,
  INTERNAL_SERVER_ERROR: 500,
  SERVICE_UNAVAILABLE: 503,
} as const;
