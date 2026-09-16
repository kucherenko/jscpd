#!/usr/bin/env node
"use strict";

const path = require("path");
const { spawnSync } = require("child_process");
const {
  getPlatformKey,
  PLATFORM_MAP,
  describeHost,
  supportedPlatforms,
} = require("./platform-map");

const key = getPlatformKey();
if (!key) {
  console.error(
    `basta: no prebuilt binary for this platform: ${describeHost()}`
  );
  console.error(
    `basta: supported platforms: ${supportedPlatforms().join(", ")}`
  );
  console.error(
    "basta: build from source instead with `cargo install basta` " +
      "(https://github.com/kucherenko/jscpd/tree/master/rust)"
  );
  process.exit(1);
}

const entry = PLATFORM_MAP[key];
const binaryName = process.platform === "win32" ? "basta.exe" : "basta";

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
  if (fs.existsSync(localBuild)) {
    binaryPath = localBuild;
  } else {
    console.error(
      `basta: Platform package "${entry.packageName}" not installed.` +
        ` Install it with: npm install ${entry.packageName}`
    );
    console.error(
      "Alternatively, build from source: cargo build --release -p basta"
    );
    process.exit(1);
  }
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
});

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error(`basta: binary not found: ${binaryPath}`);
    process.exit(1);
  }
  throw result.error;
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 0);
