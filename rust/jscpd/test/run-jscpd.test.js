"use strict";

// Runs against the binary index.js finds: the platform package when it is
// installed, else rust/target/release/jscpd (the CI smoke job builds it
// first). Run with: node --test rust/jscpd/test/run-jscpd.test.js

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const packageDir = path.join(__dirname, "..");
const launcher = path.join(packageDir, "run-jscpd.js");
const { runJscpd, getJscpdReport } = require(packageDir);
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

// A temporary directory with two files that share one function: one clone at
// CLONE_ARGS. Removed after the test.
function projectWithClone(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "jscpd-test-"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const total = [
    "function total(items) {",
    "  let sum = 0;",
    "  for (const item of items) {",
    "    sum += item.price * item.quantity;",
    "  }",
    "  return sum;",
    "}",
    "",
  ].join("\n");
  fs.writeFileSync(path.join(dir, "a.js"), `${total}module.exports = total;\n`);
  fs.writeFileSync(path.join(dir, "b.js"), `const rate = 0.2;\n${total}`);
  return dir;
}

const CLONE_ARGS = ["--min-tokens", "10", "--min-lines", "3"];

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

test("getJscpdReport returns the report the json reporter writes", (t) => {
  const dir = projectWithClone(t);
  const args = [dir, ...CLONE_ARGS];
  const report = getJscpdReport(args);

  const output = path.join(dir, "report");
  const direct = runJscpd([...args, "--reporters", "json", "--output", output]);
  assert.equal(direct.status, 0, `${direct.stderr}`);
  const expected = JSON.parse(
    fs.readFileSync(path.join(output, "jscpd-report.json"), "utf8")
  );
  delete report.statistics.detectionDate;
  delete expected.statistics.detectionDate;
  assert.deepEqual(report, expected);

  // The keys index.d.ts types.
  assert.equal(report.statistics.total.clones, 1);
  const [duplicate] = report.duplicates;
  assert.deepEqual(Object.keys(duplicate).sort(), [
    "firstFile",
    "format",
    "fragment",
    "isNew",
    "kind",
    "lines",
    "secondFile",
    "tokens",
  ]);
  assert.deepEqual(Object.keys(duplicate.firstFile).sort(), [
    "end",
    "endLoc",
    "name",
    "start",
    "startLoc",
  ]);
  assert.deepEqual(Object.keys(duplicate.firstFile.startLoc).sort(), [
    "column",
    "line",
    "position",
  ]);
  assert.deepEqual(Object.keys(report.statistics.total).sort(), [
    "clones",
    "duplicatedLines",
    "duplicatedTokens",
    "lines",
    "newClones",
    "newDuplicatedLines",
    "percentage",
    "percentageTokens",
    "sources",
    "tokens",
  ]);
});

test("getJscpdReport takes paths after --", (t) => {
  const dir = projectWithClone(t);
  assert.equal(getJscpdReport([...CLONE_ARGS, "--", dir]).duplicates.length, 1);
});

test("getJscpdReport returns the report when --threshold fails the run", (t) => {
  const args = [projectWithClone(t), ...CLONE_ARGS, "--threshold", "0"];
  assert.equal(runJscpd(args).status, 1);
  assert.equal(getJscpdReport(args).duplicates.length, 1);
});

test("getJscpdReport replaces the config's reporters, output and reportName", (t) => {
  const dir = projectWithClone(t);
  fs.writeFileSync(
    path.join(dir, ".jscpd.json"),
    JSON.stringify({
      path: ["a.js", "b.js"],
      minTokens: 10,
      minLines: 3,
      reporters: ["console", "html"],
      output: "out",
      reportName: "custom",
    })
  );
  const report = getJscpdReport([], { cwd: dir });
  assert.equal(report.duplicates.length, 1);
  assert.deepEqual(fs.readdirSync(dir).sort(), [".jscpd.json", "a.js", "b.js"]);
});

test("no report: getJscpdReport throws JSCPD_NO_REPORT with the exit status and stderr", () => {
  const missing = path.join(__dirname, "no-such-path");
  assert.throws(
    () => getJscpdReport([missing]),
    (error) => {
      assert.equal(error.code, "JSCPD_NO_REPORT");
      assert.equal(error.status, 1);
      assert.match(error.stderr, /no-such-path/);
      assert.match(error.message, /^jscpd: no report \(exit code 1\)\n/);
      return true;
    }
  );
  assert.throws(() => getJscpdReport(["--no-such-flag"]), {
    code: "JSCPD_NO_REPORT",
    status: 2,
  });
});

