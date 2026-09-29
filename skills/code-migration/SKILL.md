---
name: code-migration
description: Move code from one implementation to another function by function, tests first and code second, and check two implementations of one app for parity, with jscpd --compare as the progress measure and a coverage map binding each function to its tests. Use when porting a library or app to another language or framework (Java to Kotlin, JavaScript to Rust, a Python library to TypeScript, an iOS app to Android), when asked what is left to port, or when comparing the Android and iOS versions of an app.
---

# code-migration

`jscpd --compare SOURCE TARGET` pairs every function of one folder with the function of the other folder that does the same job, in any pair of languages, and lists the functions that have no counterpart. This skill uses it to measure a port: what is ported, what is left, and whether the port you just wrote was recognized.

A port runs in two phases: the tests first, then the code they check. A coverage report of the source's tests tells which tests exercise which function, so each function is ported together with the tests that prove it works. See [Plan: tests first, then code](#plan-tests-first-then-code).

Two words are used throughout:

- **source**: the implementation you port from, such as the iOS app, the Python library, or the JavaScript package;
- **target**: the implementation you port to, such as the Android app, the Rust crate, or the TypeScript rewrite.

Neither has to be old or new. The source may stay in production and keep changing, and the target may already hold features the source lacks. For two implementations that both live on (`ios/` and `android/`), the same report shows parity; see [Parity](#parity-between-two-implementations).

jscpd finds functions in JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro, Python, Rust, Go, Java, Kotlin, C#, C, C++, PHP, Ruby, Scala and Swift. The [compare-codebases](../compare-codebases/SKILL.md) skill explains how the comparison pairs functions and how to check its result; the [jscpd](../jscpd/SKILL.md) skill covers the rest of the tool.

## Setup

`--compare` pairs functions with a code embedding model that runs inside jscpd. Download it once (548 MB, CodeRankEmbed). Ask the user before you start the download:

```bash
npx jscpd --semantic-download
```

jscpd caches the vectors, so a repeat run embeds only the functions whose code changed and takes seconds.

Pick the two paths and keep them fixed for the whole port: the source first, the target second. Two paths are required, and they must not overlap (`app/` and `app/android/` is refused). The target may be empty at the start.

One run measures both phases: the report has a `Code` block and a `Tests` block (a `code` and a `tests` section in JSON), and a test pairs only with a test. jscpd tells a test by the conventions of its language: test files such as `*_test.go`, `test_*.py`, `*.test.ts` or `*Test.java`, folders such as `tests/`, `__tests__/` or `src/test/`, Rust tests in `#[cfg(test)]` modules, and JavaScript test cases such as `it('rounds cents', () => …)`. Phase 1 reads the `Tests` block, phase 2 the `Code` block.

## Measure

Run the console report for yourself and for the user. Here a Python billing library is being ported to TypeScript:

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

- The first line is the port's progress: the share of the source's functions that have a counterpart in the target. "Only in" the source is the work left.
- The second line and "Only in" the target describe the target's own code: helpers the port needed, features the source never had, or a port jscpd did not recognize (see [Misses](#misses)).
- Each file row gives its paired functions, the mean similarity of their pairs (with the number of `low` pairs, if any), and the counterpart file: the file on the other side that holds most of its counterparts. Use the counterpart to decide where a missing function goes.
- "Paired under other names" lists the pairs whose names differ even once case and underscores are ignored: renamed ports, constructors, platform names. Each has its similarity and level (see [Reading pairs](#reading-pairs)). These are ported already, even though a search by name would not find them.
- With an empty target the report is one line of totals plus `billing-ts/ has no functions yet`.

For work you plan and track, read the JSON report instead of the console:

```bash
npx jscpd --compare billing-py/ billing-ts/ -r json -o .jscpd-compare --silent
```

`.jscpd-compare/jscpd-compare.json` has `sides[0]` (the source) and `sides[1]` (the target), each with `path`, `functions`, `matched`, `percentage`, `files` (`file`, `functions`, `matched`, `counterpart`, `similarity`, `lowPairs`) and `unmatched` (`file`, `name`, `start`, `end`), and `pairs`, each with `a` (source), `b` (target), `similarity`, `level` (`high`, `medium`, `low`), `renamed` (the names differ) and `matchedBy` (`code` or `name`). Paths are relative to each side's folder. Add `.jscpd-compare/` to `.gitignore` or write the report outside the repository.

`-r console-full` also prints every pair with its similarity, and `-r markdown` writes `jscpd-compare.md`, a table you can paste into a PR description or a tracking issue.

## Plan: tests first, then code

Port the tests before the code. Ported tests define, before any target code exists, what it means for each function to be ported: when the function lands, its tests pass or show what still differs. A port written before its tests can only be checked against your reading of the source.

### Bind tests to functions with coverage

A test's name does not say which functions it runs: a test of `checkout` also runs `apply_discount` and `tax_for_region`. Coverage does. Before porting anything, build a map from each source function to the tests that exercise it.

1. Get a coverage report of the source's tests that says which test ran which lines: recorded per test, or per test file when the project's tooling has no per-test mode.
2. Take the source's functions and their lines from the `code` section of the JSON report. Every counted function is either in the source's `unmatched` list or the `a` side of a pair, each with `file`, `name`, `start` and `end`. Functions under `--min-tokens` or `--min-lines` are not listed there; take those from the coverage report's own function list, which most formats have.
3. A test covers a function when it runs a line from the function's `start` to its `end`. Write the map both ways, function to tests and test to functions, to a file next to the report (for example `.jscpd-compare/test-map.json`), and rebuild it when the source's tests change.

Use the map for three things:

- which tests to port with a function, and which functions a ported test needs before it can pass;
- the source functions no test covers. Before porting such a function, write a test for it on the source side that records what it does today (a characterization test), and port that test with the others. A port with no test has nothing to check it;
- the order of work: tests that need few functions come first, since they turn green soonest.

Coverage says which code a test runs, not what it checks. A function that a test only passes through on the way to another is weakly bound to that test; prefer the tests whose names or assertions are about the function.

### Phase 1: port the tests

1. Take the source's unmatched tests from the `tests` section of the JSON report. A JavaScript or TypeScript test case written as a callback, `it('rounds cents', () => …)`, goes by its title, so it pairs with `test_rounds_cents` in pytest or `rounds_cents` in Rust like any named test.
2. Port the tests file by file, into the target's test layout, against the API the target will have. Keep inputs and expected values exactly as they are. Where the target must behave differently (a platform limit, a language's number types), write the difference into the test with a comment and tell the user.
3. A ported test calls functions that do not exist yet, so it fails. In a compiled language it stops the whole test build. Keep such tests out of the build until their functions land, for example a module declaration left commented out, a cfg feature or an excluded source set, and turn them on one by one in phase 2. Do not write empty functions to make the tests compile: a stub under a source name can pair by name and count as ported.
4. Run the target's tests. The ported tests that pass already cover what the target has; the failing or disabled ones, read against the test map, are the work list for phase 2.

### Phase 2: port the code

Port the code one function at a time, with its tests already in place.

1. Run the JSON report and take the source's `unmatched` list from its `code` section.
2. Order the work. Start with functions whose tests are ported, and port a function after the functions it calls, since a caller ported before its helpers has nothing to call. Within that order, finish one file before starting the next, so each target file fills up in one go.
3. Before writing a port, make sure the target does not have it already. Pairs with `renamed: true` are ported functions under other names; they are not in `unmatched`, but check them when the user asks where a function went. Then search the target for an equivalent that jscpd missed: in the counterpart file from `files`, under other names (constructors, merged functions, a platform or library call that replaces the helper). If one exists, do not port the function again. Note the equivalent and move on.
4. Write the port in the counterpart file, or in the file the target's layout puts it in. Keep the name recognizable in the target language's convention (`encodeBinary` becomes `encode_binary` in Rust or Python), because jscpd pairs short functions by name. Follow the style of pairs that are already done in the same file. `console-full` lists them.
5. Turn on the function's ported tests, as the test map lists them, and run them. When they fail, fix the port, not the test, unless the user agrees that the target should behave differently. A pair in the report says the two functions look alike; the tests say whether they behave alike.
6. Run the target's coverage for those tests and check that they run the new function. A ported test that never reaches it is bound to something else on the target (a helper, a mock, an old path), and it proves nothing about the port.
7. Run the comparison again with the same paths and options. Check that the function has left `unmatched`, that its pair joins the right function in the target, and its level. A `high` pair is done. A `low` pair to your port usually means the port differs a lot from the source in structure; read both and make sure the behavior matches. A pair to a different function means the port is not recognized, or it resembles the wrong thing; read both before going on.
8. Report progress to the user with both measures, as the reports print them: tests ported and passing, and functions ported (`functions 73% → 76%, 3 ported: …; tests 41 of 44 ported, 38 passing`), with the functions you decided not to port and why.

Repeat until the source's `unmatched` list holds only functions you decided not to port. Typical reasons: dead code (check with `npx jscpd --dead-code` on the source where it supports the language), code the target platform or its libraries provide (a JSON parser, a retry helper), platform glue with no equivalent on the target (an iOS delegate callback, an Android notification channel), and features the user decided not to carry over. List these in a notes file or the PR, one line each, so the remaining percentage is explained.

When the source keeps changing during the port, a function that was paired can come back as unmatched after a rewrite, and new source functions join the list. Rerun the report before each batch of work instead of reusing an old list.

## Parity between two implementations

When both implementations live on, as the iOS and the Android app do, `npx jscpd --compare ios/ android/` shows parity. Read both "Only in" lists:

- a function on one side only is either platform code (Android notification channels, iOS delegate callbacks) or a feature the other side lacks. Report each missing feature to the user before implementing it;
- to add a missing feature, treat the side that has it as the source for that feature: bind its tests with coverage, port the tests, then the code, as in [Plan: tests first, then code](#plan-tests-first-then-code).

jscpd pairs functions within modules that already match. A module is the folder right under the deepest folder each side's files share, so keep the two trees parallel (`ios/<plugin>` and `android/<plugin>`) for the best pairing.

## Reading pairs

- `level` rates the similarity on the scale of the model in use, so it means the same with every model. `high` (0.7125 and up with the default CodeRankEmbed, across languages) is almost always the same function. `medium` is usually the same function, restructured. `low` needs reading both functions, since related code pairs there too (a function counting UTF-8 bytes paired with one converting a string to them). On the Tauri plugins, every `low` pair joined two differently named functions, and one of them joined two different plugins.
- `matchedBy: code` means the two functions are each other's closest match and the similarity stands out.
- `matchedBy: name` means the names match once case and underscores are ignored, and the code is similar enough. It catches short ports. Read these pairs, because a same-named function can do something else.
- The similarity does not see small differences in behavior. Two versions that drifted apart still pair. Only tests and reading the code catch drift.

## Misses

jscpd compares functions only: types, constants, enums with data, SQL and UI markup (storyboards, Android layouts) are not in the report, so port and check them yourself. Known gaps in pairing:

- constructors across languages (a Java constructor and Rust's `new`, Kotlin's `constructor`, Swift's `init`) pair only when their code is similar enough;
- a short function renamed in the port (`add_history` for `_finder_penalty_add_history`);
- one function split into several, or several merged into one: the report may pair only the closest part and list the rest as unmatched;
- anonymous functions (callbacks, closures) take no part at all, except JavaScript and TypeScript test cases such as `it('rounds cents', () => …)`, which go by their titles.

A function listed only in the target that you know is a port of a source function is one of these. Do not rename working code only to raise the number, and never add stubs or empty functions with source names: jscpd may pair a stub by name, and the progress would then report work that was not done.

## Options

- `--min-tokens` (30 with `--compare`) and `--min-lines` (5) decide which functions count toward the totals. Smaller functions still pair as partners. Raise them to focus on substantial functions, and keep them fixed across runs so percentages compare.
- `--ignore` leaves files out, such as generated code or fixtures; `--pattern` narrows the comparison to some files, such as the tests of one module. Keep the globs fixed across runs.
- `--format` narrows the walk to some languages, for example when the source mixes the code being ported with build scripts.
- `--semantic-model` and `--semantic-url` pick another embedding model or an OpenAI-compatible API (`npx jscpd --semantic-models` lists the models with calibrated thresholds). Keep the model fixed across runs, because each model scores on its own scale.
- The exit code is 0 whatever the progress, so the command does not fail a build on its own.

## Rules

- Keep the two paths, their order, the options and the model the same from run to run, or the numbers stop being comparable.
- Port the tests before the code, and a function only together with the tests the coverage map binds to it.
- Treat the report as a map of what to read and what is left. It does not prove the port is correct; tests do.
- Never game the percentage: no stubs, no renames made only for jscpd, no deleting source functions to shrink the denominator without the user's decision.
- When the report and your reading of the code disagree, trust the code and tell the user which pair looked wrong.
