import type {
  SpawnSyncOptions,
  SpawnSyncOptionsWithBufferEncoding,
  SpawnSyncOptionsWithStringEncoding,
  SpawnSyncReturns,
} from "child_process";

/**
 * Runs this platform's jscpd binary with `args`, the way
 * `child_process.spawnSync` runs a command, and returns its result. Throws
 * when there is no binary for this platform.
 */
export function runJscpd(
  args: readonly string[],
  options: SpawnSyncOptionsWithStringEncoding
): SpawnSyncReturns<string>;
export function runJscpd(
  args: readonly string[],
  options?: SpawnSyncOptionsWithBufferEncoding
): SpawnSyncReturns<Buffer>;
export function runJscpd(
  args: readonly string[],
  options?: SpawnSyncOptions
): SpawnSyncReturns<string | Buffer>;
