---
name: compare-codebases
description: Compare two folders function by function with jscpd --compare, in the same language or across languages, and explain the result. Use when asked how two codebases relate, which functions one implementation has and the other lacks, how far a port has come, whether the iOS and Android versions of an app match, or which function in one folder corresponds to a function in the other.
---

# compare-codebases

`jscpd --compare A B` pairs every function of folder `A` with the function of folder `B` that does the same job, and lists the functions of each folder that have no counterpart. The two folders may be in one language or in two (Python and TypeScript, Kotlin and Swift, Java and Rust). This skill explains how the comparison works and how to run one well. For porting code with the comparison as the progress measure, use the [code-migration](../code-migration/SKILL.md) skill; for the rest of jscpd, the [jscpd](../jscpd/SKILL.md) skill.

## How the comparison works

jscpd finds the functions of JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift files in both folders. It turns the code of each function into a vector with a code embedding model that runs inside jscpd (CodeRankEmbed by default), so two functions that do the same job point the same way even when their languages, names and structure differ. Functions of one folder are never compared with each other.

Functions pair in two steps:

1. By code. Two functions pair when each is the other's closest match in the other folder, their cosine similarity reaches the model's threshold (0.4125 across languages and 0.6375 within one language, with CodeRankEmbed), and the similarity stands out from the function's other matches. A function close to the best one also pairs when it reaches a higher bar, so a feature written twice on one side gets two pairs. Functions shorter than `--min-tokens` (30 with `--compare`) or `--min-lines` (5) stay out of this step, because a short function resembles too many others.
2. By name. A function left over pairs with a function of the other folder under the same name, once case, underscores, spaces and punctuation are ignored (`encodeBinary`, `encode_binary`, `_encode_binary`; the test title `rounds cents` and `rounds_cents`), when their similarity reaches the `medium` level (0.5625 across languages with CodeRankEmbed). A name pair skips the closest-match and stand-out checks of the first step, so it needs more than that step's threshold; otherwise every `load` and `init` of two codebases would pair. Size does not matter here, so a short port is found. A name pair has to stay within modules the first step linked. A module is the folder right under the deepest folder all files of a side share (`notification` in `android/notification/…`); a file that sits higher than the rest, such as a build script, does not move that folder up. Two modules link when one holds the most of the other's code pairs. So `checkPermissions` of one plugin does not pair with its namesake in another.

Each pair gets a level on the scale of the model, because a cosine that is high for one model is low for another:

| Level | With CodeRankEmbed, across languages | Meaning |
|---|---|---|
| `high` | 0.7125 and up | almost always the same function |
| `medium` | 0.5625 to 0.7125 | usually the same function, restructured |
| `low` | 0.4125 to 0.5625 | read both: related code pairs here too |

