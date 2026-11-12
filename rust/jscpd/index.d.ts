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

/** A point in a file. */
export interface JscpdLocation {
  /** Counts from 1. */
  line: number;
  /** Bytes from the start of the line, counting from 0. */
  column: number;
  /** Bytes from the start of the file, counting from 0. */
  position: number;
}

/** One of the two fragments of a clone. */
export interface JscpdFragment {
  /** The file's path, as the other reports name it; absolute with `--absolute`. */
  name: string;
  /** The first line of the fragment. */
  start: number;
  /** The last line of the fragment. */
  end: number;
  startLoc: JscpdLocation;
  endLoc: JscpdLocation;
  /** With `--similarity`: the size of the function's normalized syntax tree. */
  nodes?: number;
  /** With `--blame`: the commit that last changed the fragment. */
  blame?: { commitSha: string; author: string };
}

/** One entry of `duplicates` in `jscpd-report.json`. */
export interface JscpdDuplicate {
  format: string;
  /** How many lines the first fragment spans. */
  lines: number;
  /** The first fragment's lines, whole. */
  fragment: string;
  tokens: number;
  firstFile: JscpdFragment;
  secondFile: JscpdFragment;
  /** Not in the baseline (`--baseline`, `--baseline-from-ref`, `--changed`). */
  isNew: boolean;
  kind: "exact" | "renamed" | "similar" | "semantic";
  /** A `similar` or `semantic` clone's similarity. */
  similarity?: number;
  /** How a `similar` clone was found: `--max-gap-lines` or `--similarity`. */
  method?: "gap" | "ast";
}

/** The statistics of all files, or of one format. */
export interface JscpdStatisticsRow {
  lines: number;
  tokens: number;
  sources: number;
  clones: number;
  duplicatedLines: number;
  duplicatedTokens: number;
  percentage: number;
  percentageTokens: number;
  newDuplicatedLines: number;
  newClones: number;
}

/** `jscpd-report.json`, the `json` reporter's output. */
export interface JscpdReport {
  statistics: {
    total: JscpdStatisticsRow;
    formats: Record<string, JscpdStatisticsRow>;
    detectionDate: string;
  };
  duplicates: JscpdDuplicate[];
  /** With `--summary`. */
  summary?: unknown;
  /** With `--history`. */
  history?: unknown;
  /** When the config has `boundaries` rules. */
  boundaries?: unknown;
}

/**
 * Runs jscpd with `args` and `options` as `runJscpd` does, with the `json`
 * reporter writing into a temporary directory, and returns the parsed
 * `jscpd-report.json`. The directory is removed afterwards. What jscpd
 * prints to stdout is dropped unless `options.stdio` says otherwise.
 *
 * Throws an `Error` whose `code` is `"JSCPD_UNSUPPORTED_ARG"`, with the flag
 * in `arg`, before running anything, when `args` set `--output` or
 * `--report-name`, or a mode that writes no `jscpd-report.json`
 * (`--dead-code`, `--health`, `--compare`, `--complexity`, `--dashboard`,
 * `--mcp`, `--lsp`, `--list`, `--help`, `--version`).
 *
 * The report is returned whenever jscpd wrote it, even when a gate such as
 * `--threshold` failed the run. Throws `result.error` when the binary could
 * not run, and an `Error` whose `code` is `"JSCPD_NO_REPORT"`, with the exit
 * `status` and `stderr`, when it wrote no report (for example, a path that
 * does not exist). Throws as `runJscpd` does when there is no binary.
 */
export function getJscpdReport(
  args: readonly string[],
  options?: SpawnSyncOptions
): JscpdReport;
