# Dashboard demo

`parcel-desk` is a five-file TypeScript service that quotes and labels
parcels. It has a little of everything the dashboard reports: an address
block copied between two modules, one function with most of the branching,
an export nobody calls and a legacy file nobody imports. Commands run from
the repository root at default thresholds.

| File                     | What it contributes                                        |
| ------------------------ | ---------------------------------------------------------- |
| `src/index.ts`           | the entry point (`"main"` in `package.json`)               |
| `src/rates.ts`           | `quoteShipment`: weight tiers, a zone `switch`, surcharges |
| `src/checks.ts`          | the address block, and `checkBatch`, which nothing calls   |
| `src/labels.ts`          | the same address block, copied                             |
| `src/legacy/manifest.ts` | a file no import reaches                                   |

## The whole picture

`--dashboard` runs clone detection, counts complexity and, for JavaScript,
TypeScript and Python, finds dead code, then prints one screen under the
project's health badge.

```bash
jscpd fixtures/dashboard-demo --dashboard --no-colors
# Health  B   74/100  █████████████████▊░░░░░░  93 lines of code (XS)
#   duplication   75  █████████░░░  5.4% in typescript (no text)
#   dead code     72  ████████▋░░░  14.0%
#   complexity    76  █████████▏░░  0.0% in complex files
#
# ── Project ─────────────────────────────────────────────────
#   7 files · 419 lines · 2.1K tokens · 3 formats
#   largest: bash 163, markdown 163, typescript 93 lines
#   Largest code files:
#     LINES  TOKENS  SIZE  PATH
#        35     182   757  src/rates.ts
#        21     169   665  src/checks.ts
#        16      97   423  src/index.ts
#        11     109   409  src/labels.ts
#        10     113   430  src/legacy/manifest.ts
#
# ── Duplication ─────────────────────────────────────────────
#   1.19% duplicated lines · 1 clone (1 exact)
#   By format:
#     DUP%  LINES  CLONES  FORMAT
#      5.4      5       1  typescript
#
# ── Complexity ──────────────────────────────────────────────
#   25 total · 5.0 mean per file
#   Most complex files:
#     CX  LINES  SIZE  PATH
#     11     35   757  src/rates.ts
#      6     21   665  src/checks.ts
#      3     11   409  src/labels.ts
#      3     10   430  src/legacy/manifest.ts
#      2     16   423  src/index.ts
#
# ── Dead code (JavaScript, TypeScript, Python) ──────────────
#   13.98% unused lines · 2 findings in 5 files
#   1 unused-file · 1 unused-export
#   Largest findings:
#     LINES  CATEGORY       WHERE
#        10  unused-file    src/legacy/manifest.ts
#         3  unused-export  src/checks.ts:19 checkBatch
```

What each section says about this project:

- **Health** — one 0-100 score with a grade, explained below.
- **Project** — seven files in three formats: the five TypeScript files, this
  README as markdown, and the `bash` inside its code fences, which jscpd
  scans as a language of its own. `package.json` is below the default
  `--min-tokens` and is not counted. The largest files are ranked by lines
  and only code is listed: this README is the longest file in the project,
  and nobody would split it.
- **Duplication** — `src/checks.ts` and `src/labels.ts` share the six-line
  address block, one exact clone. `--kind` filters it the same way it filters
  a clone report.
- **Complexity** — `src/rates.ts` scores 11: one function, three weight
  tiers, three `case` labels, the `&&` of the express check and the `||` of
  the surcharge. Arrow functions count as functions, which is why
  `src/checks.ts` scores 6 with only two `if`s.
- **Dead code** — `src/legacy/manifest.ts` is reached by no import from
  `src/index.ts`, and `checkBatch` is exported but never imported.

`--summary-top N` sets the rows per list (default 5). `-r json` writes the
whole screen to `jscpd-dashboard.json`; clone lists and dead-code findings
still come from the regular reporters.

The dashboard runs a clone scan and a dead-code scan. On a large tree it
takes about as long as the two run separately, since they share the CPU.

## Health

The badge on top of the dashboard is the project health score: duplication,
dead code and complexity, each turned into a 0-100 sub-score and combined
with a geometric mean, so one bad dimension is not averaged away. `--health`
prints the badge alone:

```bash
jscpd fixtures/dashboard-demo --health --no-colors
# Health  B   74/100  █████████████████▊░░░░░░  93 lines of code (XS)
#   duplication   75  █████████░░░  5.4% in typescript (no text)
#   dead code     72  ████████▋░░░  14.0%
#   complexity    76  █████████▏░░  0.0% in complex files
```

Each sub-score shows what it was measured from: the share of code lines that
are duplicated, that nothing runs, and that sit in complex files (complexity
50 or more). A sub-score is 100 at zero and halves at every half-life: 8.5%
duplication, 7.5% dead code, 50% of the code in complex files. This project
is tiny — 93 lines of code — so every share is pulled towards what a typical
project shows, and one finding does not sink it: that is why 14% dead code
still scores in the seventies. The pull is 2000 lines strong, so at fifty
thousand lines it no longer matters.

Grades: `A` from 85, `B` from 70, `C` from 55, `D` from 40, then `E`.

Other tools join through `--health-input`, a JSON file of metrics. A metric
is either a ready 0-100 `score`, or a `value` with the `halfLife` that turns
it into one (`"direction": "higher"` scores the distance to 100).
[`health-metrics.json`](health-metrics.json) adds 81% test coverage and a
clean security scan:

```bash
jscpd fixtures/dashboard-demo --health --health-input fixtures/dashboard-demo/health-metrics.json --no-colors
# Health  B   78/100  ██████████████████▊░░░░░  93 lines of code (XS)
#   duplication   75  █████████░░░  5.4% in typescript (no text)
#   dead code     72  ████████▋░░░  14.0%
#   complexity    76  █████████▏░░  0.0% in complex files
#   coverage      72  ████████▋░░░  81
#   security     100  ████████████
```

`-r json` writes `jscpd-health.json` (or `jscpd-dashboard.json` with
`--dashboard`, holding every section of the screen), `-r badge` writes
`jscpd-health-badge.svg`, `-r markdown`/`-r html` write the same score and
dimension table as `jscpd-health.md`/`jscpd-health.html` (or
`jscpd-dashboard.md`/`jscpd-dashboard.html` with `--dashboard`), and `-r ai`
prints one line. Half-lives, weights and metrics can also live in the
`health` object of `.jscpd.json`.

## Complexity only

`--complexity` skips clone detection and prints the `--summary` tables ranked
by complexity:

```bash
jscpd fixtures/dashboard-demo --complexity --no-colors --no-tips
#
# Complexity (by complexity; 6 files, 3 folders analyzed)
# Top files:
#   TOKENS  LINES  SIZE  CX  PATH
#      182     35   757  11  src/rates.ts
#      169     21   665   6  src/checks.ts
#      109     11   409   3  src/labels.ts
#      113     10   430   3  src/legacy/manifest.ts
#       97     16   423   2  src/index.ts
#     1289    163  7.9K   0  README.md
# Top folders:
#   FILES  TOKENS  LINES  SIZE  CX  PATH
#       4     557     83  2.2K   5  src
#       1     113     10   430   3  src/legacy
#       1    1289    163  7.9K   0  .
```
