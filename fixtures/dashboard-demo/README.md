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
TypeScript and Python, finds dead code, then prints one screen.

```bash
jscpd fixtures/dashboard-demo --dashboard --no-colors
# ── Project ─────────────────────────────────────────────────
#   7 files · 289 lines · 1.4K tokens · 3 formats
#   largest: bash 98, markdown 98, typescript 93 lines
#
# ── Duplication ─────────────────────────────────────────────
#   1.73% duplicated lines · 1 clone (1 exact)
#   Most duplicated files:
#     DUP%  LINES  PATH
#     45.5      5  src/labels.ts
#     23.8      5  src/checks.ts
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

- **Project** — seven files in three formats: the five TypeScript files, this
  README as markdown, and the `bash` inside its code fences, which jscpd
  scans as a language of its own. `package.json` is below the default
  `--min-tokens` and is not counted.

- **Duplication** — `src/checks.ts` and `src/labels.ts` share the six-line
  address block, one exact clone. `--kind` filters it the same way it filters
  a clone report.
- **Complexity** — `src/rates.ts` scores 11: one function, three weight
  tiers, three `case` labels, the `&&` of the express check and the `||` of
  the surcharge. Arrow functions count as functions, which is why
  `src/checks.ts` scores 6 with only two `if`s.
- **Dead code** — `src/legacy/manifest.ts` is reached by no import from
  `src/index.ts`, and `checkBatch` is exported but never imported.

`--summary-top N` sets the rows per list (default 5). The dashboard prints to
the console only; use the regular reporters for files.

The dashboard runs a clone scan and a dead-code scan. On a large tree it
takes about as long as the two run separately, since they share the CPU.

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
#      703     98  4.4K   0  README.md
# Top folders:
#   FILES  TOKENS  LINES  SIZE  CX  PATH
#       4     557     83  2.2K   5  src
#       1     113     10   430   3  src/legacy
#       1     703     98  4.4K   0  .
```
