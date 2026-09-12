#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);

const npmVersion = packageJson.version;

const subCrates = [
  { dir: "crates/cpd-core", version: "0.1.12" },
  { dir: "crates/cpd-tokenizer", version: "0.1.14" },
  { dir: "crates/cpd-finder", version: "0.1.15" },
  { dir: "crates/cpd-reporter", version: "0.1.13" },
];

const mainCrate = { dir: "crates/cpd", version: npmVersion };

const subCrateVersions = {};
for (const { dir, version } of subCrates) {
  subCrateVersions[dir.split("/")[1]] = version;
}

function updateCargoToml(filePath, updates) {
  let content = fs.readFileSync(filePath, "utf8");
  let changed = false;

  for (const [pattern, replacement] of updates) {
    const newContent = content.replace(pattern, replacement);
    if (newContent !== content) {
      content = newContent;
      changed = true;
    }
  }

  if (changed) {
    fs.writeFileSync(filePath, content);
  }
  return changed;
}

// Update sub-crate versions and their dependency references
for (const { dir, version } of subCrates) {
  const filePath = path.join(root, dir, "Cargo.toml");
  const crateName = dir.split("/")[1];
  const updates = [
    [/^version = ".*"$/m, `version = "${version}"`],
  ];

  // Update dependency versions for other sub-crates this crate depends on
  for (const [depName, depVersion] of Object.entries(subCrateVersions)) {
    if (crateName !== depName) {
      updates.push([
        new RegExp(`^(${depName} = \\{ version = )"([^"]+)"(, path = "([^"]+)"\\s*)\\}`, "m"),
        `$1"${depVersion}"$3}`,
      ]);
    }
  }

  const changed = updateCargoToml(filePath, updates);
  console.log(`${changed ? "Updated" : "No change"} ${dir}/Cargo.toml version to ${version}`);
}

// Update main crate (jscpd) version and its dependency references
{
  const filePath = path.join(root, mainCrate.dir, "Cargo.toml");
  const updates = [
    [/^version = ".*"$/m, `version = "${mainCrate.version}"`],
  ];

  for (const [depName, depVersion] of Object.entries(subCrateVersions)) {
    updates.push([
      new RegExp(`^(${depName} = \\{ version = )"([^"]+)"(, path = "([^"]+)"\\s*)\\}`, "m"),
      `$1"${depVersion}"$3}`,
    ]);
  }

  const changed = updateCargoToml(filePath, updates);
  console.log(`${changed ? "Updated" : "No change"} ${mainCrate.dir}/Cargo.toml version to ${mainCrate.version}`);
}

console.log(`Version sync complete: npm=${npmVersion}, sub-crates=${JSON.stringify(subCrateVersions)}`);

// Sync cpd package optionalDependencies (platform binary packages)
{
  const cpdPkgPath = path.join(root, "package.json");
  const cpdPkg = JSON.parse(fs.readFileSync(cpdPkgPath, "utf8"));
  let changed = false;
  for (const [dep, version] of Object.entries(cpdPkg.optionalDependencies || {})) {
    if (version !== npmVersion) {
      cpdPkg.optionalDependencies[dep] = npmVersion;
      changed = true;
    }
  }
  if (changed) {
    fs.writeFileSync(cpdPkgPath, `${JSON.stringify(cpdPkg, null, 2)}\n`);
    console.log(`Updated package.json optionalDependencies to ${npmVersion}`);
  } else {
    console.log(`No change package.json optionalDependencies (${npmVersion})`);
  }
}

// Sync jscpd wrapper package version and its platform pins.
//
// Version and optionalDependencies are updated independently on purpose. An
// earlier revision skipped the whole block when the version already matched,
// which meant anything that set `version` before this script ran (a manual
// edit, a partial release) left the platform pins stale — and they ship that
// way, so the wrapper resolves an older engine than it claims to be. That is
// how jscpd@5.1.0 went out pinned to the 5.0.16 binaries.
{
  const jscpdPkgPath = path.join(root, "jscpd", "package.json");
  const jscpdPkg = JSON.parse(fs.readFileSync(jscpdPkgPath, "utf8"));
  let changed = false;

  if (jscpdPkg.version !== npmVersion) {
    jscpdPkg.version = npmVersion;
    changed = true;
  }

  for (const [dep, version] of Object.entries(jscpdPkg.optionalDependencies || {})) {
    if (version !== npmVersion) {
      jscpdPkg.optionalDependencies[dep] = npmVersion;
      changed = true;
    }
  }

  if (changed) {
    fs.writeFileSync(jscpdPkgPath, `${JSON.stringify(jscpdPkg, null, 2)}\n`);
    console.log(`Updated jscpd/package.json to ${npmVersion}`);
  } else {
    console.log(`No change jscpd/package.json (${npmVersion})`);
  }
}

