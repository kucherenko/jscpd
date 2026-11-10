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

/**
 * Runs jscpd with `args` and `options` as runJscpd does, with the `json`
 * reporter writing into a temporary directory, and returns the parsed
 * `jscpd-report.json`. The directory is removed afterwards. The report is
 * returned whenever jscpd wrote it, even when a gate such as `--threshold`
 * failed the run. Throws `result.error` when the binary could not run, and an
 * Error with `code === "JSCPD_NO_REPORT"`, the exit `status` and `stderr` when
 * it wrote no report (for example, a path that does not exist).
 */
function getJscpdReport(args, options) {
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
      options
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
