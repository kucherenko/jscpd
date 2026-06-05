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
  { dir: "crates/cpd-core", version: "0.1.0" },
  { dir: "crates/cpd-tokenizer", version: "0.1.0" },
  { dir: "crates/cpd-finder", version: "0.1.0" },
  { dir: "crates/cpd-reporter", version: "0.1.0" },
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
        new RegExp(`^(${depName} = \\{ version = )"([^"]+)"(, path = "([^"]+)")\\}`, "m"),
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
      new RegExp(`^(${depName} = \\{ version = )"([^"]+)"(, path = "([^"]+)")\\}`, "m"),
      `$1"${depVersion}"$3}`,
    ]);
  }

  const changed = updateCargoToml(filePath, updates);
  console.log(`${changed ? "Updated" : "No change"} ${mainCrate.dir}/Cargo.toml version to ${mainCrate.version}`);
}

console.log(`Version sync complete: npm=${npmVersion}, sub-crates=${JSON.stringify(subCrateVersions)}`);