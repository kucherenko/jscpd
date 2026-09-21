# basta

Find dead code: unused files, exports, declarations and imports across
JavaScript, TypeScript, Vue, Svelte, Astro and Python.

basta builds the import graph from a project's entry points, walks it, and
reports what it never reaches. It ships as its own `basta` command and inside
[jscpd](https://jscpd.dev) as `jscpd --dead-code`.

```bash
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

## What makes it different

**It is a traversal, not a reference count.** A helper whose only caller is
itself dead is reported too. That cascade is most of what a reference-counting
linter misses.

**It tells you how sure it is.** Static analysis of JavaScript and Python
cannot be certain, so every finding carries a score from 0 to 100 and the
reasons it is not higher — a file that calls `eval` or `getattr`, an
unrecognised decorator, a wildcard re-export, a name that shows up in a string
literal. `--min-confidence` sets the floor; the default is 60.

**It reads components, not just scripts.** A `.vue`, `.svelte` or `.astro`
file is a module wrapped in markup, and the markup is where a component's
imports are actually used. basta reads both halves, so `<Foo />` counts as the
use of `import Foo from './Foo.vue'` — kebab-case spellings, `{{ … }}`
interpolations, Svelte actions and attribute expressions included. A tool that
stops at `</script>` does not merely miss findings here; it reports every
component import in the project as dead.

**It knows the conventions.** A package's `__init__.py` re-exports are its API,
not unused imports. `from __future__ import annotations` is a directive, not a
binding. `# noqa: F401` and PEP 484's `import x as x` mean a deliberate
re-export. A `package.json` pointing at `dist/index.js` means `src/index.ts`.
CommonJS counts: `require('./x')`, `module.exports = { a }` and a literal
`import('./x')` are edges like any `import`. A shell script or CI workflow
that names a source file keeps it alive.

## Usage

```bash
basta [PATHS]...

  --categories <LIST>        unused-file, unused-export, unused-symbol,
                             unused-import, unused-member, or `all`
  --min-confidence <N>       drop findings below this score (default: 60)
  --entry <GLOB>             treat matching files as entry points (repeatable)
  -c, --config <FILE>        jscpd config to read the dead-code section from
                             (default: .jscpd.json, .config/jscpd.json,
                             package.json)
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

Every finding is the answer to *nothing reaches this*, so the entry points are
the whole basis of the result. basta finds them from `package.json`
(`main`, `module`, `bin`, `exports`, `files`, `scripts`), `pyproject.toml`
(`[project.scripts]` and entry-point tables), scripts in the tree (`*.sh`,
CI workflows, Makefiles, Dockerfiles) that name a source file, and
conventions: `src/index.ts`,
`__main__.py`, `manage.py`, framework routes under `pages/` and `app/`,
`*.config.ts`, `.d.ts` declarations, shebangs, `if __name__ == "__main__"`,
every `__init__.py`, and the file-system routes of the component frameworks —
`pages/`, `layouts/`, `src/routes/`, `app.vue`, `error.vue`.

When a project does something none of that covers, say so once:

```bash
basta src --entry 'src/handlers/**' --entry 'scripts/*.ts'
```

**It reads the project's own build.** `resolve.alias` from `vite.config.*`,
`kit.alias` and `$lib` from SvelteKit, `paths` from `tsconfig.json`, and — in a
monorepo — every workspace package's name, so `@acme/ui/date` reaches the file
that package's `exports` names. A specifier computed over a directory
(``import(`./locales/${l}.json`)``, `import.meta.glob('./lang/**/*.ts')`) is
expanded to the files its pattern matches, the way a bundler expands it; a
literal `import('./x.svelte')` in a component's markup and the imports of an
Astro client `<script>` are edges like any other.

A framework's aliases come with it: a `wxt.config.ts` means `@`/`~` are its
`srcDir` and `@@`/`~~` the project root (WXT declares them only in the
generated `.wxt/tsconfig.json`, which no repository commits). When no config
declares them, `~/x`, `@/x` and `~~/x` fall back to the project root, which is
what they mean in Nuxt — its alias table is generated into `.nuxt/` and never
scanned.

## Frameworks

A framework starts files no `import` names: a router turns `pages/` into URLs,
a runtime loads `plugins/` whole. basta detects which one is at work and roots
what it loads — Next.js, Nuxt, Nitro, WXT, Plasmo, Remix, React Router,
SvelteKit, Astro, SolidStart, TanStack Start, Qwik City, Gatsby, Angular,
NestJS, AdonisJS, Strapi, Medusa, Ember, Quasar, React Native, Expo, Cloudflare
Workers, Vercel, Netlify, Serverless, Docusaurus, VitePress, Eleventy,
Storybook, Jest, Vitest, Playwright, Cypress, Prisma, Knex, TypeORM and more;
`basta --list-frameworks` prints all of them with what gives each away.

A project is rarely one framework: every framework whose signal matches is in
force at once — `Frameworks: next, storybook, vitest` — and each roots its own
files. Any one signal is enough, looked for in every directory that holds a
scanned file, so each package of a monorepo is its own project:

- the framework's **config file** by name (`next.config.mjs`, `wxt.config.ts`,
  `.storybook/main.ts`);
- the package among the **dependencies** of `package.json`, in any table;
- its **section** in `package.json` (`"jest": {…}`).

A config written as source is read for the literals that move directories
(`srcDir`, `entrypointsDir`, `appDirectory`, `imports: false`), a JSON one
(`nest-cli.json`) the same way. Detected frameworks are named above the
findings: `Frameworks: next (apps/web), vitest`.

The whole table is data — [`frameworks.yaml`](https://github.com/kucherenko/jscpd/blob/master/rust/crates/basta/frameworks.yaml),
compiled into the binary. A project adds a framework basta has never heard of,
or replaces a built-in by name, with a file of the same shape:

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

`globals` are the names a framework looks up in the project's code — Next's
`getServerSideProps` and `generateMetadata`, Remix's `loader` and `action`,
SvelteKit's `load`, Angular's `ngOnInit`, a Pages Function's `onRequestGet`. No
file mentions them, so a declaration under one is taken as used: it is never
reported, and what it calls stays reachable. The built-in table carries them
for the frameworks that have them, scoped to the files each is read from.

`--framework next` takes a framework as present when the scan starts below the
`package.json` that would have named it (`basta src --framework next`), and
`--no-frameworks` leaves entry points to manifests, conventions and `--entry`.

## Configuration file

Flags are for one run; what a project always wants goes in its jscpd config,
under `deadCode` (or `dead-code`, or `basta` — the same key). basta reads the
file `--config` names, or the `.jscpd.json`, `.config/jscpd.json` or
`package.json` (`jscpd` key) of the working directory — the file
`jscpd --dead-code` reads, so the two agree:

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

Every key has a flag of the same name, and the flag wins: a list given on the
command line replaces the section's, it does not add to it. `frameworks` holds
definitions inline, in the same shape as `basta.frameworks.yaml`, and goes on
last, over a definitions file. `threshold` is dead code's own budget — the
top-level one beside it belongs to duplication. `enabled` is for jscpd, which
has other modes to choose from. A misspelled key is an error with `--config`
and a warning for a file that was only found.

## Adding a language

One file under `src/lang/` implementing `Analyzer`, and one line in the
`ANALYZERS` registry. The trait is the only place a language is allowed to be
special: it parses one file into declarations, imports and references,
resolves that language's specifiers against the index of scanned modules,
and declares its entry-point conventions, manifests and path traits. The
walker, the graph, the confidence model, the classifier, the reporters and
both CLIs are language-agnostic and pick the new language up from the
registry.

[`docs/basta-extending.md`](../../../docs/basta-extending.md) is the
walk-through: how a run works, the contract an analyzer has to honour (each
rule there was a real false positive once), a worked skeleton, and how to
prove a new language right against real projects.

## License

MIT
