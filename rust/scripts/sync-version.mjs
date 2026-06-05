#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);

const version = packageJson.version;

const crates = [
  "crates/cpd-core",
  "crates/cpd-tokenizer",
  "crates/cpd-finder",
  "crates/cpd-reporter",
  "crates/cpd",
];

for (const crate of crates) {
  const cargoTomlPath = path.join(root, crate, "Cargo.toml");
  const content = fs.readFileSync(cargoTomlPath, "utf8");
  const updated = content.replace(
    /^version = ".*"$/m,
    `version = "${version}"`,
  );
  if (content !== updated) {
    fs.writeFileSync(cargoTomlPath, updated);
    console.log(`Updated ${crate}/Cargo.toml version to ${version}`);
  } else {
    console.log(`${crate}/Cargo.toml version already ${version} or not found`);
  }
}

console.log(`Version sync complete: ${version}`);