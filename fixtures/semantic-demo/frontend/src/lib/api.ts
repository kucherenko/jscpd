// Thin fetch wrapper: JSON in and out, retries on network errors and 5xx.

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string
  ) {
    super(message);
  }
}

const RETRIES = 2;

export async function api<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
  let attempt = 0;
  for (;;) {
    try {
      const response = await fetch(path, {
        ...init,
        headers: { 'content-type': 'application/json', ...init.headers }
      });
      if (response.status >= 500 && attempt < RETRIES) {
        throw new ApiError(response.status, response.statusText);
      }
      const body = response.headers.get('content-type')?.includes('json')
        ? await response.json()
        : await response.text();
      if (!response.ok) {
        throw new ApiError(response.status, typeof body === 'string' ? body : body.error);
      }
      return body as T;
    } catch (err) {
      const retriable = !(err instanceof ApiError) || err.status >= 500;
      if (!retriable || attempt >= RETRIES) throw err;
      attempt += 1;
      await new Promise((resolve) => setTimeout(resolve, 250 * 2 ** attempt));
    }
  }
}

export function debounce<A extends unknown[]>(fn: (...args: A) => void, waitMs: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return (...args: A) => {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, waitMs);
  };
}
