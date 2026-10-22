"use strict";

// Runs against the binary index.js finds: the platform package when it is
// installed, else rust/target/release/jscpd (the CI smoke job builds it
// first). Run with: node --test rust/jscpd/test/run-jscpd.test.js

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const packageDir = path.join(__dirname, "..");
const launcher = path.join(packageDir, "run-jscpd.js");
const { runJscpd } = require(packageDir);
const { getPlatformKey, PLATFORM_MAP } = require(
  path.join(packageDir, "platform-map")
);

// platform-map.js reads process.platform when it loads, so the stub has to
// be in place before the module: a preload in a child process.
const UNSUPPORTED_PLATFORM =
  'data:text/javascript,Object.defineProperty(process,"platform",{value:"sunos"})';

function node(args) {
  return spawnSync(process.execPath, args, { encoding: "utf8" });
}

test("runJscpd returns the binary's output and exit code", () => {
  const result = runJscpd(["--version"], { encoding: "utf8" });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0);
  assert.match(result.stdout, /\b\d+\.\d+\.\d+\b/);
});

test("the encoding option picks strings or Buffers, as with spawnSync", () => {
  assert.ok(Buffer.isBuffer(runJscpd(["--version"]).stdout));
  assert.equal(
    typeof runJscpd(["--version"], { encoding: "utf8" }).stdout,
    "string"
  );
});

test("the jscpd command prints what the binary prints and exits with its code", () => {
  for (const args of [["--version"], ["--no-such-flag"]]) {
    const direct = runJscpd(args, { encoding: "utf8" });
    const viaCommand = node([launcher, ...args]);
    assert.equal(viaCommand.stdout, direct.stdout, args.join(" "));
    assert.equal(viaCommand.stderr, direct.stderr, args.join(" "));
    assert.equal(viaCommand.status, direct.status, args.join(" "));
  }
  assert.notEqual(runJscpd(["--no-such-flag"]).status, 0);
});

test("an unsupported platform: runJscpd throws, the command exits 1", () => {
  const thrown = node([
    "--import",
    UNSUPPORTED_PLATFORM,
    "-e",
    `try {
       require(${JSON.stringify(packageDir)}).runJscpd(["--version"]);
       console.log("{}");
     } catch (error) {
       console.log(JSON.stringify({ code: error.code, message: error.message }));
     }`,
  ]);
  assert.equal(thrown.status, 0, thrown.stderr);
  const error = JSON.parse(thrown.stdout);
  assert.equal(error.code, "JSCPD_NO_BINARY");
  assert.match(error.message, /^jscpd: no prebuilt binary for this platform: sunos\//);

  const command = node(["--import", UNSUPPORTED_PLATFORM, launcher, "--version"]);
  assert.equal(command.status, 1);
  assert.equal(command.stdout, "");
  assert.match(command.stderr, /^jscpd: no prebuilt binary for this platform: sunos\//);
  assert.match(command.stderr, /jscpd: supported platforms: linux-x64-gnu, /);
});

test("a platform package without its binary: ENOENT in result.error, the command exits 1", (t) => {
  const key = getPlatformKey();
  if (!key) {
    t.skip("no platform package for this host");
    return;
  }
  const { packageName } = PLATFORM_MAP[key];
  try {
    require.resolve(`${packageName}/package.json`, {
      paths: [path.resolve(packageDir, ".."), packageDir],
    });
    t.skip(`${packageName} is installed`);
    return;
  } catch {
    // Not installed: index.js would fall back to target/release.
  }

  // A stand-in platform package with no bin/, in the last folder index.js
  // looks in. node_modules is gitignored; the stand-in is removed below.
  const nodeModules = path.join(packageDir, "node_modules");
  const hadNodeModules = fs.existsSync(nodeModules);
  const fakePackage = path.join(nodeModules, packageName);
  fs.mkdirSync(fakePackage, { recursive: true });
  fs.writeFileSync(
    path.join(fakePackage, "package.json"),
    JSON.stringify({ name: packageName, version: "0.0.0" })
  );
  try {
    const expectedBinary = path.join(
      fakePackage,
      "bin",
      process.platform === "win32" ? "jscpd.exe" : "jscpd"
    );
    // Child processes: Node caches a successful lookup per process, and the
    // other tests must keep finding the real binary.
    const direct = node([
      "-e",
      `const result = require(${JSON.stringify(packageDir)}).runJscpd(["--version"]);
       console.log(JSON.stringify({
         code: result.error && result.error.code,
         path: result.error && result.error.path,
         status: result.status,
       }));`,
    ]);
    assert.equal(direct.status, 0, direct.stderr);
    assert.deepEqual(JSON.parse(direct.stdout), {
      code: "ENOENT",
      path: expectedBinary,
      status: null,
    });

    const command = node([launcher, "--version"]);
    assert.equal(command.status, 1);
    assert.equal(command.stdout, "");
    assert.equal(command.stderr, `jscpd: binary not found: ${expectedBinary}\n`);
  } finally {
    fs.rmSync(hadNodeModules ? fakePackage : nodeModules, {
      recursive: true,
      force: true,
    });
  }
});
