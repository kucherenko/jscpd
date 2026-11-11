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
// --mcp and --lsp wait for a client on stdin.
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
  "--help",
  "-h",
  "--version",
  "-V",
]);

function noBinary(message) {
  const error = new Error(message);
  error.code = NO_BINARY;
  return error;
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
    throw noBinary(
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
      throw noBinary(
        `jscpd: Platform package "${entry.packageName}" not installed.` +
          ` Install it with: npm install ${entry.packageName}\n` +
          "Alternatively, build from source: cargo build --release -p jscpd"
      );
    }
    binaryPath = localBuild;
  }

  return spawnSync(binaryPath, args, options);
}

function unsupportedArg(arg, message) {
  const error = new Error(`jscpd: getJscpdReport: ${message}`);
  error.code = UNSUPPORTED_ARG;
  error.arg = arg;
  return error;
}

// Throws for an arg that would make jscpd fail or write no jscpd-report.json.
// Everything after `--` is a path.
function checkArgs(args) {
  for (const arg of args) {
    if (arg === "--") {
      return;
    }
    const flag = arg.startsWith("--")
      ? arg.split("=")[0]
      : arg.startsWith("-o")
        ? "-o"
        : arg;
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
 * `--health`, `--compare`, `--mcp`...). Throws `result.error` when the binary
 * could not run, and an Error with `code === "JSCPD_NO_REPORT"`, the exit
 * `status` and `stderr` when it wrote no report (for example, a path that
 * does not exist).
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
    const reportPath = path.join(dir, "jscpd-report.json");
    if (fs.existsSync(reportPath)) {
      return JSON.parse(fs.readFileSync(reportPath, "utf8"));
    }
    const stderr = String(result.stderr ?? "");
    const error = new Error(
      `jscpd: no report (exit code ${result.status})` +
        (stderr ? `\n${stderr.trimEnd()}` : "")
    );
    error.code = NO_REPORT;
    error.status = result.status;
    error.stderr = stderr;
    throw error;
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

module.exports = { runJscpd, getJscpdReport };