// PyPI needs PEP 440 versions: 5.3.0-beta.1 -> 5.3.0b1 (alpha/beta/rc only).
// Mirrors `pep440()` in build-pypi-wheels.py; keep the two in step. Anything
// else throws rather than guessing, since a wrong mapping would pin the
// pre-commit hook to a version that never reaches PyPI.
function toPep440(version) {
  const m = /^(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.?(\d+))?$/.exec(version);
  if (!m) throw new Error(`cannot map version ${version} to PEP 440`);
  const [, base, kind, num] = m;
  return kind ? `${base}${{ alpha: "a", beta: "b", rc: "rc" }[kind]}${num}` : base;
}
const pypiVersion = toPep440(npmVersion);

// Sync CITATION.cff: `version` follows the engine version, and `date-released`
// is set to today's UTC date whenever the version changes, so a release commit
// carries the right citation metadata without a manual edit.
{
  const citationPath = path.join(root, "..", "CITATION.cff");
  const citation = fs.readFileSync(citationPath, "utf8");
  const current = /^version: (.*)$/m.exec(citation)?.[1];
  if (current !== npmVersion) {
    const today = new Date().toISOString().slice(0, 10);
    const updated = citation
      .replace(/^version: .*$/m, `version: ${npmVersion}`)
      .replace(/^date-released: .*$/m, `date-released: ${today}`);
    fs.writeFileSync(citationPath, updated);
    console.log(`Updated ../CITATION.cff to ${npmVersion} (released ${today})`);
  } else {
    console.log(`No change ../CITATION.cff (${npmVersion})`);
  }
}

// Sync the repository-root pyproject.toml. It is a private project whose only
// job is to make `pre-commit` (language: python, see .pre-commit-hooks.yaml)
// install the `jscpd` binary from PyPI when a project points its hook at this
// repo, so its `version` and its exact `jscpd` pin follow the engine version.
{
  const pyprojectPath = path.join(root, "..", "pyproject.toml");
  const changed = updateCargoToml(pyprojectPath, [
    [/^version = ".*"$/m, `version = "${pypiVersion}"`],
    [/^dependencies = \["jscpd==[^"]*"\]$/m, `dependencies = ["jscpd==${pypiVersion}"]`],
  ]);
  console.log(`${changed ? "Updated" : "No change"} ../pyproject.toml to ${pypiVersion}`);
}

// Fail loudly rather than shipping a wrapper that resolves the wrong engine.
{
  const problems = [];
  for (const rel of ["package.json", "jscpd/package.json"]) {
    const pkg = JSON.parse(fs.readFileSync(path.join(root, rel), "utf8"));
    if (pkg.version !== npmVersion) {
      problems.push(`${rel}: version is ${pkg.version}, expected ${npmVersion}`);
    }
    for (const [dep, version] of Object.entries(pkg.optionalDependencies || {})) {
      if (version !== npmVersion) {
        problems.push(`${rel}: ${dep} pinned to ${version}, expected ${npmVersion}`);
      }
    }
  }
  {
    const citation = fs.readFileSync(path.join(root, "..", "CITATION.cff"), "utf8");
    const version = /^version: (.*)$/m.exec(citation)?.[1];
    if (version !== npmVersion) {
      problems.push(`../CITATION.cff: version is ${version}, expected ${npmVersion}`);
    }
  }
  {
    const pyproject = fs.readFileSync(path.join(root, "..", "pyproject.toml"), "utf8");
    const version = /^version = "([^"]*)"$/m.exec(pyproject)?.[1];
    if (version !== pypiVersion) {
      problems.push(`../pyproject.toml: version is ${version}, expected ${pypiVersion}`);
    }
    const pin = /^dependencies = \["jscpd==([^"]*)"\]$/m.exec(pyproject)?.[1];
    if (pin !== pypiVersion) {
      problems.push(`../pyproject.toml: jscpd pinned to ${pin}, expected ${pypiVersion}`);
    }
  }
  if (problems.length > 0) {
    console.error("Version sync verification FAILED:");
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.log(`Verified npm package versions and platform pins are all ${npmVersion}`);
}