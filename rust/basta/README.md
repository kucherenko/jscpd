# basta — dead code detector

[![npm version](https://img.shields.io/npm/v/basta.svg)](https://www.npmjs.com/package/basta)
[![license](https://img.shields.io/npm/l/basta.svg)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)

Finds code nothing runs: unused files, exports, module-private declarations
and imports across **JavaScript, TypeScript, JSX, TSX and Python**, and Rust
through the compiler's own diagnostics.

A self-contained Rust binary. No runtime dependencies, no install scripts,
nothing to configure before the first run.

```bash
npm install -g basta
basta src
```

```
Unused files (1)
 - src/legacy-export.ts  certain 95%  11 lines

Unused exports (1)
 - function src/invoice.ts:21:17 renderReceipt  high 85%  3 lines

Unused symbols (1)
 - function src/invoice.ts:29:10 describeTotal  certain 90%  3 lines

Unused imports (1)
 - import src/invoice.ts:2:10 roundToCents  certain 100%  1 lines

Found 4 dead code findings in 4 files (31.6% of 57 lines).
Done in 12ms
```

Without installing:

```bash
npx basta src
```

## What makes it different

**It is a traversal, not a reference count.** basta builds the import graph
from the project's entry points and walks it. A helper whose only caller is
itself dead is reported too — that cascade is most of what a
reference-counting linter misses.

**It tells you how sure it is.** Static analysis of JavaScript and Python
cannot be certain, so every finding carries a score from 0 to 100 and the
reasons it is not higher: a file that calls `eval` or `getattr`, an
unrecognised decorator, a wildcard re-export, a name that shows up in a string
literal, a file whose path is written out in one. `--min-confidence` sets the floor; the default is 60.

**It knows the conventions.** A package's `__init__.py` re-exports are its
API, not unused imports. `from __future__ import annotations` is a directive.
`# noqa: F401` and PEP 484's `import x as x` mean a deliberate re-export. A
quoted annotation under `if TYPE_CHECKING:` uses the import that supplies it.
`compilerOptions.paths` from `tsconfig.json` is resolved, so the `@/components/x`
alias in the default Next.js template reaches the file it names. CommonJS
counts: `require('./x')`, `module.exports = { a }` and a literal `import('./x')`
are edges like any `import`. A shell script or CI workflow that names a source
file keeps it alive.

## Usage

```
basta [PATHS]...

  --categories <LIST>        unused-file, unused-export, unused-symbol,
                             unused-import, unused-member, or `all`
  --min-confidence <N>       drop findings below this score (default: 60)
  --entry <GLOB>             treat matching files as entry points (repeatable)
  -c, --config <FILE>        jscpd config to read the dead-code section from
                             (default: .jscpd.json, .config/jscpd.json,
                             package.json)
  --rust-diagnostics <FILE>  Rust dead code from `cargo check
                             --message-format=json`; `-` reads stdin
  --frameworks-config <FILE> framework definitions to add, YAML or JSON
                             (default: basta.frameworks.{yaml,yml,json})
  --framework <NAME>         take this framework as present (repeatable)
  --no-frameworks            do not detect frameworks
  --ignore <GLOB>            skip matching files (repeatable)
  --include-tests            report dead code inside test files
  --include-entry-exports    report exports of entry points
  --min-lines <N>            only report declarations this many lines or longer
  -r, --reporters <LIST>     console, json, sarif, html, markdown, csv, xml,
                             codeclimate, openmetrics, badge, xcode, ai, silent
  -o, --output <DIR>         where file reporters write (default: report)
  --threshold <PERCENT>      fail when dead code exceeds this share
  --exit-code <CODE>         exit with this code when anything is found
  --format <FORMAT>          restrict the scan (repeatable)
  --list                     print the formats basta analyzes
  --list-frameworks          print the frameworks basta recognises
```

## Entry points

Every finding answers *nothing reaches this*, so the entry points are the whole
basis of the result. basta finds them from `package.json` (`main`, `module`,
`bin`, `exports`, `files`, `scripts`), `pyproject.toml` (`[project.scripts]` and
entry-point tables), scripts in the tree (`*.sh`, CI workflows, Makefiles,
Dockerfiles) that name a source file, and conventions: `src/index.ts`,
`__main__.py`, `manage.py`, framework routes under `pages/` and `app/`,
`*.config.ts`, `.d.ts` declarations, shebangs, `if __name__ == "__main__"`, and
every `__init__.py`.

When a project does something none of that covers, say so once:

```bash
basta src --entry 'src/handlers/**' --entry 'scripts/*.ts'
```

## Frameworks

A framework runs files that nothing imports. A router turns the files in
`pages/` into URLs. A runtime loads everything in `plugins/`. basta works out
which frameworks a project uses and keeps the files they load out of the
report.

It knows Next.js, Nuxt, Nitro, WXT, Plasmo, Remix, React Router, SvelteKit,
Astro, SolidStart, TanStack Start, Qwik City, Gatsby, Angular, NestJS, AdonisJS,
Strapi, Medusa, Ember, Quasar, React Native, Expo, Cloudflare Workers, Vercel,
Netlify, Serverless, Docusaurus, VitePress, Eleventy, Storybook, Jest, Vitest,
Playwright, Cypress, Prisma, Knex and TypeORM. On the Python side it knows
Django, Alembic and Scrapy. Run `basta --list-frameworks` to see all of them,
with the signs basta uses to detect each one.

A framework counts as present when basta finds any one of these:

- its config file, by name (`next.config.mjs`, `wxt.config.ts`,
  `.storybook/main.ts`);
- its package among the dependencies in `package.json`, in any of the
  dependency tables;
- its section in `package.json` (`"jest": {…}`).

basta looks in every directory that holds a scanned file, so each package of a
monorepo is treated as its own project. Most projects use more than one
framework. All the frameworks that match apply at the same time, and each one
keeps its own files alive.

If the framework's config is a JavaScript or TypeScript file, basta reads the
plain values that move directories around: `srcDir`, `entrypointsDir`,
`appDirectory` and `imports: false`. It reads a JSON config such as
`nest-cli.json` the same way. The console report lists what was detected above
the findings, for example `Frameworks: next (apps/web), vitest`.

The list of frameworks is a data file,
[`frameworks.yaml`](https://github.com/kucherenko/jscpd/blob/master/rust/crates/basta/frameworks.yaml),
built into the binary. You can add a framework basta does not know, or replace
a built-in one that has the same name, with a file of the same shape:

```yaml
# basta.frameworks.yaml — or .yml / .json, or --frameworks-config <file>
frameworks:
  - name: kiosk-router
    detect:
      configFiles: ["kiosk.config.{js,ts}"]
      dependencies: ["@acme/kiosk-router"]
      packageJsonKeys: [kioskRouter]
    variables:
      screensDir: screens            # read from the config file, else this
    bases: [".", "src"]
    entry: ["${screensDir}/**/*.screen.{js,ts}"]
    directories: [plugins]           # loaded whole
    autoImports:
      disabledBy: imports            # `imports: false` turns these off
      directories: [composables]
    globals:                         # names the framework reads: always used
      - onKioskBoot                  # …in any file of the project
      - names: [guard, title]        # …or only in the files it reads them from
        files: ["${screensDir}/**/*.screen.{js,ts}"]
```

`globals` are the names a framework looks up in your code. Examples are Next's
`getServerSideProps` and `generateMetadata`, Remix's `loader` and `action`,
SvelteKit's `load`, Angular's `ngOnInit` and a Pages Function's
`onRequestGet`. No file in the project mentions these names, so basta treats a
declaration with such a name as used. It never reports it, and everything that
declaration calls stays reachable. The built-in list has these names for the
frameworks that need them, limited to the files each framework reads them
from.

Use `--framework next` when the scan starts below the `package.json` that
names the framework, as in `basta src --framework next`. basta then treats the
framework as present. `--no-frameworks` turns detection off, and entry points
then come only from manifests, conventions and `--entry`.

## Rust

basta does not parse Rust. The compiler already finds unused code and prints
it on every build, for example `function `reprint` is never used`. It has
real name resolution, trait dispatch and macro expansion, which no scan of
the source could match, so basta reads the compiler's output instead:

```bash
cargo check --all-targets --message-format=json | basta . --rust-diagnostics -
```

Or from a file, which lets CI reuse a check it already runs:

```bash
cargo check --all-targets --message-format=json > target/check.json
basta . --rust-diagnostics target/check.json
```

basta never runs cargo itself. Running it would execute the project's build
scripts and procedural macros, and everything else basta does is safe to run
on a repository nobody has read.

The findings go through the same categories, reporters and `--min-confidence`
as every other language, at 100% confidence. `dead_code` on a function,
struct, enum, constant or trait is an unused symbol. On a method, field or
variant it is an unused member. `unused_imports` is an unused import. The
compiler's span covers only the name, so basta reads each item's real size
from the source.

Test code is handled the same way as in every other language. The compiler
says which target a diagnostic came from, and `cargo check --all-targets`
checks the test harness too. A finding from a test target is reported only
with `--include-tests`, at 85% rather than 100%, because a test helper is
often kept for the next test.

basta reports only what the compiler reports. A `pub` item of a library is
never reported, because a crate outside the workspace may use it. Code that
only exists under a feature or target the check did not build is reported as
dead, because for that build it is, so check with the features and targets
you ship. `#[allow(dead_code)]` hides an item from cargo and so from basta.
`RUSTFLAGS="--force-warn dead_code"` shows it anyway.

The file can also be named in the [configuration file](#configuration-file)
as `rustDiagnostics`, which is how `jscpd --dead-code` reads it.

## Configuration file

Flags apply to one run. Settings a project always wants belong in its jscpd
config, under `deadCode`. The key can also be spelled `dead-code` or `basta`.

basta reads the file you pass with `--config`. Without that flag it looks in
the working directory for `.jscpd.json`, then `.config/jscpd.json`, then the
`jscpd` key of `package.json`. `jscpd --dead-code` reads the same file, so the
two tools give the same result.

```json
{
  "threshold": 10,
  "deadCode": {
    "minConfidence": 80,
    "categories": ["unused-file", "unused-export", "unused-symbol", "unused-import"],
    "minLines": 0,
    "entry": ["tools/*.js"],
    "ignore": ["**/generated/**"],
    "includeTests": false,
    "includeEntryExports": false,
    "threshold": 5,
    "framework": ["next"],
    "noFrameworks": false,
    "frameworksConfig": "tools/frameworks.yaml",
    "rustDiagnostics": "target/check.json",
    "frameworks": [
      {
        "name": "job-runner",
        "detect": { "packageJsonKeys": ["jobRunner"] },
        "entry": ["jobs/**/*.job.js"],
        "globals": [{ "names": ["schedule"], "files": ["jobs/**/*.job.js"] }]
      }
    ]
  }
}
```

A flag wins over the same setting in the section. A list given on the command
line replaces the list in the section and does not add to it.

`frameworks` holds definitions inline, in the same shape as
`basta.frameworks.yaml`. They are applied last, so they win over a definitions
file. `threshold` is the limit for dead code only. The top-level `threshold`
next to it is the limit for duplication. `enabled` matters to jscpd, which has
other modes to choose between, and basta ignores it.

A misspelled key stops the run when you named the file with `--config`. For a
file basta found by itself, it prints a warning and ignores the section.

## In CI

```bash
basta src --reporters json,sarif --threshold 5
```

`--threshold` fails the run when dead code exceeds that share of the scanned
lines; `--exit-code` sets the code to fail with. The SARIF report uploads to
GitHub code scanning like any other analyzer's.

## Known gaps

Honest ones, so you can judge whether the tool fits your project:

- **No Vue, Svelte or Astro.** Imports that start inside a single-file
  component are invisible, so a project built on one of them will report live
  files as unreachable.
- **Class members are matched by name**, since basta does not infer types.
  That rule is off by default; `--categories all` turns it on.
- **PEP 420 namespace packages** are not import roots: `from lib.x import y`
  resolves only when `lib/` carries an `__init__.py`.

## Related

- [`jscpd`](https://www.npmjs.com/package/jscpd) — the copy/paste detector this
  engine ships alongside. `jscpd --dead-code` runs the same analysis.
- [crates.io/crates/basta](https://crates.io/crates/basta) — `cargo install basta`
- [jscpd.dev](https://jscpd.dev) — documentation

## License

MIT
