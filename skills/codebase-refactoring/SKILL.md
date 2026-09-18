---
name: codebase-refactoring
description: Three-part workflow to improve overall codebase health with jscpd — find and fix duplicated code, find and remove or refactor dead code, then find and simplify the largest/most complex files. Starts from --dashboard/--health to prioritize and ends by re-measuring the score.
---

# codebase-refactoring

A guided pass over a codebase's three biggest maintenance costs — duplicated
code, dead code, and complexity — using jscpd to find each and a strategy to
fix it. Use this skill when asked to "clean up", "improve", "refactor" or
"pay down tech debt in" a codebase, rather than to fix one specific bug.

## Start here: measure, then prioritize

```bash
npx jscpd --health --reporters ai <path>
```

```
health 74 B (duplication 75, dead-code 72, complexity 76; 93 code lines)
```

The health score is a weighted mix of three sub-scores, each a share of the
code lines — high is good, 100 is clean. The lowest of the three names where
the codebase actually hurts most; if it isn't the order below, do that
dimension first instead. `n/a` on a dimension (e.g. `dead code n/a`) means
jscpd could not measure it — dead-code detection needs JavaScript,
TypeScript or Python to be a real share of the code — skip that step rather
than treating it as clean.

For the full picture behind that score — largest duplicated formats, most
complex files, largest dead-code findings, all in one screen — run `npx
jscpd --dashboard` instead. Its `console` output (the default) is the only
reporter with that per-file detail; `--reporters ai` on `--dashboard` prints
the same one-line summary shown above, not the breakdown, so don't reach for
it expecting more than the score. Use `--reporters json` on `--dashboard` if
you need the detail in a parseable form.

The three passes below are otherwise independent and safe to run in any
order; **duplication → dead code → complexity** is the default because
fixing duplication first means step 2 isn't wasting effort tracing call
sites that a later extract-function pass will move anyway, and clearing
dead code before step 3 means the files ranked for simplification are ones
actually worth simplifying, not ones about to be deleted.

## Step 1 — Duplicated code

```bash
npx jscpd --reporters ai --summary <path>
```

Find and read each clone, decide whether it is a real copy worth merging,
and apply the matching refactoring strategy (extract function, parameterize,
extract a shared module or constant, or a base class/template for
structural duplication). Full guided workflow, triage rules for
renamed/near-miss clones, and worked examples per clone kind:

→ **[dry-refactoring](../dry-refactoring/SKILL.md)** — run this now, then
come back here for steps 2 and 3. Its own `--summary` hotspot ranking is the
same one the health score's duplication dimension reads from, so a file it
flags as a duplication hotspot is the same file that dragged the score down.

## Step 2 — Dead code

```bash
npx jscpd --dead-code --reporters ai <path>
```

```
basta dead code report — 18 findings across 30 files (22.1% of 272 lines)
Confidence is 0-100; anything below 90 has a listed reason it may be wrong. Verify a finding before deleting the code.
unused-file typescript/src/legacy-export.ts:1:1 confidence=95 typescript/src/legacy-export.ts is never imported and is not an entry point
unused-export typescript/src/invoice.ts:21:17 confidence=85 exported function `renderReceipt` is never imported
unused-symbol typescript/src/invoice.ts:29:10 confidence=90 function `describeTotal` is never used in typescript/src/invoice.ts
unused-import typescript/src/invoice.ts:2:10 confidence=100 `roundToCents` is imported but never used
```

This is a graph traversal from the project's entry points (`package.json`
`main`/`bin`/`exports`/`scripts`, `pyproject.toml` scripts, framework
conventions, or `--entry <glob>`), not a reference count — a helper whose
only caller is itself dead is reported too, so deleting one finding can make
another true that wasn't reported yet. Covers JavaScript, TypeScript, JSX,
TSX, Vue, Svelte, Astro and Python.

### The four categories, and what to do with each

| Category | Meaning | Usual fix |
|----------|---------|-----------|
| `unused-file` | Never imported anywhere, not an entry point | Delete the file |
| `unused-export` | Exported, but no other file imports it | Delete it, or drop the `export` keyword if something in the same file still uses it |
| `unused-symbol` | A module-private declaration nothing in its own file references | Delete it |
| `unused-import` | Imported but never referenced in the file | Delete the import line |

### Workflow

1. Run with `--reporters ai` (add `--dead-code-categories` to focus on one
   category, `--include-tests` if test-only usage shouldn't count as "used",
   `--include-entry-exports` to also flag an entry file's own unused
   exports, normally excluded since its whole surface is the public API)
