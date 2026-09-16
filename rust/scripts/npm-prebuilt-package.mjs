#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targets = JSON.parse(
  fs.readFileSync(path.join(root, "npm", "prebuilt-targets.json"), "utf8"),
);
/// Each product ships its own binary in its own platform packages, so a
/// release of one never forces a republish of the other.
const PRODUCTS = {
  jscpd: {
    binary: "jscpd",
    manifest: "package.json",
    wrappers: "[cpd](https://www.npmjs.com/package/cpd) / [jscpd](https://www.npmjs.com/package/jscpd)",
    summary: "a fast Rust implementation of the copy/paste detector",
    install: "npm install -g jscpd\n# or\nnpm install -g cpd",
  },
  basta: {
    binary: "basta",
    manifest: path.join("basta", "package.json"),
    wrappers: "[basta](https://www.npmjs.com/package/basta)",
    summary: "a fast Rust dead-code detector for JavaScript, TypeScript and Python",
    install: "npm install -g basta",
  },
};

function usage() {
  console.error(
    "usage: node scripts/npm-prebuilt-package.mjs --target <target> --bin-dir <dir> --out-dir <dir> [--product <product>]",
  );
  console.error(`targets: ${Object.keys(targets).join(", ")}`);
  console.error(`products: ${Object.keys(PRODUCTS).join(", ")} (default jscpd)`);
}

function readArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      usage();
      process.exit(2);
    }
    const key = arg.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      usage();
      process.exit(2);
    }
    args[key] = value;
    index += 1;
  }
  return args;
}

function exeName(name, os) {
  return os === "win32" ? `${name}.exe` : name;
}

function copyBinary(name, target, packageName, binDir, packageDir) {
  const fileName = exeName(name, target.os);
  const from = path.join(binDir, fileName);
  if (!fs.existsSync(from)) {
    console.error(`missing built binary for ${packageName}: ${from}`);
    process.exit(1);
  }

  const to = path.join(packageDir, "bin", fileName);
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
  if (target.os !== "win32") {
    fs.chmodSync(to, 0o755);
  }
}

const args = readArgs(process.argv.slice(2));
const target = targets[args.target];
const product = PRODUCTS[args.product ?? "jscpd"];
if (!target || !product || !args["bin-dir"] || !args["out-dir"]) {
  usage();
  process.exit(2);
}

const productPackage = JSON.parse(
  fs.readFileSync(path.join(root, product.manifest), "utf8"),
);
// `jscpd-darwin-arm64`, `basta-darwin-arm64`: the product name and the target
// key, which is exactly what each wrapper's platform map expects to resolve.
const packageName = `${product.binary}-${args.target}`;
const description = `Prebuilt ${target.platform} binaries for ${product.binary}`;

const LICENSE_PATH = path.join(root, "..", "LICENSE");
if (!fs.existsSync(LICENSE_PATH)) {
  console.error(`missing LICENSE file: ${LICENSE_PATH}`);
  process.exit(1);
}

const packageDir = path.resolve(args["out-dir"], packageName);
fs.rmSync(packageDir, { recursive: true, force: true });
fs.mkdirSync(packageDir, { recursive: true });

copyBinary(product.binary, target, packageName, path.resolve(args["bin-dir"]), packageDir);

fs.copyFileSync(
  LICENSE_PATH,
  path.join(packageDir, "LICENSE"),
);

fs.writeFileSync(
  path.join(packageDir, "README.md"),
  `# ${packageName}

${description}.

This is an optional native binary package for
${product.wrappers},
${product.summary}.

Do not install this package directly. Install the main package instead:

\`\`\`bash
${product.install}
\`\`\`

This package contains only the native \`${product.binary}\` binary
for its target platform, plus package metadata and license/readme files.

Supply-chain notes:

- no runtime dependencies;
- no install scripts or postinstall downloads;
- published from the project GitHub Actions workflow with npm provenance
  enabled;
- npm registry signatures and SLSA provenance can be checked with
  \`npm audit signatures\` from an installed project.

`,
);

const packageJson = {
  name: packageName,
  version: productPackage.version,
  description,
  license: productPackage.license,
  repository: productPackage.repository,
  keywords: [
    product.binary,
    "prebuilt",
    "native-binary",
    "platform-package",
    `${target.os}-${target.cpu}`,
  ],
  os: [target.os],
  cpu: [target.cpu],
  files: ["bin", "LICENSE", "README.md"],
  publishConfig: {
    access: "public",
  },
};

if (target.libc) {
  packageJson.libc = [target.libc];
}

fs.writeFileSync(
  path.join(packageDir, "package.json"),
  `${JSON.stringify(packageJson, null, 2)}\n`,
);

console.log(packageDir);
