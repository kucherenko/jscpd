"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const {
  getPlatformKey,
  PLATFORM_MAP,
  describeHost,
  supportedPlatforms,
} = require("./platform-map");

/** `error.code` of the errors runJscpd throws itself. */
const NO_BINARY = "JSCPD_NO_BINARY";
/** `error.code` of the error getJscpdReport throws when jscpd wrote no report. */
const NO_REPORT = "JSCPD_NO_REPORT";
/** `error.code` of the error getJscpdReport throws for an arg it can't run with. */
const UNSUPPORTED_ARG = "JSCPD_UNSUPPORTED_ARG";

// Flags getJscpdReport sets itself. jscpd takes each of them once.
const OWN_FLAGS = new Set(["--output", "-o", "--report-name"]);
// Modes that write another file instead of jscpd-report.json, or none at
// all: --dead-code writes basta-report.json, --health jscpd-health.json,
// --mcp and --lsp wait for a client on stdin, --debug prints the config.
// A mode switched on another way (`"deadCode": true` in .jscpd.json) is
// caught after the run: the error names the files jscpd wrote instead.
const OTHER_MODES = new Set([
  "--dead-code",
  "--basta",
  "--compare",
  "--complexity",
  "--dashboard",
  "--health",
  "--mcp",
  "--lsp",
  "--list",
  "--debug",
  "--semantic-models",
  "--help",
  "-h",
  "--version",
  "-V",
]);
// jscpd's short flags that take a value: in `-sk30` or `-so out`, what
// follows the letter is its value, not more flags.
const SHORT_WITH_VALUE = new Set("klxmfiroctzp");

function codedError(code, message, fields) {
  return Object.assign(new Error(message), { code }, fields);
}

/**
 * Runs this platform's jscpd binary with `args`, the way
 * `child_process.spawnSync` runs a command, and returns its result. Throws an
 * Error with `code === "JSCPD_NO_BINARY"` when there is no prebuilt binary for
 * this platform or the platform package is not installed. A binary that is
 * missing on disk comes back in `result.error` (code `ENOENT`), as from
 * `spawnSync`.
 */
function runJscpd(args, options) {
  const key = getPlatformKey();
  if (!key) {
    throw codedError(
      NO_BINARY,
      `jscpd: no prebuilt binary for this platform: ${describeHost()}\n` +
        `jscpd: supported platforms: ${supportedPlatforms().join(", ")}\n` +
        "jscpd: build from source instead with `cargo install jscpd` " +
        "(https://github.com/kucherenko/jscpd/tree/master/rust)"
    );
  }

  const entry = PLATFORM_MAP[key];
  const binaryName = process.platform === "win32" ? "jscpd.exe" : "jscpd";

  let binaryPath;
  try {
    const pkgJson = require.resolve(`${entry.packageName}/package.json`, {
      paths: [path.resolve(__dirname, ".."), __dirname],
    });
    binaryPath = path.join(path.dirname(pkgJson), "bin", binaryName);
  } catch {
    const localBuild = path.join(
      __dirname,
      "..",
      "target",
      "release",
      binaryName
    );
    if (!fs.existsSync(localBuild)) {
      throw codedError(
        NO_BINARY,
        `jscpd: Platform package "${entry.packageName}" not installed.` +
          ` Install it with: npm install ${entry.packageName}\n` +
          "Alternatively, build from source: cargo build --release -p jscpd"
      );
    }
    binaryPath = localBuild;
  }

  return spawnSync(binaryPath, args, options);
}

function unsupportedArg(flag, message) {
  return codedError(UNSUPPORTED_ARG, `jscpd: getJscpdReport: ${message}`, {
    arg: flag,
  });
}

// The flags in one arg: `--name=value` gives `--name`, a cluster of short
// flags such as `-so` gives `-s` and `-o`.
function flagsOf(arg) {
  if (arg.startsWith("--")) {
    return [arg.split("=")[0]];
  }
  if (!arg.startsWith("-") || arg.length < 2) {
    return [];
  }
  const flags = [];
  for (const letter of arg.slice(1)) {
    flags.push(`-${letter}`);
    if (SHORT_WITH_VALUE.has(letter)) {
      break;
    }
  }
  return flags;
}