2. Sort by confidence, high to low. Everything is 0-100; below 90 the finding
   carries a stated reason it might be wrong — `eval`/`getattr` calls,
   decorators the analyzer doesn't recognize, a wildcard re-export, the name
   showing up only inside a string literal, or a file elsewhere that failed
   to parse (see `statistics.unparsedFiles` in the JSON report). Read the
   reason before deleting; `--min-confidence` (default 60) only sets the
   floor for what's reported, it doesn't make a low-confidence finding safe
3. For each finding above your comfort threshold, confirm it by hand — grep
   the symbol name across the repo (dynamic access, a string-based router, a
   template file jscpd doesn't parse can all hide a real use) — then apply
   the fix from the table above
4. Run the project's own test suite and build after each batch of deletions,
   not only jscpd's re-run: dead-code analysis proves nothing *reaches* the
   code from an entry point, not that removing it can't break a build step
5. Re-run `--dead-code` after a batch: a file whose only import was the one
   you just deleted may now be dead itself — that cascade is expected, keep
   going until a clean pass, not just once
6. Report what you skipped and why (low confidence, a reason you couldn't
   rule out, or intentionally-kept dead code such as a public library export)
   separately from what you actually removed

## Step 3 — Big and complex files

```bash
npx jscpd --complexity --reporters ai --summary-top 10 <path>
# or, ranked alongside duplication and size:
npx jscpd --reporters ai --summary --summary-by complexity <path>
```

```
Complexity by complexity (6 files, 6 folders):
files (tokens/lines/size/cx):
c/checkout.c 176/32/710/13
rust/status.rs 112/24/481/9
swift/Profile.swift 88/21/446/8
```

`cx` is a language-aware cyclomatic-complexity estimate — roughly one path
per function plus one per branch (`if`, loops, `&&`/`||`, `match`/`switch`
arms, the ternary and Elvis operators, each counted the way the language
actually spells it) — read as a ranking signal, not an exact metric. Prose
and data files (Markdown, JSON, YAML, lock files, …) always score `0`; a
large README is not a refactoring target. jscpd's own health score treats a
file with complexity **50 or higher** as "complex" (the threshold its
`dead code`-style half-life curve is calibrated against) — a reasonable
default cutoff for "worth reading" if the summary doesn't make an obvious
one clear.

### Workflow

1. Take the top N files by complexity (and, from the `--summary` form, by
   size — a huge low-complexity file, e.g. a big data table, is a different
   problem than a small high-complexity one)
2. Read each file and name the actual driver: deep nesting, a long function
   doing several unrelated things, a large `switch`/`if`-chain over a type
   or state, or a file that is really several modules glued together
3. Pick the matching strategy:
   - **Deep nesting** → guard clauses / early return, flatten the happy path
   - **Long function, several responsibilities** → extract function per
     responsibility, named for what it does
   - **Branching over a fixed set of cases** (type, enum, status) →
     replace with a lookup table/map, or polymorphism if each case already
     has its own type
   - **File doing too much** → split along its actual seams (one export
     group per file), not an arbitrary line-count cut
4. Refactor one file at a time; keep behavior identical — this is a
   complexity pass, not a rewrite. If a duplication clone (step 1) or a dead
   branch (step 2) turns up while reading, still handle it in its own step
   so the reason for each change stays traceable
5. Re-run `--complexity` on the touched files to confirm `cx` actually
   dropped — a split that only moves code around without reducing branching
   per function didn't fix anything, it just renamed the problem

## Step 4 — Confirm the improvement

```bash
npx jscpd --health --reporters ai <path>
```

Compare the score and grade to the Step 0 measurement. A dimension that got
worse (duplication crept back in while extracting a shared complexity fix,
say) is a signal to revisit that step, not just note it — re-run the
relevant pass (`--summary`, `--dead-code`, `--complexity`) rather than only
trusting the one aggregate number.

## Tips

- Don't run all three passes on a codebase you've never seen before without
  reading `--dashboard` first — on a project where only one dimension is
  actually bad, spending equal effort on all three wastes time on the two
  that are already fine
- A `--health-input` file (coverage, security scan results) can be layered
  onto the same score if the project already tracks those; it doesn't change
  anything about this workflow, it just adds more dimensions to Step 0
- Health scores from two runs are only comparable when built from the same
  dimensions — if a dimension went from `n/a` to measured (or the reverse)
  because the file mix changed, say so rather than reading the raw score
  delta as improvement or regression
- Commit each step separately (duplication fixes, dead-code removal,
  complexity simplification) — a reviewer verifying "did this actually
  reduce X" wants a diff scoped to X
