"use strict";

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
    const fs = require("fs");
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

module.exports = { runJscpd };
