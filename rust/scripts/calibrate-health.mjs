#!/usr/bin/env node
// Recalibrates the health-score constants in crates/cpd-core/src/health.rs
// from the jscpd.dev trending corpus: every GitHub trending repo the site's
// pipeline health-scored in a rolling window of days, deduplicated per repo.
//
//   node rust/scripts/calibrate-health.mjs [--url <url>] [--file <path>]
//                                          [--days 7] [--write]
//
// The corpus is published daily at https://jscpd.dev/health-corpus.json; a
// local checkout of the jscpd.dev repo works too via --file. The window is
// anchored on the corpus's latest day, not the wall clock, so one corpus
// always yields one set of constants. A dry run prints a report; --write
// also patches health.rs.
//
// Each corpus day records the jscpd that measured it: a value means
// whatever that version excluded from the health shares (markup, data,
// text). The report names the versions in the window and warns when they
// mix — medians are only comparable to a score measured the same way.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_URL = "https://jscpd.dev/health-corpus.json";
const HEALTH_RS = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../crates/cpd-core/src/health.rs",
);
const DEFAULT_DAYS = 7;
// score(median) = 75 → 100 · 2^(−m/h) = 75 → h = m / log2(4/3).
const LOG2_4_3 = Math.log2(4 / 3);
const DIMENSIONS = ["duplication", "dead-code", "complexity"];

// Round to one decimal, nudged so an even-n median like (55.6 + 56.3) / 2 —
// 55.94999… in IEEE doubles — rounds to 56.0, not 55.9.
const round1 = (x) => Math.round((x + 1e-9) * 10) / 10;

// `56` would be an integer literal; health.rs writes its f64 constants as
// `56.0`.
const rustFloat = (x) => (Number.isInteger(x) ? `${x}.0` : `${x}`);

const formatLines = (n) =>
  n >= 1_000_000 ? `${round1(n / 1_000_000)}M` : n >= 1_000 ? `${round1(n / 1_000)}K` : `${n}`;

const offsetDay = (date, days) => {
  const d = new Date(`${date}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
};

function parseArgs(argv) {
  const args = { url: null, file: null, days: DEFAULT_DAYS, write: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--url") args.url = argv[++i];
    else if (argv[i] === "--file") args.file = argv[++i];
    else if (argv[i] === "--days") args.days = Number(argv[++i]);
    else if (argv[i] === "--write") args.write = true;
    else {
      console.error(`unknown argument: ${argv[i]}`);
      process.exit(1);
    }
  }
  if (!Number.isInteger(args.days) || args.days < 1) {
    console.error(`--days must be a positive integer, got ${args.days}`);
    process.exit(1);
  }
  return args;
}

async function loadCorpus({ url, file }) {
  if (file) {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  }
  const target = url || DEFAULT_URL;
  process.stderr.write(`fetching ${target}…\n`);
  const res = await fetch(target);
  if (!res.ok) {
    console.error(`cannot fetch ${target}: HTTP ${res.status}`);
    process.exit(1);
  }
  return res.json();
}

function median(sorted) {
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

// Every health-scored appearance in the window, one entry per repo: the
// latest one, so a repo trending several days is counted once. Each entry
// carries the jscpd that measured it — the corpus records one version per
// day, because the health shares mean whatever that version excluded
// (markup, data, text).
function corpusWindow(corpus, days) {
  const entries = [];
  for (const day of corpus.days || []) {
    for (const repo of day.repos || []) {
      entries.push({ ...repo, date: day.date, jscpdVersion: day.jscpdVersion ?? null });
    }
  }
  if (entries.length === 0) return { entries: [], from: null, to: null };
  const to = entries.reduce((max, e) => (e.date > max ? e.date : max), entries[0].date);
  const from = offsetDay(to, -(days - 1));
  const latest = new Map();
  for (const e of entries) {
    if (e.date < from || e.date > to) continue;
    const seen = latest.get(e.name);
    if (!seen || seen.date < e.date) latest.set(e.name, e);
  }
  return { entries: [...latest.values()].sort((a, b) => a.name.localeCompare(b.name)), from, to };
}

function calibrate(entries) {
  const constants = {};
  for (const id of DIMENSIONS) {
    const values = entries
      .map((e) => e.dimensions?.[id])
      .filter((v) => typeof v === "number")
      .sort((a, b) => a - b);
    if (values.length === 0) continue;
    const medianRounded = round1(median(values));
    constants[id] = {
      n: values.length,
      median: medianRounded,
      halfLife: round1(medianRounded / LOG2_4_3),
    };
  }
  return constants;
}

const CONSTANT_NAMES = { "duplication": "DUPLICATION", "dead-code": "DEAD_CODE", "complexity": "COMPLEXITY" };

function commentBlock(constants, { from, to }, days, entries, versions) {
  const lines = entries.map((e) => e.lines).sort((a, b) => a - b);
  const deadCodeN = constants["dead-code"]?.n ?? 0;
  const block = [
    `// Calibrated on the jscpd.dev trending corpus — a rolling ${days}-day window of`,
    `// GitHub trending (${entries.length} projects on ${from} to ${to},`,
    `// ${formatLines(lines[0])} to ${formatLines(lines[lines.length - 1])} lines of code; ${deadCodeN} of them`,
    `// with JavaScript, TypeScript or Python for dead code). Each half-life puts`,
    "// the median project at 75. Refresh both with",
    "// `node rust/scripts/calibrate-health.mjs --write`.",
  ];
  // Only when the whole window was measured by one jscpd — its exclusions
  // (markup, data, text) are what the medians mean. A mixed window says
  // nothing the numbers could stand on.
  if (versions.length === 1) block.splice(5, 0, `// Measured with jscpd ${versions[0]}.`);
  return block.join("\n") + "\n";
}

