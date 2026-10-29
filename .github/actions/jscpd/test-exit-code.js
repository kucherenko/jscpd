const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { test } = require("node:test");

const actionPath = path.resolve(__dirname, "../../../action.yml");
const actionLines = fs.readFileSync(actionPath, "utf8").split(/\r?\n/);
const stepStart = actionLines.indexOf("    - name: Run jscpd");
assert.notEqual(stepStart, -1, "Run jscpd step must exist");
const scriptStart = actionLines.indexOf("      run: |", stepStart);
assert.notEqual(scriptStart, -1, "Run jscpd must have a shell script");
const scriptLines = [];
for (const line of actionLines.slice(scriptStart + 1)) {
  if (line && !line.startsWith("        ")) break;
  scriptLines.push(line.slice(8));
}
const script = scriptLines.join("\n");
assert.ok(script.trim(), "Run jscpd shell script must not be empty");

for (const [exitCode, description] of [
  [0, "successful scan"],
  [1, "failed duplication gate"],
  [7, "custom --exit-code"],
  [127, "command execution failure"],
]) {
  test(`preserves exit code ${exitCode} for a ${description}`, () => {
    const temp = fs.mkdtempSync(path.join(os.tmpdir(), "jscpd-action-"));
    try {
      const output = path.join(temp, "run-output");
      const result = spawnSync(
        process.env.BASH_PATH || "bash",
        [
          "--noprofile", "--norc", "-e", "-o", "pipefail", "-c",
          `jscpd() { return "$TEST_JSCPD_EXIT_CODE"; }\n${script}`,
        ],
        {
          encoding: "utf8",
          env: {
            ...process.env,
            GITHUB_OUTPUT: output.replace(/\\/g, "/"),
            INPUT_PATH: ".",
            INPUT_REPORTERS: "json",
            INPUT_OUTPUT: temp.replace(/\\/g, "/"),
            TEST_JSCPD_EXIT_CODE: String(exitCode),
          },
        },
      );
      assert.ifError(result.error);
      assert.equal(result.status, exitCode, result.stderr);
      assert.equal(fs.readFileSync(output, "utf8").trim(), `exit-code=${exitCode}`);

      // Reporting must still work after a failed detection step.
      fs.writeFileSync(path.join(temp, "jscpd-report.json"), JSON.stringify({
        statistics: { total: {
          percentage: 20, clones: 2, duplicatedLines: 10, lines: 50, sources: 3,
        } },
      }));
      const parsed = spawnSync(process.execPath, [path.join(__dirname, "parse-output.js"), temp], {
        encoding: "utf8",
        env: { ...process.env, GITHUB_OUTPUT: output },
      });
      assert.equal(parsed.status, 0, parsed.stderr);
      const outputs = fs.readFileSync(output, "utf8");
      assert.match(outputs, /^duplication-percentage=20$/m);
      assert.match(outputs, /^clones-found=2$/m);
    } finally {
      assert.equal(path.dirname(path.resolve(temp)), path.resolve(os.tmpdir()));
      fs.rmSync(temp, { recursive: true, force: true });
    }
  });
}
