"use strict";

const { platform, arch } = process;

const PLATFORM_MAP = {
  "linux-x64-gnu": {
    packageName: "cpd-linux-x64-gnu",
    os: "linux",
    cpu: "x64",
    libc: "glibc",
    rustTarget: "x86_64-unknown-linux-gnu",
    runner: "ubuntu-latest",
  },
  "linux-arm64-gnu": {
    packageName: "cpd-linux-arm64-gnu",
    os: "linux",
    cpu: "arm64",
    libc: "glibc",
    rustTarget: "aarch64-unknown-linux-gnu",
    runner: "ubuntu-latest",
  },
  "linux-x64-musl": {
    packageName: "cpd-linux-x64-musl",
    os: "linux",
    cpu: "x64",
    libc: "musl",
    rustTarget: "x86_64-unknown-linux-musl",
    runner: "ubuntu-latest",
  },
  "darwin-arm64": {
    packageName: "cpd-darwin-arm64",
    os: "darwin",
    cpu: "arm64",
    rustTarget: "aarch64-apple-darwin",
    runner: "macos-latest",
  },
  "darwin-x64": {
    packageName: "cpd-darwin-x64",
    os: "darwin",
    cpu: "x64",
    rustTarget: "x86_64-apple-darwin",
    runner: "macos-13",
  },
  "windows-x64-msvc": {
    packageName: "cpd-windows-x64-msvc",
    os: "win32",
    cpu: "x64",
    rustTarget: "x86_64-pc-windows-msvc",
    runner: "windows-latest",
  },
};

function detectLinuxLibc() {
  if (platform !== "linux") {
    return undefined;
  }

  const report =
    process.report && typeof process.report.getReport === "function"
      ? process.report.getReport()
      : undefined;
  if (report && report.header && report.header.glibcVersionRuntime) {
    return "glibc";
  }
  return "musl";
}

function getPlatformKey() {
  const libc = detectLinuxLibc();

  for (const [key, target] of Object.entries(PLATFORM_MAP)) {
    if (target.os !== platform || target.cpu !== arch) {
      continue;
    }
    if (target.os === "linux" && target.libc !== libc) {
      continue;
    }
    return key;
  }

  return undefined;
}

module.exports = { PLATFORM_MAP, getPlatformKey };