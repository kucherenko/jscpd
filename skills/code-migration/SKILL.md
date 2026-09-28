---
name: code-migration
description: Port a codebase to another language or framework function by function, and check two implementations of one app for parity, with jscpd --compare as the progress measure. Use when rewriting a project (Java to Kotlin, JavaScript to Rust, Python to TypeScript, a framework swap), when asked what is left to port, or when comparing the Android and iOS versions of an app.
---

# code-migration

`jscpd --compare OLD NEW` pairs every function of one folder with the function of the other folder that does the same job, in any pair of languages, and lists the functions that have no counterpart. This skill uses it as the measure of a migration: what is ported, what is left, and whether the port you just wrote was recognized. It works the same way for parity between two implementations (`android/` and `ios/`), where both lists matter.

jscpd finds functions in JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift. See the [jscpd](../jscpd/SKILL.md) skill for the rest of the tool.

## Setup

`--compare` pairs functions with a code embedding model that runs inside jscpd. Download it once (548 MB, CodeRankEmbed). Ask the user before you start the download:

```bash
npx jscpd --semantic-download
```

jscpd caches the vectors, so a repeat run embeds only the functions whose code changed and takes seconds.

Pick the two paths and keep them fixed for the whole migration: the old code first, the new code second. Two paths are required, and they must not overlap (`src/` and `src/new/` is refused). The new folder may be empty at the start.

## Measure

Run the console report for yourself and for the user:

```bash
npx jscpd --compare legacy/ src/
```

```text
 71% 5 of 7 functions in legacy have a counterpart in src
 80% 4 of 5 functions in src have a counterpart in legacy

legacy
  billing.py   4 / 5  → billing.ts
  shipping.py  1 / 2  → shipping.ts

Only in legacy (2):
  billing.py:46   due_date                6 lines
  shipping.py:18  estimate_delivery_days  8 lines

Only in src (1):
  billing.ts:39  toCurrency  8 lines
```

- The first line is the migration's progress. "Only in legacy" is the work left.
- The second line and "Only in src" describe the new code: helpers the port needed, new features, or a port jscpd did not recognize (see [Misses](#misses)).
- The arrow after a file names the file on the other side that holds most of its counterparts. Use it to decide where a missing function goes.
- With an empty new folder the report is one line of totals plus `src/ has no functions yet`.

For work you plan and track, read the JSON report instead of the console:

```bash
npx jscpd --compare legacy/ src/ -r json -o .jscpd-compare --silent
```

`.jscpd-compare/jscpd-compare.json` has `sides[0]` (old) and `sides[1]` (new), each with `path`, `functions`, `matched`, `percentage`, `files` (`file`, `functions`, `matched`, `counterpart`) and `unmatched` (`file`, `name`, `start`, `end`), and `pairs`, each with `a` (old), `b` (new), `similarity` and `matchedBy` (`code` or `name`). Paths are relative to each side's folder. Add `.jscpd-compare/` to `.gitignore` or write the report outside the repository.

`-r console-full` also prints every pair with its similarity, and `-r markdown` writes `jscpd-compare.md`, a table you can paste into a PR description or a migration issue.

## Migration loop

1. Run the JSON report and take the `unmatched` list of the old side.
2. Order the work. Port a function after the functions it calls: read each candidate and move leaf helpers first, since a caller ported before its helpers has nothing to call. Within that order, finish one file before starting the next, so each target file fills up in one go.
3. Before writing a port, search the new side for an equivalent that jscpd missed. Look in the counterpart file from `files`, under other names (constructors, merged functions, a library call that replaces the helper). If one exists, do not port the function again. Note the equivalent and move on.
4. Write the port in the counterpart file, or in the file the project's new layout puts it in. Keep the name recognizable in the new language's convention (`encodeBinary` becomes `encode_binary` in Rust or Python), because jscpd pairs short functions by name. Follow the style of pairs that are already done in the same file. `console-full` lists them.
5. Port the function's tests, or write them, and run them. A pair in the report says the two functions look alike. The tests say whether they behave alike.
6. Run the comparison again with the same paths and options. Check that the function has left `unmatched` and that its pair joins the right function on the new side. A pair to a different function means the port is not recognized, or it resembles the wrong thing; read both before going on.
7. Report progress to the user as the report prints it (`73% → 76%, 3 functions ported: …`), with the functions you decided not to port and why.

Repeat until the old side's `unmatched` list holds only functions you decided not to port. Typical reasons: dead code (check with `npx jscpd --dead-code` on the old side where it supports the language), code the new platform provides (a JSON parser, a retry helper), platform glue with no equivalent, and features the user dropped. List these in a migration notes file or the PR, one line each, so the remaining percentage is explained.

## Parity between two implementations

For `npx jscpd --compare android/ ios/` neither side is the source. Read both "Only in" lists:

- a function only on one side is either platform code (Android notification channels, iOS delegate callbacks) or a feature the other side lacks. Report each missing feature to the user before implementing it;
- for a feature you add to one side, the other side's version is the reference: read it, then write the port and its test.

jscpd pairs functions within modules that already match. A module is the folder right under the deepest folder each side's files share, so keep the two trees parallel (`android/<plugin>` and `ios/<plugin>`) for the best pairing.

## Reading pairs

- `matchedBy: code` means the two functions are each other's closest match and the similarity stands out. With the default model a similarity above about 0.7 is almost always the same function; between 0.4 and 0.55 read both, since related code pairs too (a function counting UTF-8 bytes paired with one converting a string to them).
- `matchedBy: name` means the names match once case and underscores are ignored, and the code is similar enough. It catches short ports. Read these pairs, because a same-named function can do something else.
- The similarity does not see small differences in behavior. Two versions that drifted apart still pair. Only tests and reading the code catch drift.

## Misses

jscpd compares functions only: types, constants, enums with data, SQL and UI markup are not in the report, so port and check them yourself. Known gaps in pairing:

- constructors across languages (a Java constructor and Rust's `new`, Kotlin's `constructor`, Swift's `init`) pair only when their code is similar enough;
- a short function renamed in the port (`add_history` for `_finder_penalty_add_history`);
- one function split into several, or several merged into one: the report may pair only the closest part and list the rest as unmatched;
- anonymous functions (callbacks, closures) take no part at all.

A function in "Only in NEW" that you know is a port of an old one is one of these. Do not rename working code only to raise the number, and never add stubs or empty functions with old names: jscpd may pair a stub by name, and the progress would then report work that was not done.

## Options

- `--min-tokens` (30 with `--compare`) and `--min-lines` (5) decide which functions count toward the totals. Smaller functions still pair as partners. Raise them to focus on substantial functions, and keep them fixed across runs so percentages compare.
- `--ignore "**/__tests__/**,**/*.test.*"` keeps tests out when the user wants the progress of production code only. Compare tests as their own pair of folders when both sides have them.
- `--format` narrows the walk to some languages, for example when the old side mixes the code being ported with build scripts.
- `--semantic-model` and `--semantic-url` pick another embedding model or an OpenAI-compatible API (`npx jscpd --semantic-models` lists the models with calibrated thresholds). Keep the model fixed across runs, because each model scores on its own scale.
- The exit code is 0 whatever the progress, so the command does not fail a build on its own.

## Rules

- Keep the two paths, the options and the model the same from run to run, or the numbers stop being comparable.
- Treat the report as a map of what to read and what is left. It does not prove the port is correct; tests do.
- Never game the percentage: no stubs, no renames made only for jscpd, no deleting old functions to shrink the denominator without the user's decision.
- When the report and your reading of the code disagree, trust the code and tell the user which pair looked wrong.