function report(constants, windowInfo, days, entries, versions) {
  const { from, to } = windowInfo;
  const lines = entries.map((e) => e.lines).sort((a, b) => a - b);
  console.log(`corpus: ${entries.length} repos, ${from} to ${to} (${days}-day window)`);
  console.log(`lines of code: ${formatLines(lines[0])} to ${formatLines(lines[lines.length - 1])}`);
  console.log(
    `jscpd: ${versions.length ? versions.join(", ") : "unknown — the corpus carries no version for these days"}`,
  );
  if (versions.length > 1) {
    console.warn(
      "warning: the window mixes values measured by different jscpd versions — their exclusion rules may differ",
    );
  }
  for (const id of DIMENSIONS) {
    const c = constants[id];
    if (!c) {
      console.log(`${id}: no measurements in the window, constant left as is`);
      continue;
    }
    if (c.n < 10) {
      console.warn(`warning: ${id} is measured over ${c.n} repos only — thin, grows ~12/day`);
    }
    console.log(`${id.padEnd(11)} median ${c.median}  half-life ${c.halfLife}  (n=${c.n})`);
  }
  console.log("\nhealth.rs constants:");
  for (const id of DIMENSIONS) {
    const c = constants[id];
    if (c) console.log(`const ${CONSTANT_NAMES[id]}: Calibration = Calibration { median: ${rustFloat(c.median)}, half_life: ${rustFloat(c.halfLife)} };`);
  }
  console.log("\ncorpus comment:");
  console.log(commentBlock(constants, windowInfo, days, entries, versions).trimEnd());
}

// Each pattern must match exactly once, or something moved and a stale
// constant would ship silently.
function replaceOnce(content, pattern, replacement, what) {
  const global = new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`);
  const count = [...content.matchAll(global)].length;
  if (count !== 1) {
    console.error(`cannot patch ${what}: expected 1 match, found ${count}`);
    process.exit(1);
  }
  return content.replace(pattern, replacement);
}

function writeConstants(constants, windowInfo, days, entries, versions) {
  let content = fs.readFileSync(HEALTH_RS, "utf8");
  for (const id of DIMENSIONS) {
    const c = constants[id];
    if (!c) continue;
    const name = CONSTANT_NAMES[id];
    content = replaceOnce(
      content,
      new RegExp(`const ${name}: Calibration = Calibration \\{\\n(\\s*)median: [\\d.]+,\\n\\s*half_life: [\\d.]+,\\n\\};`),
      `const ${name}: Calibration = Calibration {\n$1median: ${rustFloat(c.median)},\n$1half_life: ${rustFloat(c.halfLife)},\n};`,
      `const ${name}`,
    );
  }
  // The corpus comment: whatever contiguous `//` lines sit right above
  // `const DUPLICATION` — the block a previous run generated, or the
  // hand-written one it replaces.
  content = replaceOnce(
    content,
    /(?:^\/\/ .*\n)+(?=const DUPLICATION: Calibration)/m,
    commentBlock(constants, windowInfo, days, entries, versions),
    "the corpus comment above const DUPLICATION",
  );
  fs.writeFileSync(HEALTH_RS, content);
  console.log(`\nwrote ${path.relative(process.cwd(), HEALTH_RS)}`);
}

const args = parseArgs(process.argv.slice(2));
const corpus = await loadCorpus(args);
const { entries, from, to } = corpusWindow(corpus, args.days);
if (entries.length === 0) {
  console.error("no health-scored repos in the window");
  process.exit(1);
}
const constants = calibrate(entries);
const versions = [...new Set(entries.map((e) => e.jscpdVersion).filter(Boolean))].sort();
report(constants, { from, to }, args.days, entries, versions);
if (args.write) writeConstants(constants, { from, to }, args.days, entries, versions);