// Throws for an arg that would make jscpd fail or write no jscpd-report.json.
// Everything after `--` is a path.
function checkArgs(args) {
  const flags = [];
  for (const arg of args) {
    if (arg === "--") {
      break;
    }
    flags.push(...flagsOf(arg));
  }
  for (const flag of flags) {
    if (OWN_FLAGS.has(flag)) {
      throw unsupportedArg(
        flag,
        `${flag} is set by getJscpdReport, leave it out of args`
      );
    }
    if (OTHER_MODES.has(flag)) {
      throw unsupportedArg(
        flag,
        `${flag} does not write jscpd-report.json, run it with runJscpd`
      );
    }
  }
  // Alone, --semantic-download exits after the download.
  if (flags.includes("--semantic-download") && !flags.includes("--semantic")) {
    throw unsupportedArg(
      "--semantic-download",
      "--semantic-download without --semantic does not scan, run it with runJscpd"
    );
  }
}

// Why there is no report to return: `reason`, or else the files jscpd
// wrote instead (a mode switched on in the config writes its own file).
function noReport(result, dir, reason) {
  const stderr = String(result.stderr ?? "");
  const exit =
    result.status === null && result.signal
      ? `killed by ${result.signal}`
      : `exit code ${result.status}`;
  let detail = reason ? `, ${reason}` : "";
  if (!reason) {
    try {
      const written = fs.readdirSync(dir).sort();
      if (written.length) {
        detail = `, it wrote ${written.join(", ")} instead`;
      }
    } catch {
      // Gone already; nothing to name.
    }
  }
  return codedError(
    NO_REPORT,
    `jscpd: no report (${exit}${detail})` +
      (stderr ? `\n${stderr.trimEnd()}` : ""),
    { status: result.status, signal: result.signal, stderr }
  );
}

/**
 * Runs jscpd with `args` and `options` as runJscpd does, with the `json`
 * reporter writing into a temporary directory, and returns the parsed
 * `jscpd-report.json`. The directory is removed afterwards. The report is
 * returned whenever jscpd wrote it, even when a gate such as `--threshold`
 * failed the run. What jscpd prints to stdout is dropped unless
 * `options.stdio` says otherwise.
 *
 * Throws an Error with `code === "JSCPD_UNSUPPORTED_ARG"` and the flag in
 * `arg`, before running anything, when `args` set `--output` or
 * `--report-name`, or a mode that writes no jscpd-report.json (`--dead-code`,
 * `--health`, `--compare`, `--mcp`, `--debug`...). Throws `result.error` when
 * the binary could not run, and an Error with `code === "JSCPD_NO_REPORT"`,
 * the exit `status`, `signal` and `stderr` when there is no report to return
 * (for example, a path that does not exist, or `"deadCode": true` in the
 * config: the message names the files jscpd wrote instead).
 */
function getJscpdReport(args, options) {
  checkArgs(args);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "jscpd-"));
  try {
    // First, so that a `--` in args can't turn them into paths.
    const result = runJscpd(
      [
        "--reporters",
        "json",
        "--output",
        dir,
        "--report-name",
        "jscpd-report",
        ...args,
      ],
      // stdout is not returned, and an extra console reporter there could
      // overflow spawnSync's maxBuffer and kill the run.
      { stdio: ["pipe", "ignore", "pipe"], ...options }
    );
    if (result.error) {
      throw result.error;
    }
    let text;
    try {
      text = fs.readFileSync(path.join(dir, "jscpd-report.json"), "utf8");
    } catch {
      throw noReport(result, dir);
    }
    try {
      return JSON.parse(text);
    } catch (error) {
      throw noReport(
        result,
        dir,
        `jscpd-report.json is not valid JSON: ${error.message}`
      );
    }
  } finally {
    // A failed cleanup (Windows can hold a file that was just written) must
    // not replace the report or the error: leave the directory behind.
    try {
      fs.rmSync(dir, { recursive: true, force: true, maxRetries: 3 });
    } catch {
      // Left in os.tmpdir().
    }
  }
}

module.exports = { runJscpd, getJscpdReport };
