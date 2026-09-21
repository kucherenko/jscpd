import { afterEach, vi } from 'vitest';

// The runner loads this before every suite; no test imports it.
vi.stubEnv('PORT', '0');
vi.stubEnv('DEPOT_REGION', 'test-north');

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});
