"use strict";

const path = require("path");
const { spawnSync } = require("child_process");
const {
  getPlatformKey,
  PLATFORM_MAP,
  describeHost,
  supportedPlatforms,
} = require("./platform-map");

/**
 * Runs this platform's jscpd binary with `args`, the way
 * `child_process.spawnSync` runs a command, and returns its result. Throws
 * when there is no binary for this platform.
 */
function runJscpd(args, options) {
  const key = getPlatformKey();
  if (!key) {
    throw new Error(
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
      throw new Error(
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
