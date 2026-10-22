import type {
  SpawnSyncOptions,
  SpawnSyncOptionsWithBufferEncoding,
  SpawnSyncOptionsWithStringEncoding,
  SpawnSyncReturns,
} from "child_process";

/**
 * Runs this platform's jscpd binary with `args`, the way
 * `child_process.spawnSync` runs a command, and returns its result.
 *
 * Throws an `Error` whose `code` is `"JSCPD_NO_BINARY"` when there is no
 * prebuilt binary for this platform or the platform package is not installed.
 * A binary that is missing on disk comes back in `result.error` (code
 * `ENOENT`), as from `spawnSync`.
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
