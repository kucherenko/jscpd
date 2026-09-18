# Changelog

All notable changes to **cpd (Rust)** are documented here. Releases follow [Semantic Versioning](https://semver.org).

---

## 5.3.0

### New Features

- **`--dashboard` prints the whole picture of a project on one screen**: size and largest formats, duplication with the clone count per kind and a breakdown by format, total and mean complexity with the most complex files, and, for JavaScript, TypeScript and Python, dead code by category with the largest findings, all under a health badge (below). It runs a clone scan and a dead-code scan side by side, so it costs about as long as the slower of the two rather than their sum, and `--workers` is a budget for the pair rather than for one scan. `--summary-top` sets the rows per list, and every detection option (`--min-tokens`, `--kind`, `--ignore`, …) and dead-code option (`--entry`, `--dead-code-categories`, `--min-confidence`) applies to its own section. Reporters: `console`, `json` (`jscpd-dashboard.json`), `badge` (`jscpd-health-badge.svg`), and now `markdown`/`html` (`jscpd-dashboard.md`/`.html`). The exit gates of a clone run apply (`--threshold`, `--exit-code`, `--fail-on-empty`); the baseline family needs a clone report this mode does not write, so `--baseline`/`--baseline-from-ref`/`--update-baseline` warn and are ignored and `--fail-on-new-clones` is refused. See [`fixtures/dashboard-demo`](../fixtures/dashboard-demo/README.md).

- **`--health` scores a codebase from 0 to 100.** Three shares of the code lines — duplication, dead code, complexity — are each mixed with a "typical project" prior and scored on a half-life curve, then combined into one weighted geometric mean and a letter grade. Duplication does not count a duplicated markup, stylesheet or template block (HTML, CSS, Handlebars, …), since a repeating style rule is not the maintenance problem repeating programming logic is; component/script languages (Vue, Svelte, Astro, GraphQL) still count in full, and the console line says what it measured and, only when the project actually has files in that category, what it left out (`5.4% in typescript (no text)`). A dimension that cannot be measured — no JavaScript/TypeScript/Python for dead code, no complex files — is left out and named (`dead code n/a`) rather than scored as if it were clean. `--health-input FILE` (config key `healthInput`) folds in metrics from other tools (coverage, security, …), and the same object can live under a `health` key in `.jscpd.json` to tune the built-in dimensions' half-life and weight. Reporters: `console` (the badge), `ai` (one compact line), `json`, `badge`, and now `markdown`/`html`. See [`fixtures/dashboard-demo`](../fixtures/dashboard-demo/README.md#health).

- **`--complexity` answers the complexity half of `--summary` without the clone run**: files are walked and tokenized with the same filters, complexity is counted, and detection — the part that scales with the size of the codebase — never starts. Reporters: `console`, `ai` and `json` (`jscpd-complexity.json`). It cannot be combined with `--dead-code`, `--dashboard` or `--mcp`, and warns rather than silently ignoring options it has nothing to apply to (`--threshold`, `--exit-code`, the baseline family, `--history`, `--kind`).

- **`--kind` filters clones by how they were found**: `exact`, `renamed`, `similar`, or one of the two mechanisms behind `similar` — `gap` (`--max-gap-lines`) and `ast` (`--similarity`). Statistics, `--threshold` and every reporter see the filtered list. It never turns a detector on by itself: `--kind ast` without `--similarity` warns that no such clones can be found, and an unknown kind is an error rather than a typo that silently reports a clean scan.

- **`--summary --summary-by complexity` now reads like cyclomatic complexity** — the estimate was one path per *file* plus every branch keyword it could see, which was neither what the name promised nor consistent between languages. Three things changed, all inside the summary; detection, tokenization and clone output are untouched.

  *Short-circuit operators actually count now.* Only the JavaScript tokenizer emits `&&` as a single token; the generic one splits every punctuation run into characters, so for the other 200-odd formats the `&&` and `||` entries in the decision list had been unreachable — the most common branch operator in C, C++, C#, Java, Go, Rust, PHP, Swift and Kotlin contributed nothing. Adjacent punctuation is now joined before matching, which also stops `??` being counted twice.

  *A branch is counted the way the language spells it.* One global keyword list cannot be right for both Rust, where `match` arms are the branch and have no keyword at all, and Swift, where `?` marks an optional rather than a ternary. Per-language rules count Rust's match arms by their `=>` — every arm, less one per `match`, since three arms are three paths and so two branches — and add `guard` for Swift and `select` for Go, and treat `?` as a branch only where it opens a ternary — told apart from `String?` and `x?.y` by whether it is written against the token before it. `?:` counts as Elvis in Kotlin and Groovy, and as an optional property in TypeScript, which has no Elvis operator.

  *Complexity is one path per function, not per file.* Where a language has a marker the scan can trust (`def`, `fn`, `func`, `function`, `fun`, and `=>` for JavaScript arrow functions) the baseline is the number of functions; elsewhere it stays at one, as before. A docstring no longer adds branches either. The tokenizer closes strings at end of line, so a `"""` body used to arrive as ordinary words and a docstring saying "if the value is big" charged the file three branches; the quote characters on each line are now read back from the literals they arrive in, so `"""Summary.` opens a string, a lone `"""` closes it and `"""One line."""` leaves it as it was. This applies only to languages where `"""` really is a delimiter — Python, Kotlin, Scala, Groovy, Swift, Java, Julia, Elixir and Dart — because where a doubled quote is an escape (C# verbatim strings, VB.NET, SQL, Pascal) three quotes in a row are ordinary string content, and reading them as a delimiter swallowed the rest of the file.

  *A word the file binds as a name is that file's name.* `case`, `when` and `cond` are branch keywords in some languages and ordinary variables in others, and the tokenizer cannot help: `classify_word` returns `Identifier` for every word, so `if` in Python is tagged exactly like a variable called `case`. What a file does reveal is what it assigns to, what it reads off an object and what it lists as a parameter or argument, and a word it writes `case = 3`, `x.case` or `f(case, when)` for is no longer counted as a branch in that file.

  *The C family declares a function with no keyword at all.* `head(args) {` is the only marker C, C++, Java, Objective-C and C# give, and it has to be told from the `) {` that closes an `if`, a `switch` or a `for`. Tracking what preceded each open paren does that, so those languages now count functions like the rest instead of falling back to one path per file.

  Measured against [lizard](https://github.com/terryyin/lizard) on ten GitHub-trending projects across nine languages, agreement on file ranking rose from 0.83 to 0.92 (median Spearman) and the median ratio to lizard's own figure went from 0.75 to 1.00, with every one of the ten projects improving. C++ moved from 0.71 to 0.94 and C# from 0.82 to 0.99, both ratios landing on 1.00. Where the two still differ they differ on purpose: lizard counts a `match` once however many arms it has, and does not count `??`. [`fixtures/summary-demo`](../fixtures/summary-demo/README.md) has one file per language whose complexity can be counted by hand.

- **`--dead-code`: find code nothing runs** — a second question about the same tree, answered by a new engine (**basta**) that ships inside the jscpd binary and also stands alone as a `basta` command. It builds the import graph from the project's entry points and walks it, reporting unused files, unused exports, unused module-private declarations, unused imports and (opt-in) unused class members, across JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro and Python.

  Entry points come from `package.json` (`main`, `module`, `bin`, `exports`, `scripts`), `pyproject.toml` (`[project.scripts]` and entry-point tables) and conventions (`src/index.ts`, `__main__.py`, framework routes, `*.config.ts`, `.d.ts`, shebangs, `if __name__ == "__main__"`, every `__init__.py`); a manifest naming a built file (`./dist/index.js`) is mapped back to the source it was built from. `--entry <glob>` adds more.

  Because it is a graph traversal and not a reference count, dead code cascades: a helper whose only caller is itself dead is reported too. Every finding carries a confidence score from 0 to 100 and, below 100, the reasons it might be wrong — a file that calls `eval` or `getattr`, an unrecognised decorator, a wildcard re-export, a name that appears in a string literal, a file in the scan that did not parse. `--min-confidence` sets the floor (default 60).

  CommonJS is read alongside ESM — `require('./x')`, `const { a } = require('./x')`, `module.exports = { a, b }`, `exports.a = …` and a literal `import('./x')` are all edges — so a Node project that never touched `import` does not read as a pile of unreachable files. A project that renames its own import paths is read on its own terms: `compilerOptions.paths` and `baseUrl` from `tsconfig.json` or `jsconfig.json` are resolved, including through a relative `extends` chain and per-package in a monorepo, so the `@/components/x` alias that ships in the default Next.js template reaches the file it names instead of looking like a missing dependency. A file named only by a path string — a worker spawned through `new URL('./w.ts', import.meta.url)`, a build entry, a setup file listed in `vitest.config.ts` — counts as used. On the Python side a quoted type annotation is read as the forward reference it is, so the `if TYPE_CHECKING:` import that supplies it is not reported as unused, and a `src/` layout resolves `import mypkg.thing` from scripts that belong to no package. Entry points also include source files under `package.json`'s `files` and any file a shell script, CI workflow, Makefile or Dockerfile in the tree names by path. Files that fail to parse are listed in the console trailer and under `statistics.unparsedFiles` in the JSON report.

  Single-file components are read whole. A `.vue`, `.svelte` or `.astro` file is a JavaScript or TypeScript module wrapped in markup, and the markup is where a component's imports are used: `<Foo />` is the only thing that uses `import Foo from './Foo.vue'`. Both halves are analyzed — the script is masked in place rather than extracted, so every reported position is a position in the real file, and the markup is scanned for the names it reads, including Vue's kebab-case component spellings, `{{ … }}` interpolations, Svelte actions and transitions, props, and expressions inside attributes. Without that a component project would not merely lose findings; every component import in it would be reported as dead, and every module only a component imports would be reported as an unused file. `pages/`, `layouts/`, `src/routes/`, `app.vue` and `error.vue` are rooted as file-system routes. A framework that loads whole directories is read from its own config rather than guessed at from directory names, which would silence real findings in the projects that do write the import: a `nuxt.config.*` beside the tree roots `components/`, `composables/`, `utils/`, `middleware/`, `plugins/`, `modules/` and `server/`, in Nuxt 3's layout, Nuxt 4's `app/` layout and whatever `srcDir` names. When no config declares them, `~/x`, `@/x` and `~~/x` fall back to the project root — what they mean in Nuxt, whose own alias table is generated into `.nuxt/` and never scanned; a project that points `@` at `src/` says so in its `vite.config.*` or `tsconfig.json`, and that declaration wins.

  Import paths are read the way the project's own build reads them, because on real projects that is where most of the answer lives. `resolve.alias` from `vite.config.*` and `kit.alias` from `svelte.config.*` join `tsconfig.json`'s `paths` — a JS project that never adopted TypeScript keeps its alias table only in the bundler config, and SvelteKit's `$lib` is declared in a `.svelte-kit/tsconfig.json` that `svelte-kit sync` generates and no repository commits, so the convention itself is read instead. A computed specifier is read as the glob it stands for — ``import(`./services/${type}.vue`)`` reaches `services/*.vue`, `*` staying within one directory as it does for the bundler — and `import.meta.glob('./lang/**/*.ts')` with it, every pattern of an array form included; both used to produce nothing at all, so a router or an i18n table loaded that way reported every file it reached as dead. An `import()` written in a component's markup expression (`{#await import('./Heavy.svelte')}`) is an edge too, while the same words in a comment or in plain attribute text are not, and Astro's client `<script>` blocks are read as the separate modules Astro bundles them into. A quoted attribute value is read as an expression only on a directive (`:prop`, `@click`, `v-if`), so `class="card"` credits nothing. A path literal in a build config resolves against the project root as well as against the file holding it, which is what `context` means to a bundler. Nitro's `api/`, `routes/` and `middleware/` are file-system routes like Nuxt's directories.

  Inside a monorepo a package is imported by its *name* — `@acme/ui`, `@acme/ui/date` — from anywhere in the tree, and its own `package.json` is the only thing that says which directory that name means. Those names now resolve: `exports` is read subpath by subpath, preferring the source conditions over the built ones because `./dist/index.mjs` is a file the repository does not contain, and `main`/`module` fall back through the same build-output mapping as a published entry. The name reaches as far as the workspace does — found by walking up to a `pnpm-workspace.yaml`, a `lerna.json`, a `turbo.json` or a root manifest with `workspaces` — rather than as far as the package directory, since an app importing a package is never underneath it.

  Measured on thirty Vue, Svelte and Astro projects from GitHub trending against the previous release, reported unused files fall from 1093 to 481, and the share of them that some import in the tree names drops from 83% to 62%. A bundler query on an import (`./script.js?raw`) is stripped before resolving, since it says how a file is loaded, not which. Components imported only from `.mdx` content are still reported: basta does not read MDX, and `--entry` is the way to say they are used.

  It ships on npm as [`basta`](https://www.npmjs.com/package/basta) and on crates.io as [`basta`](https://crates.io/crates/basta), with its own version line (0.1.0), its own `basta-<platform>` prebuilt packages and its own release trigger — a `basta-v*` tag, not the push to master that releases jscpd — so a jscpd release never drags it along and it never holds one up. Inside the jscpd binary the same engine answers to `jscpd --dead-code`.

  Languages plug in through one trait (`basta::lang::Analyzer`) that owns parsing, specifier resolution, entry-point conventions, manifests and path traits; nothing outside `lang/` is language-specific, and [`docs/basta-extending.md`](../docs/basta-extending.md) walks through adding one.

  The mode reuses everything a jscpd user already knows: the same walker and filters (`--ignore`, `--format`, `.gitignore`, `--max-size`, `--follow-symlinks`), the same fifteen reporter names, the same `--threshold` and `--exit-code` gates. `--dead-code-categories` narrows what is reported; `--include-tests` and `--include-entry-exports` widen it. See [`fixtures/dead-code-demo`](../fixtures/dead-code-demo/README.md) and [the docs](../docs/rust.md#dead-code-detection---dead-code).

### Fixes

- A single wrong-typed field in `.jscpd.json` (`"entry": "src/index.js"` where an array was expected) discarded the whole config instead of just that field. Each top-level key is now checked in isolation, and only the ones that actually fail to parse are stripped and reported, so the rest of the config still applies.
- A hostile file path, custom format name or external health-metric id could break a Markdown table, inject raw HTML into a rendered dashboard, or forge extra table rows in the console output, since none of `--dashboard`, `--health` or their `markdown`/`html` reporters sanitized untrusted strings before interpolating them. Every such value is now escaped for its destination.
- `--dashboard`/`--health` refused the whole report when `--format` excluded every language the dead-code engine analyzes, instead of dropping just that section the way a project with none of those files already does; a bad `--dead-code-categories` or `--min-confidence` could also skip validation entirely when combined with such a format. Both are fixed: an unsupported format drops the dead-code section, and its options are always validated first.
- `--complexity --fail-on-empty` on an empty scan skipped writing reports (e.g. `-r json`) instead of writing them and then failing the process, unlike every other mode's `--fail-on-empty`.
- `--min-confidence` above 100 was accepted by `--dashboard`/`--health` (which build basta's config directly) even though the standalone `basta`/`--dead-code` CLI already clamped it with a warning; the clamp now applies everywhere the option is read.
- A health-score regression: markup-format duplication (HTML, CSS, templates, …) was weighted down for scoring but still counted in full toward the denominator, so excluding it barely moved the score on a real project. It is now a hard exclusion on both sides, and a project whose code is entirely markup skips the duplication dimension rather than scoring it from the size prior alone.
- Windows report paths used backslashes (`src\route.js`) where every other output already normalized to forward slashes, breaking any comparison against a fixture or a previous run.
- `basta` bumped its `oxc_*` parser crates to 0.150 (from 0.147).

---

## 5.2.1

### New Features

- **`--history`: duplication trend over git history** — `jscpd src --history v5.0.0..HEAD` scans every commit in the range in a detached worktree and prints a bar chart, a per-commit table with the change between points, the overall trend and how far `--threshold` could be tightened without failing the build. `--history-since`, `--history-every N` and `--history-limit N` narrow the range; the JSON reporter carries the points under a `history` key and the GitHub Action takes a `history` input. ([#1002](https://github.com/kucherenko/jscpd/issues/1002), [#1050](https://github.com/kucherenko/jscpd/pull/1050), [#1052](https://github.com/kucherenko/jscpd/pull/1052))
- **Exit codes you can gate on, and `--fail-on-empty`** — an unknown `--format`, a scan path that does not exist and a reporter that cannot write its file now print an error and exit 1 instead of passing with an empty report. `--fail-on-empty` (config key `failOnEmpty`, action input `fail-on-empty`) turns "analyzed no files" into a failure, so a mistyped path or an over-broad ignore cannot look like a clean run. ([#1047](https://github.com/kucherenko/jscpd/issues/1047), [#1049](https://github.com/kucherenko/jscpd/pull/1049))
- **PyPI: `pip install jscpd`** — the release now publishes eight platform wheels built from the same prebuilt binaries as the npm and GitHub Release artifacts, so `pip install jscpd` and `uvx jscpd` get the Rust engine with no Python code and no Node.js runtime involved. The repository-hosted pre-commit hook installs from PyPI instead of npm, which removes Node.js from the pre-commit path. ([#1037](https://github.com/kucherenko/jscpd/issues/1037), [#1039](https://github.com/kucherenko/jscpd/pull/1039))

### Bug Fixes

- **An open clone could be stretched past the file it started in** — while growing a clone the detector accepted a continuation from *any* stored occurrence of the next window, so a third file that shared the same text but continued differently could extend a fragment beyond what its own file contains. The clone was then dropped or reported with mismatched ends (`fixtures/haxe` reported `file1.hx [1:1 - 62:76]` against `file2.hx [1:1 - 62:2]`). The match now asks first whether the clone's own anchor continues, and starts a new clone when it does not, so N-way copies no longer lose pairs. ([#1033](https://github.com/kucherenko/jscpd/issues/1033), [#1034](https://github.com/kucherenko/jscpd/pull/1034))
- **The XML report could be rejected by every parser** — a clone containing a byte XML 1.0 cannot represent (an ANSI escape, a form feed) was written verbatim, and `xmllint` refused the file with `PCDATA invalid Char value 27`; `]]>` inside a fragment closed the CDATA section early, and attribute values were escaped twice. Such characters are now replaced with U+FFFD, `]]>` is split across two CDATA sections, and paths are escaped once. ([#375](https://github.com/kucherenko/jscpd/issues/375), [#1055](https://github.com/kucherenko/jscpd/pull/1055))
- **`--follow-symlinks` renamed and double-counted linked files** — a file reached through a symlink was reported by its resolved real path, which could be an absolute path outside the scan root, so the report and `--ignore` disagreed about its name; a file reachable through two paths counted as two sources, and a file symlink next to its target was reported as a clone of itself. Files now keep the path they were found at, and each real file is scanned once. ([#1059](https://github.com/kucherenko/jscpd/issues/1059), [#1060](https://github.com/kucherenko/jscpd/pull/1060))

### Other

- **Symlinks are skipped by default in v5** — v4 followed them unless `--noSymlinks` was set; v5 needs `--follow-symlinks` (config key `followSymlinks`, and a v4 `noSymlinks: false` still maps to following). This was true in every 5.x release but undocumented, and it silently drops a corpus mounted through a symlink. Now in the README and the migration table. ([#1059](https://github.com/kucherenko/jscpd/issues/1059))
- **`CITATION.cff` and a Citation section** — GitHub's "Cite this repository" button and a BibTeX entry for the papers that use jscpd as their detector. The version and release date are kept in step by `sync-version.mjs`. ([#1051](https://github.com/kucherenko/jscpd/pull/1051))
- **Docs: jscpd is language-aware** — the README and the Rust docs now say that detection runs on language tokens, per-format comment and string syntax with the oxc parser for JavaScript/TypeScript, rather than on raw text. ([#1048](https://github.com/kucherenko/jscpd/pull/1048))
- **Agent skills know about clone kinds, the summary and their noise** — the bundled `jscpd` and `dry-refactoring` skills (`npx skills add kucherenko/jscpd`) document `--summary`, the Type-2 and Type-3 flags with their kind suffixes, and warn that normalized passes surface look-alike code, with conservative defaults and a triage step before refactoring. ([#1056](https://github.com/kucherenko/jscpd/pull/1056), [#1057](https://github.com/kucherenko/jscpd/pull/1057))
- **`console-full` prints the `--history` block** like `console` does, and the test scaffolding behind the CLI, MCP, reporter and finder suites was deduplicated. ([#1053](https://github.com/kucherenko/jscpd/pull/1053))
- **CI: the npm platform-package gate polls against a 5-minute deadline** instead of a fixed sleep, so a slow registry no longer fails a release that would have succeeded. ([#1032](https://github.com/kucherenko/jscpd/pull/1032))

### Dependencies

- Bump `askama` from 0.16.0 to 0.16.1 in `/rust` ([#1045](https://github.com/kucherenko/jscpd/pull/1045))
- Bump `taiki-e/install-action` from 2.87.3 to 2.87.8 in `/.github/workflows` ([#1046](https://github.com/kucherenko/jscpd/pull/1046))

### Thank You ❤️

- [@mnahkies](https://github.com/mnahkies) for correcting the ignore examples in the README — `--ignore-pattern` has no short flag and a bare `node_modules` does not match, since globs are matched against the whole path ([#1038](https://github.com/kucherenko/jscpd/pull/1038))

---

## 5.2.0

### New Features

- **Type-2 clone detection: `--ignore-identifiers`, `--ignore-literals`, `--ignore-annotations`** — three opt-in flags (config keys `ignoreIdentifiers`, `ignoreLiterals`, `ignoreAnnotations`, GitHub Action inputs of the same names) normalize token classes before hashing, so blocks that differ only in names, literal values or annotations are found. Identifiers hash as one class while keywords keep their value, strings and numbers stay distinct classes, and `@Name(...)` runs are dropped in Java, Kotlin, Scala, Groovy, Python, Dart, Swift, JavaScript and TypeScript (`@interface` declarations are kept). Every clone now carries a `kind`: `exact` or `renamed`. A run without the flags is unchanged apart from the additive `"kind": "exact"` JSON field. See [`fixtures/type2-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/type2-demo/README.md). ([#998](https://github.com/kucherenko/jscpd/issues/998), [#1019](https://github.com/kucherenko/jscpd/pull/1019))
- **Near-miss clone merging with `--max-gap-lines N`** — a copy with a line inserted, removed or changed in the middle used to show up as two shorter clones. With `--max-gap-lines N` (config `maxGapLines`, Action input `max-gap-lines`, default `0` = off) clones of one file pair whose fragments follow each other in both files with at most `N` unmatched lines between them are merged into one clone of kind `similar` with a `similarity` value (matched tokens over the merged span). A merge whose similarity would fall below `0.5` is refused, duplicated-line statistics count only the matched lines, and a merge of renamed halves is reported as `similar`. See [`fixtures/type3-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/type3-demo/README.md). ([#999](https://github.com/kucherenko/jscpd/issues/999), [#1020](https://github.com/kucherenko/jscpd/pull/1020), [#1030](https://github.com/kucherenko/jscpd/pull/1030))
- **Function-level similarity for JavaScript and TypeScript with `--similarity RATIO`** — edits spread through a function rather than concentrated in one gap still escape a token window. `--similarity` (config `similarity`, Action input `similarity`, a number in `(0, 1]`; the default `1` means exact matches only, so nothing runs until you lower it) compares every function, method and arrow function by the bag of 4-grams over its syntax-tree node types, indexed with MinHash, and reports pairs at or above the ratio as `similar` clones spanning the whole functions. Names and literals do not take part: a renamed copy scores `1.0`, one inserted line about `0.9`, two inserted statements plus renames about `0.75`. Every `similar` clone records its `method` (`gap` or `ast`) because the two scores are not on the same scale. The MCP `check_duplication` tool accepts the same `similarity` argument. ([#999](https://github.com/kucherenko/jscpd/issues/999), [#727](https://github.com/kucherenko/jscpd/issues/727), [#1021](https://github.com/kucherenko/jscpd/pull/1021))
- **Clone kinds in every reporter** — console prints `Clone found (javascript, renamed)` and `Clone found (javascript, similar (gap) ~0.91)`, `ai` appends `(renamed)` / `[~0.91 gap]`, JSON adds `kind`, `similarity` and `method` to each duplicate and `renamedClones` / `similarClones` to the statistics, XML adds the same attributes, HTML shows a badge, Xcode a suffix, and SARIF and Code Climate use the rules `jscpd/renamed-code` and `jscpd/similar-code` next to `jscpd/duplicate-code`. ([#1019](https://github.com/kucherenko/jscpd/pull/1019), [#1021](https://github.com/kucherenko/jscpd/pull/1021), [#1030](https://github.com/kucherenko/jscpd/pull/1030))
- **Tips are skipped when stdout is not a terminal** — the tips and sponsor lines are printed only on an interactive terminal; a pipe, a file, a CI log or an agent hook no longer receives them. `JSCPD_NO_TIPS` joins `CI` as an environment switch and `--no-tips` stays the explicit one; `NO_COLOR` only removes the colours. ([#1008](https://github.com/kucherenko/jscpd/issues/1008), [#1029](https://github.com/kucherenko/jscpd/pull/1029), thanks [@7487](https://github.com/7487))
- **MCP: fully described tool definitions** — the four tools now carry a title, read-only annotations, parameter descriptions with examples and defaults, and descriptions that say when to use each tool and what it returns; the server instructions describe the workflow across them. Tool names and schemas are unchanged. ([#1028](https://github.com/kucherenko/jscpd/pull/1028))

### Bug Fixes

- **Config-file `ignorePattern` entries without `*` or `?` silently did nothing** — such entries were treated as relative paths and joined onto the config directory, so `"ignorePattern": ["Copyright 2026 Example Authors"]` matched nothing while the same string via `--ignore-pattern` worked. Config entries are now applied verbatim, and an invalid regex prints a `Warning:` line instead of being dropped silently. See [`fixtures/ignore-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/ignore-demo/README.md). ([#997](https://github.com/kucherenko/jscpd/pull/997))
- **JavaScript/TypeScript files with a recoverable parse error could not match clean files** — any parser diagnostic sent the file to the word-split fallback tokenizer, so a file containing, say, a redeclared function was tokenized differently from every well-formed file and never paired with one. Tokens now come from the lexer whenever the parser did not fail outright. Clone counts on codebases with such files change; that is the correction. ([#1023](https://github.com/kucherenko/jscpd/issues/1023), [#1024](https://github.com/kucherenko/jscpd/pull/1024))
- **Markdown inherited the C comment style** — a `/*` (a glob like `docs/**`) or `//` (any URL) in prose opened a comment that swallowed the rest of the file, so two files sharing a paragraph after such a line were never reported. Markdown now has no comment syntax. ([#1026](https://github.com/kucherenko/jscpd/pull/1026), thanks [@kwesolowski](https://github.com/kwesolowski))
- **Vue template clones were reported with wrong ranges** — the wrapper tags of the file and the template body were appended to the html token stream out of source order, so a clone across the seam took its endpoints from opposite ends of the file. The stream is now in source order, and the wrapper tags (`<template>`, `<script>`, `<style>` and their closing tags) are left out of it altogether, so a template clone is reported with the template's own line range and the script and style bodies are not counted as duplicated html. See [`fixtures/sfc-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/sfc-demo/README.md). ([#1031](https://github.com/kucherenko/jscpd/pull/1031), thanks [@zero-stroke](https://github.com/zero-stroke))

### Other

- **Runnable demos under `fixtures/`** — every feature and fix above ships a demo directory (`ignore-demo`, `type2-demo`, `type3-demo`, `parse-errors-demo`, `sfc-demo`) whose README lists each command with its expected output, and the same files feed the smoke scan that runs on every pull request.
- **Docs: ignore patterns and inline markers** — `--ignore-pattern` / `ignorePattern` source-region filtering and the `jscpd:ignore-start` / `jscpd:ignore-end` markers are documented in the v5 reference, with license-header recipes and a note on the Rust regex syntax. ([#993](https://github.com/kucherenko/jscpd/issues/993), [#996](https://github.com/kucherenko/jscpd/pull/996), thanks [@w3lld1](https://github.com/w3lld1))
- **GitHub Action inputs** `ignore-identifiers`, `ignore-literals`, `ignore-annotations`, `max-gap-lines` and `similarity` for the features above.

### Dependencies

- Add `regex` 1 to the `jscpd` crate for `--ignore-pattern` validation ([#997](https://github.com/kucherenko/jscpd/pull/997))
- Bump `taiki-e/install-action` from 2.87.2 to 2.87.3 in `/.github/workflows` ([#995](https://github.com/kucherenko/jscpd/pull/995))

### Thank You ❤️

- [@7487](https://github.com/7487) for skipping the tips on a non-terminal stdout ([#1029](https://github.com/kucherenko/jscpd/pull/1029))
- [@zero-stroke](https://github.com/zero-stroke) for the Vue template clone ranges ([#1031](https://github.com/kucherenko/jscpd/pull/1031))
- [@kwesolowski](https://github.com/kwesolowski) for the Markdown comment-style fix ([#1026](https://github.com/kucherenko/jscpd/pull/1026))
- [@w3lld1](https://github.com/w3lld1) for documenting ignore patterns and inline markers ([#996](https://github.com/kucherenko/jscpd/pull/996))

---

## 5.1.2

### New Features

- **Linux ARM64 musl prebuilt binaries** — npm installs on Alpine and other musl-based ARM64 Linux systems now select a native binary from the new `jscpd-linux-arm64-musl` platform package, bringing the prebuilt platform count to 8. The GitHub release ships the matching `jscpd-linux-arm64-musl.tar.gz` asset. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **`cargo binstall jscpd`** — the crate now carries `cargo-binstall` metadata pointing at the release tarballs for every supported target, so `cargo binstall jscpd` downloads a prebuilt binary instead of compiling the `oxc` parser stack from source. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Docker image `ghcr.io/kucherenko/jscpd`** — a multi-arch (amd64/arm64) distroless image built from the release binaries is published with every release, tagged `latest`, `5`, `5.1` and the exact version, with SLSA provenance and an SBOM attached. Run it as `docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd`; see [docs/ci-and-hooks.md](https://github.com/kucherenko/jscpd/blob/master/docs/ci-and-hooks.md). ([#988](https://github.com/kucherenko/jscpd/pull/988))

### Bug Fixes

- **`jscpd --version` and `jscpd --help` now say `jscpd`** — both binaries are built from the same source and the command name was the literal `cpd`, so `jscpd --version` printed `cpd 5.1.1` and the usage line read `Usage: cpd`. The name is now taken from the invoked executable (`jscpd` or `cpd`). ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Windows: drive-anchored `--pattern` values are treated as absolute** — the Windows-only check for patterns like `C:\src\**\*.ts` compared the first character against `:` and `\` after already requiring it to be a letter, so it could never match and such patterns were also given the relative `**/` variant. The check is now a platform-independent helper with a unit test that runs everywhere. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **pre-commit hook passed v4-only flags** — `.pre-commit-hooks.yaml` still invoked `--gitignore --exitCode '1'`, which the v5 CLI rejects, so `repo: https://github.com/kucherenko/jscpd` hooks failed on every run. The hook now passes `--exit-code 1`. ([#989](https://github.com/kucherenko/jscpd/pull/989))
- **Unsupported-platform error is actionable** — when no prebuilt binary matches, the `jscpd` and `cpd` npm launchers now name the host (`os/arch (libc)`), list the supported platform keys and point to `cargo install jscpd` instead of printing a bare "Unsupported platform". ([#988](https://github.com/kucherenko/jscpd/pull/988))

### Other

- **Repository split: `master` is v5-only** — the TypeScript v4 engine (`apps/`, `packages/`, changesets, Node.js CI) moved to the long-lived [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4) branch and releases from there under the `latest-4` npm dist-tag. `master` keeps the Rust workspace, the shared `fixtures/` corpus, the GitHub Action, Dockerfile and flake. `README-v4.md` describes the TypeScript version in one page; `FORMATS.md` is now generated from the Rust tokenizer (224 formats). ([#989](https://github.com/kucherenko/jscpd/pull/989), [#990](https://github.com/kucherenko/jscpd/pull/990))
- **Floating `v5` tag for the GitHub Action** — `uses: kucherenko/jscpd@v5` follows the latest 5.x release; the release workflow moves the tag on every stable release. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **crates.io metadata** — every crate now declares `repository`, `documentation`, `keywords` and `categories`; the `jscpd` crate excludes `tests/` from the published package, ships an expanded README rendered on docs.rs, and npm packages carry a `funding` field. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Signed release assets** — in addition to SLSA provenance, each release archive and `checksums.txt` now has a Sigstore keyless signature (`<asset>.sigstore.json`) verifiable with `cosign verify-blob`; the release notes include the exact commands. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **CI** — Windows joined the pull-request build matrix, a smoke test runs the release binary against the `fixtures/` corpus on every push, and a nightly job runs `cargo audit` and `cargo deny`. ([#988](https://github.com/kucherenko/jscpd/pull/988), [#989](https://github.com/kucherenko/jscpd/pull/989))

### Dependencies

- Bump `quick-xml` to 0.42.0 in `/rust` ([#991](https://github.com/kucherenko/jscpd/pull/991))

---

## 5.1.1

### Bug Fixes

- **`jscpd` on npm installed the 5.0.16 engine instead of 5.1.0** — the `jscpd` wrapper package published its `optionalDependencies` pinned to the `5.0.16` platform binaries, so `npm i jscpd@5.1.0` resolved a native binary one release behind and `jscpd --version` reported `cpd 5.0.16`. Everything 5.1.0 fixed was therefore absent for `jscpd` users, including the Windows `--baseline-from-ref` fix. The `cpd` package was pinned correctly and is unaffected, as are the platform packages themselves — only the wrapper's pins were stale.

  The cause was in `scripts/sync-version.mjs`: the wrapper's version and its platform pins were updated together behind a single `version !== npmVersion` guard, so once anything set `version` before the script ran, the guard read "already up to date" and left the pins untouched. The two are now updated independently, and the script ends by verifying that every npm version and platform pin matches the release version, exiting non-zero if any disagree — the release workflow runs this script, so a repeat of this mismatch now fails the release instead of publishing. This is the same defect that produced the 5.0.13 republish; the earlier fix covered the `cpd` package but not the `jscpd` wrapper.

### Other

- **Declared MSRV corrected to 1.96** — the workspace advertised `rust-version = "1.87"` on crates.io, a floor the crate could not build on: the `oxc` parser crates require 1.96.0, and `ignore`, `globset` and `askama` require 1.88. The value had been set when the Rust workspace was created and never revisited, and no CI job built at the declared MSRV, so the drift went unnoticed. CI now derives the toolchain from `rust-version` and checks against exactly that version.

---

## 5.1.0

### New Features

- **Windows on ARM support** — npm installs now select a native `aarch64-pc-windows-msvc` binary from the `jscpd-windows-arm64-msvc` platform package on Windows ARM64.

- **Clone baseline (`--baseline`, `--update-baseline`, `--fail-on-new-clones`)** — gate CI on *new* duplication only. A committed baseline file (e.g. `.jscpd-baseline.json`) records content-hash fingerprints of accepted clones (the same hash the SARIF reporter emits as `partialFingerprints["jscpdCloneHash/v1"]`, with a multiplicity count per fingerprint); clones absent from it are reported as new, and `--fail-on-new-clones[=N]` exits 1 when more than N (default 0) new clones are found — independently of `--threshold`, so legacy duplication is tolerated while regressions fail the build. `--update-baseline` rewrites the file from the current run (creating it if missing) and prints added/removed fingerprint counts so baseline growth stays visible in CI logs and PR review. The baseline file is versioned, sorted one fingerprint per line for reviewable diffs and trivial merges, and configurable via the `baseline` / `failOnNewClones` config keys. New-clone info flows through the reporters: `[NEW]` markers and a "(N new)" found-count in `console`/`console-full`, per-clone `isNew` plus the `newClones` / `newDuplicatedLines` statistics in `json`, level `error` in `sarif`, and `jscpd_new_clones` / `jscpd_new_duplicated_lines` gauges in `openmetrics`. ([#944](https://github.com/kucherenko/jscpd/issues/944))

- **Ephemeral baseline from a git ref (`--baseline-from-ref`)** — stateless variant of the clone baseline for PR gates without a committed file: `cpd --baseline-from-ref origin/main --fail-on-new-clones .` checks the base ref's tree out into a temporary detached git worktree (removed afterwards; shells out to `git` like blame does), scans it with the same detection configuration, and compares the current run against that in-memory fingerprint set — clones absent from the base ref are new. Costs a second scan of the corpus, where the committed `--baseline` file needs only one. When the ref is missing (shallow CI checkout) it fails with a clear hint to `git fetch origin main` or use `fetch-depth: 0`. Config key `baselineFromRef`; conflicts with `--baseline` / `--update-baseline`. ([#944](https://github.com/kucherenko/jscpd/issues/944))
- **OpenMetrics reporter (`--reporters openmetrics`)** — writes `jscpd-metrics.txt` in the [OpenMetrics](https://openmetrics.io/) text exposition format, ready to be declared as a GitLab CI `artifacts:reports:metrics` artifact so merge requests show duplication metric changes against the target branch. Exposes gauges for files/lines/tokens analyzed, clones found, duplicated lines/tokens with percentages (project total plus a `format`-labeled sample per format), and detection duration in seconds. ([#422](https://github.com/kucherenko/jscpd/issues/422))
- **CodeClimate / GitLab Code Quality reporter (`--reporters codeclimate`, alias `gitlab`)** — writes `gl-code-quality-report.json` (the filename GitLab's docs use) in the CodeClimate issue format, restricted to the subset GitLab defines as its [Code Quality report format](https://docs.gitlab.com/ci/testing/code_quality/#code-quality-report-format), ready to be declared as an `artifacts:reports:codequality` artifact so duplicates appear as code quality issues in merge requests — unlike the SARIF reporter, which GitLab ingests as security vulnerability findings. Each clone yields an issue per fragment (each describing the other location, plus the CodeClimate `other_locations` field), with a deterministic fingerprint derived from the clone's content hash so GitLab can tell new issues from pre-existing ones across pipeline runs. Severity is `minor`, escalating to `major` for clones absent from a configured baseline or when the run exceeds `--threshold`. ([#958](https://github.com/kucherenko/jscpd/issues/958))
- **Config discovery in `.config/` (dot-config convention)** — auto-discovery now also checks `.config/jscpd.json` (and `.config/.jscpd.json`) per the [dot-config convention](https://dot-config.github.io/), between the root `.jscpd.json` and the `package.json` `jscpd` key. A root `.jscpd.json` still wins, so existing setups are unaffected; paths inside the config resolve against the working directory, as with other auto-discovered sources. ([#979](https://github.com/kucherenko/jscpd/issues/979))

### Bug Fixes

- **Unknown `--format` values warn instead of silently matching nothing** — a typo like `--format cs` (instead of `csharp`) used to scan 0 files and exit 0, indistinguishable from a clean codebase in CI. The CLI now prints a stderr warning naming the unsupported value and pointing to `--list`; custom formats declared via `--formats-exts` stay accepted. ([#964](https://github.com/kucherenko/jscpd/issues/964))
- **Nix flake builds again** — the flake pinned the hash of the mutable `channel-rust-1.97.toml` manifest, which broke with a fixed-output hash mismatch when Rust 1.97.1 was published. The toolchain is now pinned to the exact patch version (immutable manifest), so the hash can no longer drift. ([#976](https://github.com/kucherenko/jscpd/issues/976))
- **Windows: `--baseline-from-ref` no longer reports every clone as new** — the format-suffix stripper treated the drive colon in Windows verbatim paths (`\\?\C:\...`, the form `canonicalize` returns) as a `:format` suffix and truncated the base scan's source ids to `\\?\C`, so every snippet read behind the fingerprint computation failed silently and the ephemeral baseline never matched. A colon followed by a path separator is now recognized as structural. Clone fingerprints are also line-ending agnostic now (CR stripped before hashing), so committed baselines survive CRLF/LF differences between platforms.

### Other

- **Glama MCP listing** — the repository now ships a `glama.json` maintainer manifest and a `Dockerfile` that runs the stdio MCP server (`jscpd --mcp`), used by [Glama](https://glama.ai/mcp/servers/kucherenko/jscpd) to build and score the server listing
- **Signed releases** — release artifacts are signed with SLSA provenance, and piped downloads in workflows are pinned (OpenSSF Scorecard)

### Dependencies

- Bump Rust toolchain to 1.97.1 and `oxc` crates to 0.147 in `/rust`
- Bump `thiserror` to 2.0.20, `globset` to 0.4.20, `ignore` to 0.4.33, `log` to 0.4.34 in `/rust`

### Thank You ❤️

- [@luchsamapparat](https://github.com/luchsamapparat) for contributing Windows on ARM support ([#963](https://github.com/kucherenko/jscpd/pull/963))
- [@dmromanov](https://github.com/dmromanov) for proposing the OpenMetrics reporter ([#422](https://github.com/kucherenko/jscpd/issues/422))
- [@beanaroo](https://github.com/beanaroo) for proposing the GitLab / CodeClimate Code Quality report format ([#958](https://github.com/kucherenko/jscpd/issues/958))
- [@MRDGH2821](https://github.com/MRDGH2821) for proposing config discovery from the `.config/` subfolder ([#979](https://github.com/kucherenko/jscpd/issues/979))
- [@zbcoding](https://github.com/zbcoding) for reporting the silent unknown-`--format` behavior ([#964](https://github.com/kucherenko/jscpd/issues/964))
- [@eaves-dropper](https://github.com/eaves-dropper) for reporting the Nix build failure ([#976](https://github.com/kucherenko/jscpd/issues/976))

---

## 5.0.16

### New Features

- **MCP server over stdio (`--mcp`)** — `cpd --mcp /path/to/project` serves the Model Context Protocol on stdin/stdout, the transport MCP clients spawn and manage themselves (no port, no network policy). The project is scanned once at startup and kept in memory as detection-ready token hashes, so `check_duplication` snippet checks answer in milliseconds. Tools: `check_duplication` (accepts format names or file extensions), `get_file_clones` (clones involving one file — new over the HTTP server), `get_statistics`, and `check_current_directory` (returns the clone list). All clone/match lists are sorted biggest-first and capped by an optional `limit` argument (default 100) with the untruncated total always reported. Implements protocol revision `2025-06-18` (accepting `2025-03-26` / `2024-11-05` clients); all standard detection options (`--min-tokens`, `--format`, `--cross-formats`, ...) apply to the scan and to snippet checks. ([#891](https://github.com/kucherenko/jscpd/issues/891))
- **Codebase summary (`--summary`)** — opt-in refactoring-hotspot overview appended to the run output: top files and folders ranked by tokens, lines, size, or a token-based cyclomatic-complexity estimate, with each file's duplication share. `--summary-top <n>` sets the list length, `--summary-by tokens|lines|size|complexity` picks the ranking metric (config file: `summary`, `summaryTop`, `summaryBy`). Renders in `console`/`console-full`, as a compact one-line-per-entry block in the `ai` reporter, and as an additive `summary` key in the JSON report (absent when the flag is off, so the schema is unchanged for existing consumers). Computed after detection from data already in memory — runs without `--summary` are unaffected. ([#934](https://github.com/kucherenko/jscpd/issues/934))
- **Isolated folder groups (`--skip-isolated`)** — skip duplication between monorepo folders owned by different teams: declare isolation groups as comma-separated lists of pipe-separated folders (`--skip-isolated "packages/team-a|packages/team-b,libs/a|libs/b"`), and clones whose two fragments fall under two *different* folders of the same group are dropped. Duplication inside a single folder, against shared code, or across unrelated groups is still reported. The config file accepts the nested-array shape `"skipIsolated": [["packages/a", "packages/b"]]` (kebab-case `skip-isolated` works too), and the option applies to MCP project scans as well. Ports [#628](https://github.com/kucherenko/jscpd/pull/628) to the Rust engine. ([#942](https://github.com/kucherenko/jscpd/pull/942))

### Security

- **Supply-chain hardening (OpenSSF Scorecard)** — every GitHub Action in the release and CI pipelines is pinned to a full commit SHA (kept fresh by Dependabot), workflow tokens follow least privilege (top-level `contents: read`, write grants scoped to the jobs that need them), and the repository now has a `SECURITY.md` with private disclosure channels, private vulnerability reporting, and a protected `master` branch

### Bug Fixes

- **GitHub "Latest" release badge stays on v5** — Rust v5 releases are created with `--latest`; legacy TypeScript v4 and `cpd v*` releases explicitly opt out, so a v4 maintenance release can no longer take the Latest badge from the v5 line

### Other

- **npm package page polish** — README links are absolute GitHub URLs so they resolve on npmjs.com, and the package description and keywords better describe what jscpd does

### Dependencies

- Bump Rust toolchain to 1.97 and `oxc` crates to 0.144 in `/rust`
- Bump `serde` to 1.0.229 in `/rust`
- Bump `clap` to 4.6.6 in `/rust`
- Bump `memchr` to 2.8.3 in `/rust`
- Bump `xxhash-rust` to 0.8.18 in `/rust`

### Thank You ❤️

- [@hanzhangyu](https://github.com/hanzhangyu) for proposing isolated folder groups for monorepos and contributing the original `skipIsolated` implementation ([#628](https://github.com/kucherenko/jscpd/pull/628)), which this release ports to the Rust engine

---

## 5.0.15

### New Features

- **SARIF: size-based severity** — new `--sarif-error-tokens <N>` flag (also `sarifErrorTokens` in `.jscpd.json`): clones with at least N tokens are reported at level `error` while smaller ones stay `warning`. When overall duplication exceeds `--threshold`, **all** SARIF results are emitted as `error`, matching the threshold check that fails the build. Default output is unchanged when neither option is set. ([#908](https://github.com/kucherenko/jscpd/issues/908))
- **SARIF: clone fingerprints** — each result carries `token_count` and a `clone_hash` in its properties bag, plus a `partialFingerprints` entry (`jscpdCloneHash/v1`) for cross-run result identity in consumers like GitHub code scanning. The hash is order-insensitive, so the same clone pair produces the same hash regardless of file discovery order. ([#909](https://github.com/kucherenko/jscpd/issues/909))
- **SARIF: related-location messages** — the duplicate's counterpart location now has a message (`Duplicated at <path>:<line>`), and the primary message references it via a SARIF embedded link so GitHub code scanning displays it. ([#911](https://github.com/kucherenko/jscpd/issues/911))
- **SARIF: richer rule metadata** — the `jscpd/duplicate-code` rule now includes a display name, full description, default configuration, and quality tags for better presentation in SARIF viewers and Azure DevOps. ([#914](https://github.com/kucherenko/jscpd/pull/914))

### Bug Fixes

- **Scan-root-relative report paths** — fragments store their scan root separately (`source_root`), so report paths are relative to the scanned directory again (as in 4.x) while reporters can still resolve and read source files; SARIF emits `originalUriBaseIds` with per-root base ids. Fixes empty snippets and unresolvable paths when scanning from outside the target directory, including multi-root scans. ([#872](https://github.com/kucherenko/jscpd/issues/872), [#892](https://github.com/kucherenko/jscpd/issues/892))
- **Report version stamping** — the SARIF `tool.driver.version` (previously hardcoded `5.0.3`) and the HTML report version now match `cpd --version`, bundled at build time. ([#915](https://github.com/kucherenko/jscpd/issues/915))
- **Multi-root blame attribution** — with multiple scan roots containing the same relative path, git blame data is now keyed by resolved path, so the second root no longer inherits the first root's authors.
- **Git root discovery** — walking up from a relative scan path no longer terminates early before reaching the repository root.

### Dependencies

- Bump `serde_json` to 1.0.151 in `/rust`
- Bump `ignore` to 0.4.32 in `/rust`
- Bump `anyhow` to 1.0.104 in `/rust`

### Thank You ❤️

This release was shaped by community contributions — huge thanks to:

- [@chrisc-onaorg](https://github.com/chrisc-onaorg) for the SARIF clone fingerprints ([#910](https://github.com/kucherenko/jscpd/pull/910)), related-location messages ([#912](https://github.com/kucherenko/jscpd/pull/912)), and richer rule metadata ([#914](https://github.com/kucherenko/jscpd/pull/914)), plus reporting #909, #911, and #915
- [@darronz](https://github.com/darronz) for the scan-root-relative paths fix ([#913](https://github.com/kucherenko/jscpd/pull/913))
- [@nvuillam](https://github.com/nvuillam) for proposing size-based SARIF severity ([#908](https://github.com/kucherenko/jscpd/issues/908))

---

## 5.0.14

### New Features

- `--cross-formats` — detect clones across related formats via format equivalence groups sharing one comparison pool, e.g. `--cross-formats "javascript,typescript"` or the `js-ts` preset (`javascript,jsx,typescript,tsx`). When a group mixes TypeScript with JavaScript, TS files are compared with erasable type syntax stripped (positions still reference the original source), so `function f(a: number): void` matches `function f(a)`. Also configurable as `crossFormats` in `.jscpd.json` / `package.json` (string, array-of-strings, or array-of-arrays). Cross-format clones are attributed to one member format in per-format statistics. ([#810](https://github.com/kucherenko/jscpd/issues/810))

### Bug Fixes

- **Prose-only Markdown files are now analyzed** — the Markdown tokenizer only extracted fenced code blocks, so `.md` files without code fences produced zero tokens and were silently skipped (`-f markdown` matched 0 files in Markdown-only projects). Prose is now tokenized too, so duplicated prose is detected as clones, while embedded code fences keep being detected under their own sub-format pools. ([#883](https://github.com/kucherenko/jscpd/issues/883))

### Dependencies

- Bump `regex` to 1.13.1 in `/rust`
- Bump `globset` to 0.4.19 in `/rust`
- Bump `ignore` to 0.4.28 in `/rust`
- Bump `xxhash-rust` to 0.8.16 in `/rust`

---

## 5.0.13

npm-only release: republished the `cpd` package so its `optionalDependencies` point at the 5.0.12 platform binaries (the `cpd@5.0.12` package still referenced the 5.0.11 binaries). No code changes. The version sync script now keeps these in sync automatically.

---

## 5.0.12

### Dependencies

- Bump `askama` to 0.16.0 in `/rust`
- Bump `log` to 0.4.33 in `/rust`
- Bump `env_logger` to 0.11.11 in `/rust`
- Bump `rustc-hash` to 2.1.3 in `/rust`

---

## 5.0.11

### New Features

- **Razor (.razor) support** — new tokenizer for Razor files, parsing HTML content and Razor keyword blocks (thanks to [@chrisc-onaorg](https://github.com/chrisc-onaorg) in [#829](https://github.com/kucherenko/jscpd/pull/829))

### Bug Fixes

- Fix `cargo test` on Windows by using `CARGO_BIN_EXE_cpd` or `.exe` suffix to locate the test binary
- Fix `cargo fmt` formatting issues across the workspace

### Refactoring

- Extract methods and optimize intervals in `cpd-core` (bumped to 0.1.6)

### Dependencies

- `cpd-core` updated to 0.1.6, `cpd-tokenizer` updated to 0.1.7 across all dependent crates

---

## 5.0.10

### Bug Fixes

- Emit scan-root-relative paths in all reporters when `absolute: false` (or the default). Previously, `jscpd /abs/path` from a different CWD left absolute paths in SARIF/JSON/XML/HTML/CSV/Markdown/console output, and Windows/macOS path canonicalization could leave `\\?\` or `./` prefixes. Paths are now normalized against the canonicalized scan root (with CWD fallback) and stripped of any leading `./` or `.\\` component. Fixes [#827](https://github.com/kucherenko/jscpd/issues/827)
- Fix `--skip-local` to match jscpd v4 TypeScript semantics: it now filters clones where both fragments are under the same scan root, instead of only skipping clones in the same parent directory

### Refactoring

- DRY duplication in reporters: extract shared helpers (`print_clone_header`, `print_clone_locations`, `print_snippet`, `write_report_file`, report statistics, test fixtures, etc.) into `cpd-reporter/src/shared.rs`. Console, console-full, CSV, JSON, HTML, Markdown, silent, XML, and SARIF reporters now reuse the same implementation, reducing the monorepo's reported duplication ratio from 5.0% to 0.56% and fixing a latent `--absolute` path relativization bug in the same pass
- Move blame enrichment from `gitoxide` to `git blame --porcelain`; capture elapsed time after blame so timing includes blame work
- Resolve `needless_borrow` clippy warnings in CSV and Markdown reporters

### Documentation

- Add Nix and Homebrew install instructions to Rust READMEs. [#818](https://github.com/kucherenko/jscpd/issues/818)
- Update project homepage URLs to `https://jscpd.dev` in all `Cargo.toml` and npm `package.json` files, add curl install method to READMEs, clean up outdated badges
- Remove defunct Universal Analytics tracking pixels from all READMEs

---

## 5.0.9

### New Features

- GitHub Action for jscpd (Rust v5) — `jscpd-copy-paste-detector` action for GitHub Actions Marketplace. Scan your repo for copy/paste in CI with `uses: kucherenko/jscpd/.github/workflows/action.yml@v5`

### Bug Fixes

- Resolve platform binary resolution when `cpd` is installed as a nested dependency (e.g. in a project's `node_modules` via a parent package). The runner now correctly locates the platform-specific binary relative to the installed package rather than assuming a top-level install. Fixes [#816](https://github.com/kucherenko/jscpd/issues/816)

---

## 5.0.8

### Bug Fixes

- Prevent mmap exhaustion crashes when scanning repositories with more files than `vm.max_map_count` (default 131 072 on Linux). The walker previously held a live `Mmap` per discovered file; each rayon worker now opens and drops its mapping within the processing closure, capping concurrent mappings to the thread-pool size (typically 8–32). Fixes [#813](https://github.com/kucherenko/jscpd/issues/813)
- Fix `--pattern` not matching relative paths when the scan root is absolute (e.g. CWD). Patterns like `src/**/*.ts` now match correctly by comparing against both the relative path and the full absolute path, and bare patterns like `*.ts` gain a `**/` prefix to match at any depth. Fixes [#811](https://github.com/kucherenko/jscpd/issues/811)
- Fix trailing-newline off-by-one in line-count filter: files not ending with `\n` now count the final line correctly

---

## 5.0.7

### Bug Fixes

- Prevent stack overflow when scanning directories containing deeply-nested JS/TS files (e.g. Bun's `test/bundler` with 320K+ nested for-loops). OXC's recursive-descent parser allocates one stack frame per AST nesting level; pathological inputs now exceed the default 8 MiB thread stack. Fixed by building a local rayon `ThreadPool` with 64 MiB stacks instead of using the global pool (which silently fails on re-init)
- Default `--max-size` to `1mb` — files exceeding the limit are skipped at walk time, consistent with jscpd v4's `maxSize` behavior. This prevents OXC from ever seeing megabyte-scale generated files that would overflow the stack
- `--workers N` now correctly takes effect on every `run()` call (previously `build_global()` silently no-op'd after the first invocation)

---

## 5.0.6

### New Features

- v4 config backward compatibility — `.jscpd.json` fields `path`, `pattern`, `ignore`, and `ignorePattern` are now read and applied, matching jscpd v4 behavior
- `ignore` and `ignorePattern` are now distinct: `ignore` matches file-level globs, `ignorePattern` matches code-level regex patterns (previously conflated)
- `.jscpd.json` path config support — reads scan directories from the `path` field, resolving relative paths against the config file's directory
- `jscpd` npm wrapper package — publishes the same Rust binary under the `jscpd` name on npm with v5.x versioning
- `--exit-code` now matches v4 behavior: accepts optional integer value (`--exit-code` exits 1, `--exit-code 2` exits 2); `--threshold` and `--exit-code` are now independent
- Performance improvements: memory-mapped file I/O (via `memmap2`) eliminates heap copies of file contents; SIMD-accelerated line counting (via `memchr`); parallel detection pipeline uses `flat_map` to avoid intermediate allocations; JS tokenizer no longer clones source strings before parsing (thanks to [@auterium](https://github.com/auterium), [#808](https://github.com/kucherenko/jscpd/pull/808))

### Bug Fixes

- Fixed `--exit-code` to match jscpd v4's `--exitCode` behavior (was boolean, now optional integer)
- Fixed unique temp dir generation in reporter tests (added PID to prevent race conditions under parallel test runners)

---

## 5.0.4

### New Features

- CLI alignment with jscpd v4: new `--absolute`, `--ignore-case`, `--formats-exts`, `--formats-names` flags; fixed `--threshold`, improved `--max-size`
- Detection and statistics aligned with jscpd for consistent output across Rust and TypeScript versions
- Side-by-side blame comparison in console-full reporter
- Clone list display in console reporter

### Bug Fixes

- HTML reporter now outputs `jscpd-report.html` at the `output_dir` root
- Resolved all clippy warnings across workspace
- Fixed unique temp dir generation in tests (use `as_nanos()` instead of `subsec_nanos()`)

---

## 5.0.3

### New Features

- Rust-based cpd CLI with full feature parity to TypeScript jscpd
- Cross-platform binary distribution via npm platform packages (linux-x64-gnu, linux-arm64-gnu, linux-x64-musl, darwin-arm64, darwin-x64, windows-x64-msvc)
- 13 reporters: json, console, xml, csv, html, markdown, sarif, ai, badge, xcode, threshold, silent, console-full
- Time reporter for execution timing
- CLI short-form aliases matching TypeScript jscpd conventions
- ReportContext data structure for extensible reporter signatures
- Trusted Publishing support for crates.io via OIDC

---

## 5.0.2

### Bug Fixes

- Fixed Vue SFC tokenization to dispatch each block to its own sub-format
- Fixed entire-file duplicates silently dropped by RabinKarp store flush logic
- Fixed ReDoS hang on Lisp/Elisp files
- Fixed crash on malformed package.json when reading config

---

## 5.0.1

### New Features

- Initial Rust workspace with cpd-core, cpd-tokenizer, cpd-finder, cpd-reporter, and jscpd crates
- Cross-format detection for Vue SFC, Svelte, Astro, and Markdown files
- Shebang detection for extensionless scripts

---

## 5.0.0

### Breaking Changes

- First stable Rust release — replaces the TypeScript-based CLI with a native binary
- Reporter trait signature changed to use ReportContext instead of Statistics directly
