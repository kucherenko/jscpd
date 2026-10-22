#!/usr/bin/env node
"use strict";

const { runJscpd } = require("./index");

let result;
try {
  result = runJscpd(process.argv.slice(2), { stdio: "inherit" });
} catch (error) {
  // Only the lookup errors are expected here. Anything else is a bug and
  // keeps its stack.
  if (error.code !== "JSCPD_NO_BINARY") {
    throw error;
  }
  console.error(error.message);
  process.exit(1);
}

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error(`jscpd: binary not found: ${result.error.path}`);
    process.exit(1);
  }
  throw result.error;
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 0);