test("getJscpdReport removes its temporary directory", (t) => {
  const project = projectWithClone(t);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "jscpd-test-"));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  t.mock.method(os, "tmpdir", () => tmp);
  getJscpdReport([project, ...CLONE_ARGS]);
  assert.throws(() => getJscpdReport(["--no-such-flag"]), {
    code: "JSCPD_NO_REPORT",
  });
  assert.deepEqual(fs.readdirSync(tmp), []);
});

test("getJscpdReport throws JSCPD_UNSUPPORTED_ARG for its own flags and other modes, before running", (t) => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "jscpd-test-"));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  t.mock.method(os, "tmpdir", () => tmp);
  const cases = [
    [["--output", "out"], "--output"],
    [["--output=out"], "--output"],
    [["-o", "out"], "-o"],
    [["-oout"], "-o"],
    [["--report-name", "x"], "--report-name"],
    [["--report-name=x"], "--report-name"],
    [["src", "--dead-code"], "--dead-code"],
    [["--basta"], "--basta"],
    [["--health"], "--health"],
    [["--compare", "old", "new"], "--compare"],
    [["--complexity"], "--complexity"],
    [["--dashboard"], "--dashboard"],
    [["--mcp"], "--mcp"],
    [["--lsp"], "--lsp"],
    [["--list"], "--list"],
    [["--help"], "--help"],
    [["-h"], "-h"],
    [["--version"], "--version"],
    [["-V"], "-V"],
    [["--debug"], "--debug"],
    [["--semantic-models"], "--semantic-models"],
    [["--semantic-download"], "--semantic-download"],
    [["--semantic-download=minilm"], "--semantic-download"],
    // Short flags in one arg.
    [["-so", "out"], "-o"],
    [["-sao"], "-o"],
    [["-sh"], "-h"],
    [["-sV"], "-V"],
  ];
  for (const [args, arg] of cases) {
    assert.throws(
      () => getJscpdReport(args),
      (error) => {
        assert.equal(error.code, "JSCPD_UNSUPPORTED_ARG", args.join(" "));
        assert.equal(error.arg, arg);
        assert.ok(
          error.message.startsWith(`jscpd: getJscpdReport: ${arg} `),
          error.message
        );
        return true;
      }
    );
  }
  // Nothing ran, so there was no temporary directory to remove.
  assert.deepEqual(fs.readdirSync(tmp), []);
});

test("getJscpdReport reads a short flag's value as a value, not as more flags", (t) => {
  const dir = projectWithClone(t);
  // -k10: "10" is the value of -k. -sa: --silent and --absolute.
  const report = getJscpdReport([dir, "-k10", "-l", "3", "-sa"]);
  assert.equal(report.duplicates.length, 1);
  assert.ok(path.isAbsolute(report.duplicates[0].firstFile.name));
});

test("a mode switched on in the config: JSCPD_NO_REPORT names the file jscpd wrote", (t) => {
  const dir = projectWithClone(t);
  fs.writeFileSync(path.join(dir, ".jscpd.json"), JSON.stringify({ deadCode: true }));
  assert.throws(
    () => getJscpdReport(["."], { cwd: dir }),
    (error) => {
      assert.equal(error.code, "JSCPD_NO_REPORT");
      assert.equal(error.status, 0);
      assert.equal(error.signal, null);
      assert.match(
        error.message,
        /^jscpd: no report \(exit code 0, it wrote basta-report\.json instead\)/
      );
      return true;
    }
  );
});

test("getJscpdReport reads flag names after -- as paths", (t) => {
  const dir = projectWithClone(t);
  fs.mkdirSync(path.join(dir, "--health"));
  fs.renameSync(path.join(dir, "b.js"), path.join(dir, "--health", "b.js"));
  const report = getJscpdReport([...CLONE_ARGS, "--", "a.js", "--health"], {
    cwd: dir,
  });
  assert.equal(report.duplicates.length, 1);
});

test("getJscpdReport drops stdout, so a console reporter can't fill spawnSync's buffer", (t) => {
  const args = [projectWithClone(t), ...CLONE_ARGS, "--reporters", "console-full"];
  // The console-full output alone is a few kilobytes.
  assert.equal(runJscpd(args, { maxBuffer: 512 }).error.code, "ENOBUFS");
  assert.equal(getJscpdReport(args, { maxBuffer: 512 }).duplicates.length, 1);
});