Tests and code are measured apart, in two blocks of the report, and a test pairs only with a test. A test is told by the conventions of its language: a file such as `*_test.go`, `test_*.py`, `*.test.ts`, `*.spec.js`, `*Tests.swift` or `*_spec.rb`, a folder such as `tests/`, `__tests__/`, `spec/`, `src/test/` (where Java, Kotlin and Scala keep theirs) or `MyAppTests/` (the compared folder's own name counts), a Rust function in a `#[cfg(test)]` module or under `#[test]`, or a JavaScript test case. Without tests on either side the report has one block and no headings.

Totals count the functions of at least `--min-tokens` tokens and `--min-lines` lines; smaller ones appear only as partners. Anonymous functions (callbacks, closures) take no part, except JavaScript and TypeScript test cases: `it('rounds cents', () => …)` (and `test`, `specify`, `fit`, `xit`, `xtest`, `bench`, with `.only`, `.skip` or `.each(table)`) goes by its title, so tests pair like any other function. Suites and hooks stay anonymous. Types, constants, SQL and UI markup are not compared.

## A way to compare two folders

### 1. Pick the folders

- Point at the code, not the repositories: `app/src/main/java` and `ios/Sources`, not the two repository roots. Build output, vendored code and generated files dilute the result; jscpd already skips what `.gitignore` excludes, and `--ignore` takes more globs.
- Two folders are required, and they must not overlap: `app/` and `app/android/` is refused.
- Keep parallel structures when you can (`ios/<module>` and `android/<module>`). Modules steer the name step, so matching folder names help.
- For a port, put the source first and the target second, so the first line of the report is the port's progress. For two implementations that both live on, the order does not matter.
- Tests need no separate run: the report measures them in a block of their own. Leave them out with `--ignore "**/__tests__/**,**/*.test.*,**/test/**"` only when the user asks about the code alone, or compare the test folders alone (`--pattern`, or the two test folders as the paths) when they ask about the tests.

### 2. Get the model

The first run needs the model (548 MB). Ask the user before downloading it:

```bash
npx jscpd --semantic-download
```

jscpd caches the vectors per pair of folders, so later runs embed only the functions whose code changed.

### 3. Read the overview

```bash
npx jscpd --compare billing-py/ billing-ts/
```

```text
 71% 5 of 7 functions in billing-py/ have a counterpart in billing-ts/
 80% 4 of 5 functions in billing-ts/ have a counterpart in billing-py/

billing-py/
  file         paired  similarity  counterpart
  billing.py   4 / 5   0.89        billing.ts
  shipping.py  1 / 2   0.91        shipping.ts

billing-ts/
  file         paired  similarity  counterpart
  billing.ts   3 / 4   0.89        billing.py
  shipping.ts  1 / 1   0.91        shipping.py

Paired under other names (1):
  billing-py/                   billing-ts/             similarity
  billing.py:28 tax_for_region  billing.ts:27 salesTax  0.87 high

Only in billing-py/ (2):
  billing.py (1)
    46  due_date                6 lines
  shipping.py (1)
    18  estimate_delivery_days  8 lines

Only in billing-ts/ (1):
  billing.ts (1)
    39  toCurrency  8 lines
```

- When both folders hold tests, the report has a `Code` block and a `Tests` block, each with everything below. The two top lines of a block give the share of each folder's functions (or tests) that have a counterpart in the other.
- The file tables give each file's paired functions, the mean similarity of its pairs (with the number of `low` pairs, as in `0.62, 1 low`), and the file on the other side that holds most of its counterparts.
- "Paired under other names" lists the pairs whose names differ even once case and underscores are ignored. A search by name never finds these.
- "Only in" lists the functions with no counterpart, per folder, grouped by file with the number of each file's functions, then each function's first line, name and length.
- In colour, levels are green (`high`), yellow (`medium`) and red (`low`), and the shares are green when complete, yellow when partial and red when nothing is paired. Pass `--no-colors` when you parse the console output, or read the JSON report instead.
- If one folder has no functions, the report is one line of totals and `<folder> has no functions yet`. Check the path and the languages before concluding anything else.

### 4. Check the pairs

```bash
npx jscpd --compare billing-py/ billing-ts/ -r console-full
```

`console-full` adds every pair with its similarity and level, and marks the pairs found by name. Before reporting the comparison as reliable, read a sample:

- two or three `high` pairs, to confirm the pairing works on this code;
- every `low` pair, since related code pairs there too (a client call and the endpoint it calls, a function counting UTF-8 bytes and one converting a string to them);
- the pairs marked `by name`, since a same-named function can do something else.

When a sample pair is wrong, say which one and why.

### 5. Check the functions with no counterpart

A function in an "Only in" list is either missing on the other side, or its counterpart was not recognized. Before calling it missing, search the other folder: in its counterpart file, under other names, as part of a larger function, or replaced by a library or platform call. The known gaps in pairing:

- constructors across languages (a Java constructor and Rust's `new`, Kotlin's `constructor`, Swift's `init`) pair only when their code is similar enough;
- a short function renamed on the other side (`add_history` for `_finder_penalty_add_history`);
- one function split into several, or several merged into one: only the closest part may pair.

Functions that exist on one side only by design are expected: platform glue (an iOS delegate callback, an Android notification channel), helpers a language needs and another does not, features one side dropped.

### 6. Report

For the user, summarize in a few lines: the two percentages, the notable renamed pairs, the `low` pairs you checked and what you found, and the real gaps on each side, grouped by file or module. For a document or an issue, write the Markdown report and attach or paste it:

```bash
npx jscpd --compare billing-py/ billing-ts/ -r markdown -o .jscpd-compare
```

For your own processing, read the JSON report (`-r json`, written to `jscpd-compare.json`). It has a `code` and a `tests` section of the same shape. Each has `sides[0]` and `sides[1]`, each with `path`, `functions`, `matched`, `percentage`, `files` (`file`, `functions`, `matched`, `counterpart`, `similarity`, `lowPairs`) and `unmatched` (`file`, `name`, `start`, `end`), and `pairs`, each with `a`, `b`, `similarity`, `level`, `renamed` and `matchedBy`. Paths are relative to each folder. Write reports outside the repository or add the folder to `.gitignore`.

## Options that change the result

- `--min-tokens` and `--min-lines` set which functions count. Raise them to focus on substantial functions.
- `--ignore` and `--format` narrow the files, for example to leave out tests or build scripts.
- `--semantic-model` picks another model, and `--semantic-url` an OpenAI-compatible embeddings API; `npx jscpd --semantic-models` lists the models with calibrated thresholds. `--semantic-threshold` and `--semantic-same-threshold` move the thresholds; change them only after reading pairs on both sides of the new value.
- `--semantic-rebuild-cache` embeds everything again, which is needed only when the cache is suspect.

Keep the folders, their order, the options and the model the same when you compare runs over time, or the numbers stop being comparable. The exit code is 0 whatever the result.

## Limits

- Similarity does not see small differences in behavior: two versions that drifted apart still pair, often at `high`. Only tests and reading the code catch drift.
- The more one side is restructured, the fewer of its functions pair by code.
- Only functions are compared.
- The model runs on the CPU. The first run embedded about 25 functions a second on an Apple M1 (261 functions in 11 seconds) and can be several times slower on a small CI machine; later runs reuse the cache.
