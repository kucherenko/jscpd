# Changelog

All notable changes to **cpd (Rust)** are documented here. Releases follow [Semantic Versioning](https://semver.org).

---

## 5.3.1

### New Features

- **`--dashboard` lists the largest code files.** The Project section now ranks files by lines, with their tokens and size, next to the largest formats — the files worth splitting, beside the most complex ones further down. Only code is ranked: a lockfile, a changelog or a long HTML page is often the longest file in a repository and nothing anyone would refactor, so prose, data and markup files are left out, as they are from the complexity list. `--summary-top` sets the rows, and the `json`, `markdown` and `html` reporters carry the same list (`project.largestFiles` in `jscpd-dashboard.json`, an added key). See [`fixtures/dashboard-demo`](../fixtures/dashboard-demo/README.md). ([#1086](https://github.com/kucherenko/jscpd/pull/1086))

- **basta now detects which frameworks a project uses.** A framework runs files that nothing imports: a router's pages, a runtime's plugin directories, handlers it finds by convention. basta used to know three such cases, hard-coded: Nuxt, Nitro and WXT. It now reads them from a list of about fifty definitions built into the binary, [`frameworks.yaml`](crates/basta/frameworks.yaml). The list covers Next.js, Nuxt, Nitro, WXT, Plasmo, Remix, React Router, SvelteKit, Astro, SolidStart, TanStack Start, Qwik City, Gatsby, RedwoodJS, Angular, NestJS, AdonisJS, Sails, `@fastify/autoload`, Strapi, Medusa, Ember, Quasar, React Native, Expo, Cloudflare Workers, Vercel, Netlify, Serverless, Trigger.dev, Docusaurus, VitePress, Eleventy, Storybook, Histoire, Jest, Vitest, Mocha, AVA, Playwright, Cypress, Cucumber, Prisma, Knex, Sequelize, TypeORM, Hardhat and Create React App.
  - A framework counts as present when basta finds any one of three things: its config file, its package among the `package.json` dependencies, or its section in `package.json`. basta checks every directory that holds scanned files, so each package of a monorepo is treated as its own project.
  - Several frameworks can apply at once, and each one keeps its own files alive. A Next.js application with Storybook, Vitest and Prisma counts as four.
  - The files and directories a framework loads are matched relative to the directory where the framework was found. basta also reads the framework's config for the settings that move directories: `srcDir`, `entrypointsDir`, `appDirectory`, `sourceRoot` in `nest-cli.json`, and `imports: false`.
  - The console report lists what was detected, for example `Frameworks: next (apps/web), vitest`. Both `basta` and `jscpd --dead-code` print it.
  - New `basta` flags: `--list-frameworks` prints the list. `--framework <name>` treats a framework as present, which helps when the scan starts below its `package.json`. `--no-frameworks` turns detection off. `--frameworks-config <file>` loads your own definitions, in YAML or JSON, in the same shape as the built-in list. basta also picks up `basta.frameworks.{yaml,yml,json}` from the working directory. Your definitions can add a framework basta does not know, or replace a built-in one that has the same name. If the file fails to load, basta stops with an error, because running without it would report as dead the very files the definitions protect.
  - A definition can also list the names its framework looks up in your code, under `globals`. The built-in list has `getServerSideProps`, `generateMetadata` and the route segment config for Next; `loader`, `action` and `meta` for Remix and React Router; `load`, `actions`, `prerender` and the hooks for SvelteKit; `getStaticPaths` for Astro; the Gatsby Node, browser and SSR APIs; the lifecycle methods of Angular and Nest; `onRequest*` for Cloudflare Pages Functions; `handler` for Netlify and Serverless functions; and more. basta treats a declaration with such a name as used. It never reports it, including under `--include-entry-exports` and as an unused member, and everything the declaration calls stays reachable.
  - A plain name in `globals` applies to every file of the project. The `{ names, files }` form limits names to the files the framework reads them from, so `loader` counts for Remix under the app directory and is an ordinary name everywhere else. This also fixes a React Router route named from `routes.ts`. basta reaches that file through a string, so it is not an entry point, and its `loader` used to be reported as an unused export.
  - Nitro now follows `srcDir` and treats `tasks/` as loaded by the framework.
  - See [`fixtures/dead-code-demo`](../fixtures/dead-code-demo/README.md#frameworks). ([#1087](https://github.com/kucherenko/jscpd/pull/1087))

- **`.jscpd.json` can hold dead-code settings in a section of their own.** The `deadCode` key (also spelled `dead-code` or `basta`) used to be a boolean that turned the mode on. It can now also be an object with these keys: `enabled`, `categories`, `minConfidence`, `minLines`, `entry`, `ignore`, `includeTests`, `includeEntryExports`, `threshold`, and the framework settings `frameworks` (definitions written inline, in the shape of `frameworks.yaml`), `frameworksConfig`, `framework` and `noFrameworks`.
  - The object only supplies settings. It turns the mode on only when it says `"enabled": true`, so you can keep clone settings and dead-code settings in one file and choose the run on the command line. `"deadCode": true` still works as the switch.
  - A flag wins over the section. The section wins over the older top-level keys (`minConfidence`, `entry`, `deadCodeCategories`, …), which still work.
  - `minLines` and `threshold` exist only inside the section, because the top-level keys with those names are about clones. The top-level `minLines` is the smallest clone to report and is never applied to a dead-code run. The top-level `threshold` is a limit for duplicated lines. A dead-code run still falls back to it when the section sets no `threshold`, as before.
  - A misspelled key inside the section is reported by name.
  - `jscpd --dead-code` and `--dashboard` now also pick up `basta.frameworks.{yaml,yml,json}` from the working directory.
  - The standalone `basta` binary reads the same section from the same file. It has a new `-c, --config <file>` flag. Without it, basta looks in the working directory for `.jscpd.json`, `.config/jscpd.json` or `package.json`. Both tools give the same result when you start them in the same directory.
  - See [`fixtures/dead-code-demo`](../fixtures/dead-code-demo/README.md#in-the-config-file). ([#1087](https://github.com/kucherenko/jscpd/pull/1087))

### Fixes

We ran basta next to knip and fallow on 55 repositories: the GitHub trending lists for JavaScript, TypeScript, Vue, Svelte and Astro, plus the source code of Nuxt, Next.js, Svelte, Gatsby and Astro. The run found these problems in basta, all fixed in [#1087](https://github.com/kucherenko/jscpd/pull/1087:

- **`--dead-code` was very slow on projects with many path aliases.** basta tested every import against every alias in the project, and it did the slow check first: comparing the importer's path with the directory of the config that declared the alias, one path segment at a time. A monorepo with 150 packages, each listing a few hundred `paths`, took 198 s (ever-gauzy). Next.js took 83 s. basta now checks first whether the alias pattern matches the import at all, which fails on the first character for almost every alias. The same two projects now take 3.3 s and 4.4 s, and the findings are byte for byte the same.
- **A catch-all alias made every package import slow.** With `"*": ["./*", "../../node_modules/*"]`, an import of `react` went through the resolver from every file, and each miss built and hashed about twenty candidate paths. The module index can now tell in two lookups whether a path could name any module, under any extension or as a directory index, and the answer is exact. LibreChat went from 4.0 s to 0.86 s with identical findings.
- **Vite's `'@': '/src'` alias was read as the root of the file system.** In a Vite alias target a leading `/` means the project root, as it does in a URL, and many projects write the alias that way. basta lost every `@/` import in such a project and reported its components as unused files at 95% confidence. MoeKoeMusic went from 55 unused files to 0.
- **``require(`./x`)`` with backticks was ignored.** A template literal with nothing interpolated is as static as a quoted string, and Gatsby writes every string that way. Gatsby went from 584 unused files to 352.
- **An arrow function in a Svelte or Astro attribute broke parsing.** In `<script on:load={() => { … }} src=…>`, basta took the `>` of `=>` for the end of the tag and passed the rest of the attribute to the JavaScript parser as the component's script. The file failed to parse, and everything it imports was reported as unused. cobalt went from 37 unused files to 8.
- **Django migrations and management commands were reported as unused files.** So were `admin.py`, `apps.py`, template tags, and the modules that settings name by dotted path. Django, Alembic and Scrapy are now in the framework list, detected by `manage.py`, `alembic.ini` and `scrapy.cfg`. AdventureLog went from 144 unused Python files to 6.
- **A file named only by a path string without an extension is no longer reported at full confidence.** `resolve(distDir, 'runtime/handlers/island')` is how a framework registers a file it loads itself. It is not an import, and basta does not treat it as one. But a file whose own path ends the same way now gets a new reason, `path-appears-in-string` ("its path appears in a string literal"), which costs 40 points. That puts the file under the default threshold of 60, and it still shows up with `--min-confidence 0`. Nuxt went from 83 unused files to 46. The reason is a new possible value of `reasons` in the JSON report.
- `--dead-code` read a [WXT](https://wxt.dev) browser extension as almost entirely dead ([#1082](https://github.com/kucherenko/jscpd/issues/1082)): the framework's `entrypoints/` directory — background, content scripts, popup pages — was not recognized as entry points, and its `@`/`~` → `srcDir`, `@@`/`~~` → root aliases live only in the generated `.wxt/tsconfig.json`, which no repository commits, so the whole tree dangled and cascaded (Tencent/BrowserSkill: 27.9% "unused", 115 unused files — now 0.4% and 5). A `wxt.config.*` now marks the entrypoints and auto-import directories as entries, honoring `srcDir`, `entrypointsDir` and `imports: false`, and declares the conventional aliases, the same way `nuxt.config.*` roots Nuxt's directories and `svelte.config.*` supplies `$lib`. ([#1083](https://github.com/kucherenko/jscpd/pull/1083))
- Markup, stylesheet and template files (HTML, XML, SVG, CSS, Handlebars, …) were assigned a complexity, counting words like `if`, `for` or a media query's `and` as branches — an HTML page could top the "Most complex files" list. These formats now have complexity `0`, like prose and data files already did, and are therefore no longer counted as code by the health score: an all-markup project reports "no code files" instead of being scored on its markup. Component formats (Vue, Svelte, Astro) still count in full through their script blocks. The markup block of a component file (tokenized as `html`) is now also excluded from the duplication share, matching the `css`/`scss` blocks that already were. ([#1081](https://github.com/kucherenko/jscpd/pull/1081)) ([#1084](https://github.com/kucherenko/jscpd/pull/1084))

### Other

- The dead-code engine in this release is basta 0.2.0, which is also published on its own: [`basta` on npm](https://www.npmjs.com/package/basta), [crates.io](https://crates.io/crates/basta) and [GitHub](https://github.com/kucherenko/basta/releases/tag/v0.2.0).

---

## 5.3.0

### New Features

- **`--dashboard` shows the whole project on one screen**: size and largest formats, duplication (clone count per kind, breakdown by format), complexity (most complex files), and dead code (JavaScript/TypeScript/Python, largest findings), all under a health badge. It runs the clone and dead-code scans side by side, so it costs about as long as the slower one, not their sum; `--workers` budgets the pair. `--summary-top` sets rows per list; detection options (`--min-tokens`, `--kind`, `--ignore`, …) and dead-code options (`--entry`, `--dead-code-categories`, `--min-confidence`) apply to their own sections. Reporters: `console`, `json` (`jscpd-dashboard.json`), `badge` (`jscpd-health-badge.svg`), `markdown`/`html` (`jscpd-dashboard.md`/`.html`). Clone-run exit gates apply (`--threshold`, `--exit-code`, `--fail-on-empty`); the baseline family needs a clone report this mode doesn't write, so `--baseline`/`--baseline-from-ref`/`--update-baseline` warn and are ignored, and `--fail-on-new-clones` is refused. See [`fixtures/dashboard-demo`](../fixtures/dashboard-demo/README.md).

- **`--health` scores a codebase from 0 to 100.** Duplication, dead code and complexity — each a share of code lines — are mixed with a "typical project" prior, scored on a half-life curve, and combined into one weighted geometric mean and a letter grade. Duplication skips markup, stylesheet and template blocks (HTML, CSS, Handlebars, …), since a repeating style rule isn't the same maintenance problem as repeating logic; component/script languages (Vue, Svelte, Astro, GraphQL) still count in full. The console line names what it measured and, when relevant, what it left out (`5.4% in typescript (no text)`). A dimension that can't be measured — no JavaScript/TypeScript/Python for dead code, no complex files — is left out and named (`dead code n/a`) rather than scored as clean. `--health-input FILE` (config key `healthInput`) folds in metrics from other tools; the same object can live under a `health` key in `.jscpd.json` to tune the built-in dimensions' half-life and weight. Reporters: `console` (badge), `ai` (one line), `json`, `badge`, `markdown`/`html`. See [`fixtures/dashboard-demo`](../fixtures/dashboard-demo/README.md#health).

- **`--complexity` runs the complexity half of `--summary` without the clone scan**: same walk, same tokenizing, same filters, but detection — the part that scales with codebase size — never starts. Reporters: `console`, `ai`, `json` (`jscpd-complexity.json`). Can't combine with `--dead-code`, `--dashboard` or `--mcp`, and warns instead of silently ignoring options it has nothing to apply to (`--threshold`, `--exit-code`, the baseline family, `--history`, `--kind`).

- **`--kind` filters clones by how they were found**: `exact`, `renamed`, `similar`, or one of the two mechanisms behind `similar` — `gap` (`--max-gap-lines`) and `ast` (`--similarity`). Statistics, `--threshold` and every reporter see the filtered list. It never turns a detector on by itself: `--kind ast` without `--similarity` warns that no such clones can be found, and an unknown kind is an error rather than a typo that silently reports a clean scan.

- **`--summary --summary-by complexity` now reads like real cyclomatic complexity** — the old estimate was one path per *file* plus every branch keyword it could see, matching neither the name nor any language consistently. Detection, tokenization and clone output are untouched; everything below is scoped to the summary.

  Short-circuit operators now count: only the JavaScript tokenizer emitted `&&` as one token, so for the other 200-odd formats `&&`/`||` were unreachable branch entries — the most common branch operator in C, C++, C#, Java, Go, Rust, PHP, Swift and Kotlin contributed nothing. Adjacent punctuation is now joined before matching, which also stops `??` being counted twice.

  A branch is counted the way its language spells it — Rust's `match` arms by their `=>` (one branch fewer than arm count), Swift's `guard` and Go's `select` added, `?` counted as a ternary only where it actually opens one (told apart from `String?`/`x?.y` by the preceding token), and `?:` read as Elvis in Kotlin/Groovy but as an optional property in TypeScript. One global keyword list couldn't have handled all of that at once.

  Complexity is now one path per function, not per file: where a language has a reliable marker (`def`, `fn`, `func`, `function`, `fun`, `=>` for JS arrow functions) the baseline is the function count; elsewhere it stays at one, as before. A docstring no longer adds branches either — the tokenizer now closes a `"""` string against the literal it actually arrives in (so a three-line docstring doesn't get read as three branch-bearing words), for the nine languages where `"""` is a real delimiter (Python, Kotlin, Scala, Groovy, Swift, Java, Julia, Elixir, Dart) and not where it's an escape (C# verbatim strings, VB.NET, SQL, Pascal).

  A word a file binds as a name is that file's name, not a branch keyword: `case`, `when` and `cond` are keywords in some languages and ordinary identifiers in others, and the tokenizer can't tell — but what a file assigns to, reads off an object, or lists as a parameter is now excluded, so `case = 3` or `f(case, when)` no longer counts as a branch.

  C, C++, Java, Objective-C and C# declare a function with no keyword at all (`head(args) {`), told apart from an `if`/`switch`/`for`'s closing `) {` by tracking what preceded the open paren — so those languages now count functions like the rest instead of falling back to one path per file.

  Measured against [lizard](https://github.com/terryyin/lizard) on ten GitHub-trending projects across nine languages, agreement on file ranking rose from 0.83 to 0.92 (median Spearman) and the median ratio to lizard's own figure went from 0.75 to 1.00, every project improving. C++ moved from 0.71 to 0.94, C# from 0.82 to 0.99. Remaining differences are intentional: lizard counts a `match` once regardless of arm count, and doesn't count `??`. [`fixtures/summary-demo`](../fixtures/summary-demo/README.md) has one file per language whose complexity can be counted by hand.

- **`--dead-code`: find code nothing runs** — a new engine (**basta**) that ships inside the jscpd binary and also stands alone as a `basta` command. It builds the import graph from a project's entry points and walks it, reporting unused files, exports, module-private declarations, imports and (opt-in) unused class members, across JavaScript, TypeScript, JSX, TSX, Vue, Svelte, Astro and Python.

  Entry points come from `package.json` (`main`, `module`, `bin`, `exports`, `scripts`), `pyproject.toml` (`[project.scripts]` and entry-point tables) and conventions (`src/index.ts`, `__main__.py`, framework routes, `*.config.ts`, `.d.ts`, shebangs, `if __name__ == "__main__"`, every `__init__.py`); a manifest naming a built file is mapped back to its source. `--entry <glob>` adds more.

  It's a graph traversal, not a reference count, so dead code cascades: a helper whose only caller is dead is reported too. Every finding carries a confidence score (0-100) and, below 100, a reason it might be wrong — an `eval`/`getattr` call, an unrecognised decorator, a wildcard re-export, a name that only appears in a string literal, a file that failed to parse. `--min-confidence` sets the floor (default 60).

  CommonJS is read alongside ESM (`require`, destructured `require`, `module.exports`, `exports.a`, dynamic `import()`), so a Node project that never touched `import` isn't read as a pile of dead files. Import aliases resolve through `tsconfig.json`/`jsconfig.json` `paths`/`baseUrl`, including monorepo `extends` chains, so a Next.js `@/components/x` alias reaches its file instead of looking missing. A path named only as a string — `new URL('./w.ts', import.meta.url)`, a build entry, a `vitest.config.ts` setup file — counts as used. Python honors quoted type annotations under `TYPE_CHECKING` and resolves `src/`-layout imports; files that fail to parse are listed under `statistics.unparsedFiles`.

  Single-file components (`.vue`/`.svelte`/`.astro`) are read whole, script and markup together — the script is masked in place rather than extracted, so reported positions stay real, and the markup is scanned for kebab-case component names, `{{ … }}` interpolations, actions/transitions and attribute expressions. Skipping this would report every component import as dead and every module a component alone imports as unused. Framework directory conventions (Nuxt's `components/`, `composables/`, etc., in both v3 and v4 layouts) are read from the project's own config rather than guessed from names, so a real import isn't silenced.

  Import paths are read the way the project's own build reads them: `vite.config.*`/`svelte.config.*` aliases join `tsconfig.json` paths, SvelteKit's `$lib` convention is read directly since its generated tsconfig isn't committed, computed specifiers (`` import(`./services/${type}.vue`) ``) resolve as globs, `import.meta.glob()` patterns are followed, and markup-expression `import()` counts as an edge while the same text in a comment doesn't. Nitro's `api/`/`routes/`/`middleware/` are read as file-system routes like Nuxt's.

  Monorepo package names (`@acme/ui`, `@acme/ui/date`) now resolve through the owning `package.json`'s `exports`/`main`/`module`, found by walking up to the workspace root (`pnpm-workspace.yaml`, `lerna.json`, `turbo.json`, or a root `workspaces` manifest) rather than stopping at the package directory.

  Measured on thirty Vue, Svelte and Astro trending projects against the previous release, reported unused files fell from 1093 to 481, and the share with a real importer in the tree dropped from 83% to 62%. Components imported only from `.mdx` are still reported — basta doesn't read MDX, so use `--entry` to mark them used.

  It ships on npm and crates.io as [`basta`](https://www.npmjs.com/package/basta) ([crates.io](https://crates.io/crates/basta)), version 0.1.0, with its own `basta-<platform>` packages and its own release trigger (a `basta-v*` tag, not the push to master that releases jscpd), so neither release drags on the other. Inside the jscpd binary, the same engine answers to `jscpd --dead-code`. Languages plug in through one trait (`basta::lang::Analyzer`); [`docs/basta-extending.md`](../docs/basta-extending.md) walks through adding one.

  The mode reuses what a jscpd user already knows — the same walker and filters (`--ignore`, `--format`, `.gitignore`, `--max-size`, `--follow-symlinks`), the same fifteen reporter names, the same `--threshold`/`--exit-code` gates. `--dead-code-categories` narrows what's reported; `--include-tests` and `--include-entry-exports` widen it. See [`fixtures/dead-code-demo`](../fixtures/dead-code-demo/README.md) and [the docs](../docs/rust.md#dead-code-detection---dead-code).

### Fixes

- A single wrong-typed field in `.jscpd.json` (`"entry": "src/index.js"` where an array was expected) discarded the whole config instead of just that field. Each top-level key is now checked in isolation, so only the ones that actually fail to parse are stripped and reported.
- A hostile file path, custom format name or external health-metric id could break a Markdown table, inject raw HTML into a rendered dashboard, or forge extra console table rows, since `--dashboard`, `--health` and their `markdown`/`html` reporters didn't sanitize untrusted strings before interpolating them. Every such value is now escaped for its destination.
- `--dashboard`/`--health` refused the whole report when `--format` excluded every language the dead-code engine analyzes, instead of dropping just that section like a project with none of those files already does; a bad `--dead-code-categories` or `--min-confidence` could also skip validation entirely when combined with such a format. Both are fixed: an unsupported format drops the dead-code section, and its options are always validated first.
- `--complexity --fail-on-empty` on an empty scan skipped writing reports (e.g. `-r json`) instead of writing them and then failing, unlike every other mode's `--fail-on-empty`.
- `--min-confidence` above 100 was accepted by `--dashboard`/`--health` even though standalone `--dead-code` already clamped it with a warning; the clamp now applies everywhere the option is read.
- A health-score regression: markup-format duplication (HTML, CSS, templates, …) was weighted down but still counted in full toward the denominator, so excluding it barely moved the score. It's now a hard exclusion on both sides, and a project whose code is entirely markup skips the duplication dimension instead of scoring it from the size prior alone.
- Windows report paths used backslashes (`src\route.js`) where every other output already normalized to forward slashes.
- `basta` bumped its `oxc_*` parser crates to 0.150 (from 0.147).

---

## 5.2.1

### New Features

- **`--history`: duplication trend over git history** — `jscpd src --history v5.0.0..HEAD` scans every commit in the range in a detached worktree and prints a bar chart, a per-commit table, the overall trend, and how far `--threshold` could be tightened without failing the build. `--history-since`, `--history-every N` and `--history-limit N` narrow the range; the JSON reporter carries the points under `history` and the GitHub Action takes a `history` input. ([#1002](https://github.com/kucherenko/jscpd/issues/1002), [#1050](https://github.com/kucherenko/jscpd/pull/1050), [#1052](https://github.com/kucherenko/jscpd/pull/1052))
- **Exit codes you can gate on, and `--fail-on-empty`** — an unknown `--format`, a missing scan path, or a reporter that can't write now print an error and exit 1 instead of passing with an empty report. `--fail-on-empty` (config `failOnEmpty`, action input `fail-on-empty`) turns "analyzed no files" into a failure, so a mistyped path or an over-broad ignore can't look like a clean run. ([#1047](https://github.com/kucherenko/jscpd/issues/1047), [#1049](https://github.com/kucherenko/jscpd/pull/1049))
- **PyPI: `pip install jscpd`** — eight platform wheels built from the same prebuilt binaries as npm and the GitHub release, so `pip install jscpd`/`uvx jscpd` get the Rust engine with no Python code and no Node.js involved. The repository pre-commit hook now installs from PyPI instead of npm. ([#1037](https://github.com/kucherenko/jscpd/issues/1037), [#1039](https://github.com/kucherenko/jscpd/pull/1039))

### Bug Fixes

- **An open clone could be stretched past the file it started in** — growing a clone accepted a continuation from *any* stored occurrence of the next window, so a third file sharing the same text but continuing differently could extend a fragment beyond its own file (`fixtures/haxe` reported mismatched ends between two files). The match now checks whether the clone's own anchor continues before extending, so N-way copies no longer lose pairs. ([#1033](https://github.com/kucherenko/jscpd/issues/1033), [#1034](https://github.com/kucherenko/jscpd/pull/1034))
- **The XML report could be rejected by every parser** — a byte XML 1.0 can't represent was written verbatim (`xmllint` refused with `PCDATA invalid Char value 27`), `]]>` inside a fragment closed the CDATA section early, and attributes were escaped twice. Such characters are now replaced with U+FFFD, `]]>` is split across two CDATA sections, and paths are escaped once. ([#375](https://github.com/kucherenko/jscpd/issues/375), [#1055](https://github.com/kucherenko/jscpd/pull/1055))
- **`--follow-symlinks` renamed and double-counted linked files** — a symlinked file was reported by its resolved real path (sometimes outside the scan root), so the report and `--ignore` disagreed about its name; a file reachable two ways counted twice, and a symlink beside its target reported as a clone of itself. Files now keep the path they were found at, and each real file is scanned once. ([#1059](https://github.com/kucherenko/jscpd/issues/1059), [#1060](https://github.com/kucherenko/jscpd/pull/1060))

### Other

- **Symlinks are skipped by default in v5** — v4 followed them unless `--noSymlinks` was set; v5 needs `--follow-symlinks` (config `followSymlinks`). True in every 5.x release but undocumented until now, and it silently drops a corpus mounted through a symlink. ([#1059](https://github.com/kucherenko/jscpd/issues/1059))
- **`CITATION.cff` and a Citation section** — GitHub's "Cite this repository" button and a BibTeX entry, version and date kept in step by `sync-version.mjs`. ([#1051](https://github.com/kucherenko/jscpd/pull/1051))
- **Docs: jscpd is language-aware** — detection runs on language tokens with per-format comment/string syntax and the oxc parser for JS/TS, not on raw text. ([#1048](https://github.com/kucherenko/jscpd/pull/1048))
- **Agent skills know about clone kinds, the summary and their noise** — the bundled `jscpd` and `dry-refactoring` skills document `--summary` and the Type-2/Type-3 flags, and warn that normalized passes surface look-alike code, with conservative defaults and a triage step. ([#1056](https://github.com/kucherenko/jscpd/pull/1056), [#1057](https://github.com/kucherenko/jscpd/pull/1057))
- **`console-full` prints the `--history` block** like `console` does; test scaffolding behind the CLI, MCP, reporter and finder suites was deduplicated. ([#1053](https://github.com/kucherenko/jscpd/pull/1053))
- **CI: the npm platform-package gate polls a 5-minute deadline** instead of a fixed sleep, so a slow registry no longer fails a release that would have succeeded. ([#1032](https://github.com/kucherenko/jscpd/pull/1032))

### Dependencies

- Bump `askama` from 0.16.0 to 0.16.1 in `/rust` ([#1045](https://github.com/kucherenko/jscpd/pull/1045))
- Bump `taiki-e/install-action` from 2.87.3 to 2.87.8 in `/.github/workflows` ([#1046](https://github.com/kucherenko/jscpd/pull/1046))

### Thank You ❤️

- [@mnahkies](https://github.com/mnahkies) for correcting the ignore examples in the README — `--ignore-pattern` has no short flag, and a bare `node_modules` doesn't match since globs match against the whole path ([#1038](https://github.com/kucherenko/jscpd/pull/1038))

---

## 5.2.0

### New Features

- **Type-2 clone detection: `--ignore-identifiers`, `--ignore-literals`, `--ignore-annotations`** — three opt-in flags that normalize token classes before hashing, so blocks differing only in names, literal values or annotations are found. Identifiers hash as one class while keywords keep their value, strings/numbers stay distinct, and `@Name(...)` runs are dropped in Java, Kotlin, Scala, Groovy, Python, Dart, Swift, JS and TS (`@interface` kept). Every clone now carries a `kind`: `exact` or `renamed`. A run without the flags is unchanged apart from the additive `"kind": "exact"` field. See [`fixtures/type2-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/type2-demo/README.md). ([#998](https://github.com/kucherenko/jscpd/issues/998), [#1019](https://github.com/kucherenko/jscpd/pull/1019))
- **Near-miss clone merging with `--max-gap-lines N`** — a copy with a line inserted, removed or changed in the middle used to show up as two shorter clones; now, clones of one file pair whose fragments follow each other with at most N unmatched lines between them merge into one clone of kind `similar` with a `similarity` value. A merge below `0.5` similarity is refused, duplicated-line stats count only matched lines, and a merge of renamed halves reports as `similar`. See [`fixtures/type3-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/type3-demo/README.md). ([#999](https://github.com/kucherenko/jscpd/issues/999), [#1020](https://github.com/kucherenko/jscpd/pull/1020), [#1030](https://github.com/kucherenko/jscpd/pull/1030))
- **Function-level similarity for JavaScript and TypeScript with `--similarity RATIO`** — compares every function/method/arrow function by the bag of 4-grams over its syntax-tree node types (MinHash-indexed), reporting pairs at or above the ratio as `similar` clones spanning whole functions. Names and literals don't count: a renamed copy scores `1.0`, one inserted line about `0.9`, two inserted statements plus renames about `0.75`. Each `similar` clone records its `method` (`gap` or `ast`) since the two scores aren't on the same scale. The MCP `check_duplication` tool accepts the same argument. ([#999](https://github.com/kucherenko/jscpd/issues/999), [#727](https://github.com/kucherenko/jscpd/issues/727), [#1021](https://github.com/kucherenko/jscpd/pull/1021))
- **Clone kinds in every reporter** — console prints `Clone found (javascript, renamed)`/`similar (gap) ~0.91`, `ai` appends `(renamed)`/`[~0.91 gap]`, JSON adds `kind`/`similarity`/`method` plus `renamedClones`/`similarClones` stats, XML/HTML/Xcode show the same, and SARIF/Code Climate add rules `jscpd/renamed-code` and `jscpd/similar-code`. ([#1019](https://github.com/kucherenko/jscpd/pull/1019), [#1021](https://github.com/kucherenko/jscpd/pull/1021), [#1030](https://github.com/kucherenko/jscpd/pull/1030))
- **Tips are skipped when stdout is not a terminal** — a pipe, file, CI log or agent hook no longer gets the tips/sponsor lines. `JSCPD_NO_TIPS` joins `CI` as an environment switch; `--no-tips` stays the explicit one. ([#1008](https://github.com/kucherenko/jscpd/issues/1008), [#1029](https://github.com/kucherenko/jscpd/pull/1029), thanks [@7487](https://github.com/7487))
- **MCP: fully described tool definitions** — the four tools now carry a title, read-only annotations, parameter examples/defaults, and descriptions of when to use each and what it returns; names and schemas are unchanged. ([#1028](https://github.com/kucherenko/jscpd/pull/1028))

### Bug Fixes

- **Config-file `ignorePattern` entries without `*` or `?` silently did nothing** — treated as relative paths and joined onto the config directory, so a plain string matched nothing while the same value via `--ignore-pattern` worked. Entries are now applied verbatim, and an invalid regex warns instead of being dropped silently. See [`fixtures/ignore-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/ignore-demo/README.md). ([#997](https://github.com/kucherenko/jscpd/pull/997))
- **JavaScript/TypeScript files with a recoverable parse error could not match clean files** — any parser diagnostic sent the file to the word-split fallback tokenizer, so it never paired with a well-formed file. Tokens now come from the lexer whenever the parser didn't fail outright; clone counts on such codebases change as a result. ([#1023](https://github.com/kucherenko/jscpd/issues/1023), [#1024](https://github.com/kucherenko/jscpd/pull/1024))
- **Markdown inherited the C comment style** — a `/*` (a glob like `docs/**`) or `//` (a URL) in prose opened a comment that swallowed the rest of the file. Markdown now has no comment syntax. ([#1026](https://github.com/kucherenko/jscpd/pull/1026), thanks [@kwesolowski](https://github.com/kwesolowski))
- **Vue template clones were reported with wrong ranges** — wrapper tags and template body were appended to the html token stream out of source order. The stream is now in source order and wrapper tags are excluded from it, so a template clone gets the template's own range. See [`fixtures/sfc-demo`](https://github.com/kucherenko/jscpd/blob/master/fixtures/sfc-demo/README.md). ([#1031](https://github.com/kucherenko/jscpd/pull/1031), thanks [@zero-stroke](https://github.com/zero-stroke))

### Other

- **Runnable demos under `fixtures/`** — every feature and fix above ships a demo directory whose README lists each command and its expected output; the same files feed the pull-request smoke scan.
- **Docs: ignore patterns and inline markers** — `--ignore-pattern`/`ignorePattern` and the `jscpd:ignore-start`/`-end` markers, with license-header recipes and a Rust regex note. ([#993](https://github.com/kucherenko/jscpd/issues/993), [#996](https://github.com/kucherenko/jscpd/pull/996), thanks [@w3lld1](https://github.com/w3lld1))
- **GitHub Action inputs** `ignore-identifiers`, `ignore-literals`, `ignore-annotations`, `max-gap-lines`, `similarity` for the features above.

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

- **Linux ARM64 musl prebuilt binaries** — npm installs on Alpine and other musl-based ARM64 Linux systems now get a native binary from `jscpd-linux-arm64-musl`, bringing the prebuilt platform count to 8. The GitHub release ships the matching `.tar.gz`. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **`cargo binstall jscpd`** — the crate now carries `cargo-binstall` metadata for every supported target, so `cargo binstall jscpd` downloads a prebuilt binary instead of compiling the `oxc` parser stack from source. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Docker image `ghcr.io/kucherenko/jscpd`** — a multi-arch (amd64/arm64) distroless image published with every release, tagged `latest`, `5`, `5.1` and the exact version, with SLSA provenance and an SBOM. Run as `docker run --rm -v "$PWD:/src" ghcr.io/kucherenko/jscpd`; see [docs/ci-and-hooks.md](https://github.com/kucherenko/jscpd/blob/master/docs/ci-and-hooks.md). ([#988](https://github.com/kucherenko/jscpd/pull/988))

### Bug Fixes

- **`jscpd --version` and `jscpd --help` now say `jscpd`** — both binaries share source and the command name was hardcoded to `cpd`, so `jscpd --version` printed `cpd 5.1.1`. The name now follows the invoked executable. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Windows: drive-anchored `--pattern` values are treated as absolute** — the Windows check for patterns like `C:\src\**\*.ts` compared the wrong character and could never match. Now a platform-independent helper with a unit test that runs everywhere. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **pre-commit hook passed v4-only flags** — `.pre-commit-hooks.yaml` still invoked `--gitignore --exitCode '1'`, which v5 rejects. The hook now passes `--exit-code 1`. ([#989](https://github.com/kucherenko/jscpd/pull/989))
- **Unsupported-platform error is actionable** — the npm launchers now name the host (`os/arch (libc)`), list supported platforms, and point to `cargo install jscpd` instead of a bare "Unsupported platform". ([#988](https://github.com/kucherenko/jscpd/pull/988))

### Other

- **Repository split: `master` is v5-only** — the TypeScript v4 engine moved to [`master-v4`](https://github.com/kucherenko/jscpd/tree/master-v4), releasing under the `latest-4` npm dist-tag. `master` keeps the Rust workspace, `fixtures/`, the Action, Dockerfile and flake. `README-v4.md` covers TypeScript in one page; `FORMATS.md` is now generated from the Rust tokenizer (224 formats). ([#989](https://github.com/kucherenko/jscpd/pull/989), [#990](https://github.com/kucherenko/jscpd/pull/990))
- **Floating `v5` tag for the GitHub Action** — `uses: kucherenko/jscpd@v5` follows the latest 5.x release. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **crates.io metadata** — every crate declares `repository`, `documentation`, `keywords`, `categories`; `jscpd` excludes `tests/` from the package and ships an expanded docs.rs README; npm packages carry `funding`. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **Signed release assets** — each archive and `checksums.txt` now has a Sigstore keyless signature, verifiable with `cosign verify-blob`. ([#988](https://github.com/kucherenko/jscpd/pull/988))
- **CI** — Windows joined the pull-request build matrix, a smoke test runs the release binary against `fixtures/` on every push, and a nightly job runs `cargo audit`/`cargo deny`. ([#988](https://github.com/kucherenko/jscpd/pull/988), [#989](https://github.com/kucherenko/jscpd/pull/989))

### Dependencies

- Bump `quick-xml` to 0.42.0 in `/rust` ([#991](https://github.com/kucherenko/jscpd/pull/991))

---

## 5.1.1

### Bug Fixes

- **`jscpd` on npm installed the 5.0.16 engine instead of 5.1.0** — the `jscpd` wrapper's `optionalDependencies` were pinned to the `5.0.16` platform binaries, so `npm i jscpd@5.1.0` resolved a binary one release behind. Everything 5.1.0 fixed was absent for `jscpd` users, including the Windows `--baseline-from-ref` fix; the `cpd` package and the platform packages were unaffected.

  Cause: `scripts/sync-version.mjs` updated the wrapper's version and its platform pins behind one `version !== npmVersion` guard, so once anything set `version` first, the guard read "already up to date" and skipped the pins. The two now update independently, and the script verifies every npm version and platform pin matches the release version before finishing — this is the same defect that produced the 5.0.13 republish, this time covering the `jscpd` wrapper too.

### Other

- **Declared MSRV corrected to 1.96** — the workspace advertised `rust-version = "1.87"` on crates.io, a floor it couldn't actually build on (the `oxc` crates need 1.96.0). CI now derives the toolchain from `rust-version` and builds at exactly that version.

---

## 5.1.0

### New Features

- **Windows on ARM support** — npm installs select a native `aarch64-pc-windows-msvc` binary from `jscpd-windows-arm64-msvc`.
- **Clone baseline (`--baseline`, `--update-baseline`, `--fail-on-new-clones`)** — gate CI on *new* duplication only. A committed baseline file records content-hash fingerprints of accepted clones; clones absent from it are new, and `--fail-on-new-clones[=N]` exits 1 past N (default 0) — independent of `--threshold`, so legacy duplication is tolerated while regressions fail the build. `--update-baseline` rewrites the file and prints added/removed counts. New-clone info flows through the reporters: `[NEW]` markers in console, `isNew`/`newClones`/`newDuplicatedLines` in JSON, level `error` in SARIF, matching gauges in OpenMetrics. ([#944](https://github.com/kucherenko/jscpd/issues/944))
- **Ephemeral baseline from a git ref (`--baseline-from-ref`)** — stateless PR gating without a committed file: checks the base ref out into a temporary worktree, scans it, and compares in memory. Costs a second scan where the committed file needs only one; fails with a clear hint when the ref is missing in a shallow checkout. ([#944](https://github.com/kucherenko/jscpd/issues/944))
- **OpenMetrics reporter (`--reporters openmetrics`)** — writes `jscpd-metrics.txt`, ready for a GitLab CI `artifacts:reports:metrics` artifact. ([#422](https://github.com/kucherenko/jscpd/issues/422))
- **CodeClimate / GitLab Code Quality reporter (`--reporters codeclimate`, alias `gitlab`)** — writes `gl-code-quality-report.json` for `artifacts:reports:codequality`, with a deterministic per-fingerprint id so GitLab can tell new issues from old ones across runs. Severity is `minor`, escalating to `major` past a baseline or `--threshold`. ([#958](https://github.com/kucherenko/jscpd/issues/958))
- **Config discovery in `.config/`** — also checks `.config/jscpd.json` per the [dot-config convention](https://dot-config.github.io/), between the root `.jscpd.json` and `package.json`'s `jscpd` key. A root file still wins. ([#979](https://github.com/kucherenko/jscpd/issues/979))

### Bug Fixes

- **Unknown `--format` values warn instead of silently matching nothing** — a typo like `cs` used to scan 0 files and exit 0. Now prints a stderr warning naming the value and pointing to `--list`. ([#964](https://github.com/kucherenko/jscpd/issues/964))
- **Nix flake builds again** — the flake pinned a mutable manifest hash that broke on a Rust point release. Now pinned to the exact patch version. ([#976](https://github.com/kucherenko/jscpd/issues/976))
- **Windows: `--baseline-from-ref` no longer reports every clone as new** — the format-suffix stripper mistook the drive colon in Windows verbatim paths for a format suffix, truncating source ids and breaking every fingerprint lookup silently. A colon before a separator is now read as structural; fingerprints are also line-ending agnostic (CR stripped before hashing).

### Other

- **Glama MCP listing** — a `glama.json` manifest and a `Dockerfile` for the stdio MCP server, used by [Glama](https://glama.ai/mcp/servers/kucherenko/jscpd) for its server listing.
- **Signed releases** — SLSA provenance on release artifacts; piped downloads in workflows are pinned (OpenSSF Scorecard).

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

- **MCP server over stdio (`--mcp`)** — `cpd --mcp /path/to/project` serves MCP on stdin/stdout, no port or network policy needed. The project is scanned once at startup and kept in memory, so `check_duplication` snippet checks answer in milliseconds. Tools: `check_duplication`, `get_file_clones`, `get_statistics`, `check_current_directory`; all lists sorted biggest-first, capped by an optional `limit` (default 100). Implements protocol `2025-06-18`, accepting older clients; standard detection options apply to snippet checks too. ([#891](https://github.com/kucherenko/jscpd/issues/891))
- **Codebase summary (`--summary`)** — opt-in hotspot overview: top files/folders by tokens, lines, size, or a token-based complexity estimate, each with its duplication share. `--summary-top <n>` and `--summary-by tokens|lines|size|complexity` control it. Renders in console, compactly in `ai`, and as an additive `summary` JSON key (absent when off). ([#934](https://github.com/kucherenko/jscpd/issues/934))
- **Isolated folder groups (`--skip-isolated`)** — skip duplication between monorepo folders owned by different teams (`--skip-isolated "packages/team-a|packages/team-b,libs/a|libs/b"`); duplication inside one folder or against shared code is still reported. Ports [#628](https://github.com/kucherenko/jscpd/pull/628) to the Rust engine. ([#942](https://github.com/kucherenko/jscpd/pull/942))

### Security

- **Supply-chain hardening (OpenSSF Scorecard)** — every Action pinned to a commit SHA, least-privilege workflow tokens, and a `SECURITY.md` with private disclosure.

### Bug Fixes

- **GitHub "Latest" release badge stays on v5** — Rust v5 releases use `--latest`; v4 and `cpd v*` releases opt out, so a v4 maintenance release can't steal the badge.

### Other

- **npm package page polish** — README links are absolute so they resolve on npmjs.com; description and keywords improved.

### Dependencies

- Bump Rust toolchain to 1.97 and `oxc` crates to 0.144 in `/rust`
- Bump `serde` to 1.0.229, `clap` to 4.6.6, `memchr` to 2.8.3, `xxhash-rust` to 0.8.18 in `/rust`

### Thank You ❤️

- [@hanzhangyu](https://github.com/hanzhangyu) for proposing isolated folder groups and the original `skipIsolated` implementation ([#628](https://github.com/kucherenko/jscpd/pull/628)), ported here to the Rust engine

---

## 5.0.15

### New Features

- **SARIF: size-based severity** — `--sarif-error-tokens <N>`: clones with at least N tokens report at level `error`, smaller ones stay `warning`; past `--threshold`, all results become `error`. ([#908](https://github.com/kucherenko/jscpd/issues/908))
- **SARIF: clone fingerprints** — each result carries a `partialFingerprints` entry (`jscpdCloneHash/v1`), order-insensitive, for cross-run identity in tools like GitHub code scanning. ([#909](https://github.com/kucherenko/jscpd/issues/909))
- **SARIF: related-location messages** — the counterpart location gets a message and an embedded link so GitHub code scanning displays it. ([#911](https://github.com/kucherenko/jscpd/issues/911))
- **SARIF: richer rule metadata** — `jscpd/duplicate-code` now has a display name, description and quality tags for SARIF viewers and Azure DevOps. ([#914](https://github.com/kucherenko/jscpd/pull/914))

### Bug Fixes

- **Scan-root-relative report paths** — fragments store their scan root separately, so paths are relative to the scanned directory again while reporters can still resolve source files. Fixes empty snippets and unresolvable paths on multi-root scans. ([#872](https://github.com/kucherenko/jscpd/issues/872), [#892](https://github.com/kucherenko/jscpd/issues/892))
- **Report version stamping** — SARIF and HTML report versions now match `cpd --version`. ([#915](https://github.com/kucherenko/jscpd/issues/915))
- **Multi-root blame attribution** — git blame data is now keyed by resolved path, so a second scan root no longer inherits the first root's authors.
- **Git root discovery** — walking up from a relative scan path no longer stops short of the repository root.

### Dependencies

- Bump `serde_json` to 1.0.151, `ignore` to 0.4.32, `anyhow` to 1.0.104 in `/rust`

### Thank You ❤️

This release was shaped by community contributions:

- [@chrisc-onaorg](https://github.com/chrisc-onaorg) for SARIF clone fingerprints ([#910](https://github.com/kucherenko/jscpd/pull/910)), related-location messages ([#912](https://github.com/kucherenko/jscpd/pull/912)), richer rule metadata ([#914](https://github.com/kucherenko/jscpd/pull/914)), plus reporting #909, #911, #915
- [@darronz](https://github.com/darronz) for the scan-root-relative paths fix ([#913](https://github.com/kucherenko/jscpd/pull/913))
- [@nvuillam](https://github.com/nvuillam) for proposing size-based SARIF severity ([#908](https://github.com/kucherenko/jscpd/issues/908))

---

## 5.0.14

### New Features

- `--cross-formats` — detect clones across related formats sharing one comparison pool, e.g. `"javascript,typescript"` or the `js-ts` preset. When a group mixes TS with JS, TS files compare with erasable types stripped, so `function f(a: number): void` matches `function f(a)`. Also configurable as `crossFormats` in `.jscpd.json`/`package.json`. ([#810](https://github.com/kucherenko/jscpd/issues/810))

### Bug Fixes

- **Prose-only Markdown files are now analyzed** — the tokenizer only extracted fenced code blocks, so `.md` files without fences produced zero tokens and were silently skipped. Prose is now tokenized too. ([#883](https://github.com/kucherenko/jscpd/issues/883))

### Dependencies

- Bump `regex` to 1.13.1, `globset` to 0.4.19, `ignore` to 0.4.28, `xxhash-rust` to 0.8.16 in `/rust`

---

## 5.0.13

npm-only release: republished `cpd` so its `optionalDependencies` point at the 5.0.12 platform binaries (it still referenced 5.0.11's). No code changes; the version sync script now keeps these in sync automatically.

---

## 5.0.12

### Dependencies

- Bump `askama` to 0.16.0, `log` to 0.4.33, `env_logger` to 0.11.11, `rustc-hash` to 2.1.3 in `/rust`

---

## 5.0.11

### New Features

- **Razor (.razor) support** — new tokenizer parsing HTML content and Razor keyword blocks (thanks [@chrisc-onaorg](https://github.com/chrisc-onaorg) in [#829](https://github.com/kucherenko/jscpd/pull/829))

### Bug Fixes

- Fix `cargo test` on Windows via `CARGO_BIN_EXE_cpd`/`.exe` suffix to locate the test binary
- Fix `cargo fmt` formatting across the workspace

### Refactoring

- Extract methods and optimize intervals in `cpd-core` (bumped to 0.1.6)

### Dependencies

- `cpd-core` to 0.1.6, `cpd-tokenizer` to 0.1.7 across dependent crates

---

## 5.0.10

### Bug Fixes

- Emit scan-root-relative paths in all reporters by default. `jscpd /abs/path` from another CWD used to leave absolute paths in every report, and Windows/macOS canonicalization could leave `\\?\` or `./` prefixes. Paths now normalize against the canonicalized scan root and strip any leading `./`. Fixes [#827](https://github.com/kucherenko/jscpd/issues/827)
- Fix `--skip-local` to match v4 semantics: filters clones where both fragments share a scan root, not just a parent directory

### Refactoring

- DRY duplication in reporters: shared helpers moved into `cpd-reporter/src/shared.rs`, reused by console, console-full, CSV, JSON, HTML, Markdown, silent, XML and SARIF — cutting the monorepo's own reported duplication from 5.0% to 0.56% and fixing a latent `--absolute` bug along the way
- Move blame enrichment from `gitoxide` to `git blame --porcelain`
- Resolve `needless_borrow` clippy warnings in CSV and Markdown reporters

### Documentation

- Add Nix and Homebrew install instructions. [#818](https://github.com/kucherenko/jscpd/issues/818)
- Point project homepage URLs at `https://jscpd.dev`, add curl install instructions, clean up outdated badges
- Remove defunct Universal Analytics tracking pixels from READMEs

---

## 5.0.9

### New Features

- GitHub Action for jscpd (Rust v5) on the Actions Marketplace: `uses: kucherenko/jscpd/.github/workflows/action.yml@v5`

### Bug Fixes

- Resolve platform binary resolution when `cpd` is installed as a nested dependency. Fixes [#816](https://github.com/kucherenko/jscpd/issues/816)

---

## 5.0.8

### Bug Fixes

- Prevent mmap exhaustion when scanning repos with more files than `vm.max_map_count` (default 131,072 on Linux): each worker now opens and drops its mapping within its own closure instead of holding one live per file. Fixes [#813](https://github.com/kucherenko/jscpd/issues/813)
- Fix `--pattern` not matching relative paths when the scan root is absolute. Fixes [#811](https://github.com/kucherenko/jscpd/issues/811)
- Fix trailing-newline off-by-one in the line-count filter

---

## 5.0.7

### Bug Fixes

- Prevent stack overflow on deeply-nested JS/TS files (e.g. Bun's `test/bundler`): OXC's recursive-descent parser allocates one stack frame per nesting level, exceeding the default 8 MiB thread stack. Fixed with a local 64 MiB-stack thread pool
- Default `--max-size` to `1mb`, matching v4, so OXC never sees megabyte-scale generated files
- `--workers N` now takes effect on every `run()` call, not just the first

---

## 5.0.6

### New Features

- v4 config backward compatibility for `.jscpd.json` fields `path`, `pattern`, `ignore`, `ignorePattern`
- `ignore` and `ignorePattern` are now distinct: file-level globs vs. code-level regex (previously conflated)
- `.jscpd.json` `path` field support, resolved against the config file's directory
- `jscpd` npm wrapper package, publishing the same binary under a second name
- `--exit-code` now matches v4: optional integer value, independent of `--threshold`
- Performance: memory-mapped file I/O, SIMD line counting, `flat_map` detection pipeline, no source-string cloning in the JS tokenizer (thanks [@auterium](https://github.com/auterium), [#808](https://github.com/kucherenko/jscpd/pull/808))

### Bug Fixes

- Fixed `--exit-code` to match v4's optional-integer behavior
- Fixed unique temp dir generation in reporter tests under parallel runners

---

## 5.0.4

### New Features

- CLI alignment with v4: `--absolute`, `--ignore-case`, `--formats-exts`, `--formats-names`; fixed `--threshold`, improved `--max-size`
- Detection and statistics aligned with jscpd v4
- Side-by-side blame comparison in console-full
- Clone list display in console

### Bug Fixes

- HTML reporter now outputs `jscpd-report.html` at the `output_dir` root
- Resolved all clippy warnings across the workspace
- Fixed unique temp dir generation in tests

---

## 5.0.3

### New Features

- Rust-based cpd CLI with full feature parity to TypeScript jscpd
- Cross-platform npm binaries (linux-x64-gnu, linux-arm64-gnu, linux-x64-musl, darwin-arm64, darwin-x64, windows-x64-msvc)
- 13 reporters: json, console, xml, csv, html, markdown, sarif, ai, badge, xcode, threshold, silent, console-full
- Time reporter, CLI short-form aliases, ReportContext data structure, crates.io Trusted Publishing via OIDC

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

- Initial Rust workspace with cpd-core, cpd-tokenizer, cpd-finder, cpd-reporter, jscpd crates
- Cross-format detection for Vue SFC, Svelte, Astro, Markdown
- Shebang detection for extensionless scripts

---

## 5.0.0

### Breaking Changes

- First stable Rust release — replaces the TypeScript CLI with a native binary
- Reporter trait signature changed to use ReportContext instead of Statistics directly
