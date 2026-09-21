# Dead code demo

Eight small projects. The first three — one TypeScript, one Python, one built
out of single-file components — contain one example of every finding `basta`
reports. The next two exist to show imports that only the project's own build
can resolve: a bundler's aliases and globs, and a monorepo's package names.
Two more have files no import reaches at all, because a framework starts
them: one basta recognises by itself, one the project has to describe. The
last keeps its dead-code settings in `.jscpd.json`.
Commands run from the repository root at default settings, with no threshold
or category flags, so they are the ones a user would actually type.

Each project is self-contained: scanning just that directory shows the whole
effect, and scanning all eight together reports the same twenty-six findings.

| Directory     | Language           | Findings                                          |
| ------------- | ------------------ | ------------------------------------------------- |
| `typescript/` | TypeScript, ESM    | 1 unused file, 1 unused export, 1 unused symbol, 1 unused import |
| `python/`     | Python package     | 1 unused file, 1 unused export, 2 unused symbols, 1 unused import |
| `components/` | Vue, Svelte, Astro | 1 unused file, 2 unused exports, 1 unused symbol, 1 unused import |
| `bundler/`    | Vite, Vue          | 1 unused file, 1 unused export                    |
| `monorepo/`   | pnpm workspace, TypeScript | 1 unused file, 1 unused export            |
| `frameworks/detected/` | `@fastify/autoload` and Vitest, together | 1 unused file          |
| `frameworks/custom/`   | An in-house router | 3 unused files, until the project describes it |
| `config/`     | A `.jscpd.json` with a dead-code section | 3 unused files and 1 unused export, 1 finding once the config is read |

## What an entry point is

Nothing here is marked dead because it "looks unused" — every finding is the
answer to *no entry point reaches this*. The projects declare their entry
points the way real projects do:

- `typescript/package.json` sets `"main": "./src/index.ts"`.
- `python/shop/__main__.py` is what `python -m shop` runs, and
  `python/shop/__init__.py` is the package's public surface.
- `components/package.json` sets `"main": "./src/main.ts"`, and
  `components/src/pages/status.astro` is a file-system route: the framework
  imports it, no file does.
- `bundler/package.json` sets `"main": "./src/main.js"`.
- `monorepo/apps/shop/package.json` sets `"main": "./src/main.ts"`. The
  `packages/ui` package declares no entry point at all, which is the point of
  that demo.

Delete any one of those declarations and its project reads as dead, which is
the failure mode the entry-point rules exist to prevent.

## TypeScript

`src/index.ts` imports `renderInvoice` and `formatMoney`. Following the
imports from there reaches `invoice.ts` and `money.ts` and nothing else.

```bash
jscpd --dead-code fixtures/dead-code-demo/typescript --no-colors
# Unused files (1)
#  - src/legacy-export.ts                 certain 95%
# Unused exports (1)
#  - function src/invoice.ts:21:17 renderReceipt      high 85%
# Unused symbols (1)
#  - function src/invoice.ts:29:10 describeTotal      certain 90%
# Unused imports (1)
#  - import src/invoice.ts:2:10 roundToCents          certain 100%
# Found 4 dead code findings in 4 files (31.6% of 57 lines).
```

Each one shows a different rule:

- **`src/legacy-export.ts`** — no module imports it and no rule makes it an
  entry point. Note what is *not* reported: the file exports a function, a
  constant and a type, and none of them is listed separately. The file is the
  finding; listing its contents underneath would bury the one line that
  matters.
- **`renderReceipt`** — exported from a live file, but nothing imports the
  name. Reported at 85 rather than 95 because an export can be used by code
  outside the scan; that is what the confidence number is for.
- **`describeTotal`** — module-private, and its only caller is
  `renderReceipt`, which is itself unreachable. This is the cascade: dead code
  reached only from other dead code is dead too, which a reference count would
  miss.
- **`roundToCents`** — `invoice.ts` imports it and never mentions it again.

## Python

`shop/__main__.py` calls `complete_order`, which calls `_format_total` and
`apply_tax`. Everything else is unreachable.

```bash
jscpd --dead-code fixtures/dead-code-demo/python --no-colors
# Unused files (1)
#  - shop/legacy.py                       certain 95%
# Unused exports (1)
#  - function shop/checkout.py:14:5 refund_order      high 85%
# Unused symbols (2)
#  - function shop/checkout.py:19:5 _format_refund    certain 90%
#  - function shop/pricing.py:12:5 _unused_rounding   certain 90%
# Unused imports (1)
#  - import shop/pricing.py:1:8 os                    certain 100%
# Found 5 dead code findings in 5 files (34.5% of 55 lines).
```

Python has no `export` keyword, so the split between the two symbol rules
comes from convention:

- **`refund_order`** has no leading underscore, so it is part of the module's
  public surface — an **unused export**, reported the way an unimported
  `export` would be.
- **`_format_refund`** and **`_unused_rounding`** begin with an underscore, so
  they are module-private — **unused symbols**, and nobody outside the project
  can be relying on them.
- **`import os`** is never used. `import math` beside it *is* used, and is not
  reported.

`shop/__init__.py` re-exports `complete_order` and lists it in `__all__`.
Neither the import nor the name is reported: an import in a package's
`__init__.py` is the package surface, not an unused local binding.

## Components

A `.vue`, `.svelte` or `.astro` file is a JavaScript or TypeScript module
wrapped in markup, and basta reads both halves. Reading only the script would
not merely miss findings — it would invent them, because the markup is where a
component's imports are actually used.

```bash
jscpd --dead-code fixtures/dead-code-demo/components --no-colors
# Unused files (1)
#  - src/components/RetiredBanner.vue               certain 95%
# Unused exports (2)
#  - variable src/components/QueueGauge.svelte:5:14 caption   high 85%
#  - function src/units.ts:9:17 describeParcel                high 85%
# Unused symbols (1)
#  - variable src/App.vue:10:7 pendingLabel         certain 90%
# Unused imports (1)
#  - import src/App.vue:4:10 convertToPounds       certain 100%
# Found 5 dead code findings in 9 files (11.2% of 80 lines).
```

- **`RetiredBanner.vue`** — a component with no script at all. Nothing renders
  it and nothing imports it, so the file is the finding.
- **`caption`** — a Svelte prop. `export let level` beside it is read by the
  markup and is not reported; `caption` is read by nothing, which is what an
  unused prop looks like.
- **`convertToPounds`** — imported into `App.vue` and then mentioned neither in
  the script nor in the template.

What is *not* reported is the point of the rest of the demo. Each of these is
reached only through markup, and each would be a false positive for a tool
that stopped at `</script>`:

- **`ShipmentRow`** is imported in PascalCase and rendered as
  `<shipment-row>`, the kebab-case spelling Vue's own style guide prefers.
- **`formatWeight`** is reached only from a `{{ … }}` interpolation.
- **`StatusCard`** and **`QueueGauge`** are rendered by `status.astro` and
  imported by nothing else.
- **`clampLevel`** is reached only from inside an attribute expression,
  `value={clampLevel(level)}`.

Deleting any one of those five is a build break, and every one of them is
invisible to the script alone.

### Components a framework imports for you

Nuxt auto-imports everything under `components/` and `composables/`, so such a
file is alive with no `import` anywhere in the project. Guessing that from the
directory name would silence real findings in the many projects that do write
the import, so basta reads it from the project instead: a `nuxt.config.*`
beside the tree is what says the convention is in force.

This demo has no such config — its components are imported the ordinary way,
which is why `RetiredBanner.vue` and the unread `caption` prop are reported.
Add one and both stop being findings, because Nuxt would reach them:

```bash
echo "export default defineNuxtConfig({ srcDir: 'src/' })" \
  > fixtures/dead-code-demo/components/nuxt.config.ts
basta fixtures/dead-code-demo/components --no-colors
# Found 3 dead code findings in 10 files (6.2% of 81 lines).
rm fixtures/dead-code-demo/components/nuxt.config.ts
```

Nuxt's own two layouts — the tree at the root, and Nuxt 4's under `app/` — need
no `srcDir` line; it is read for the projects that moved theirs.

For a framework basta does not know, `--entry` says it once:

```bash
basta fixtures/dead-code-demo/components --entry 'src/components/**'
```

## Bundler imports

A project built with Vite keeps half its import graph in places a plain module
resolver never looks. This one has no `tsconfig.json`; `@` is declared only in
`vite.config.js`. Its pages are loaded by name, and its translations by a glob:

```js
import { openPage } from '@/router.js';                     // main.js
const loaded = await import(`./pages/${name}.vue`);         // router.js
const catalogs = import.meta.glob('./locales/*.js', { eager: true }); // i18n.js
```

```bash
jscpd --dead-code fixtures/dead-code-demo/bundler --no-colors
# Unused files (1)
#  - src/legacy/courier-api.js                      certain 95%
# Unused exports (1)
#  - function src/router.js:8:17 preloadPage        high 85%
# Found 2 dead code findings in 9 files (12.5% of 64 lines).
```

- **`courier-api.js`** — nothing imports it, and no alias or glob reaches
  `src/legacy/`.
- **`preloadPage`** — exported from the router and imported by nothing.

What is *not* reported is again the point. Each of these is reached only
through the build, and each was reported as **certain** before basta read it:

- **`router.js`** and **`i18n.js`** are imported through `@/`, which exists
  only in `vite.config.js`. The alias is read from there, including the
  `fileURLToPath(new URL('./src', import.meta.url))` form, and from
  `svelte.config.js` the same way — SvelteKit's `$lib` needs no config at all,
  because the `.svelte-kit/tsconfig.json` that declares it is never committed.
- **`pages/home.vue`** and **`pages/tracking.vue`** are named by no file. The
  template literal stands for the glob `./pages/*.vue`, which the bundler
  expands to every page that matches, so each of them is an edge.
- **`locales/en.js`** and **`locales/uk.js`**, and the `messages` each
  exports. A glob hands the importer whole module objects, so every export of
  a reached file counts as read; `catalog?.messages` picks one by a name
  computed at runtime.

## Monorepo

Inside a workspace a package is imported by its name, and only that package's
own `package.json` says which directory the name means:

```ts
import { badge } from '@demo/ui/badge';   // apps/shop/src/main.ts
```

```bash
jscpd --dead-code fixtures/dead-code-demo/monorepo --no-colors
# Unused files (1)
#  - packages/ui/src/tooltip.ts                     certain 95%
# Unused exports (1)
#  - function packages/ui/src/badge.ts:7:17 pill    high 85%
# Found 2 dead code findings in 3 files (37.5% of 16 lines).
```

`@demo/ui` declares no `main` and no `exports`, so nothing roots it as an
entry point: `badge.ts` is alive only because the app imports it by package
name. That name reaches as far as the workspace does — `pnpm-workspace.yaml`
marks where it ends — rather than as far as `packages/ui/`, since the app that
imports it is never underneath it. Read that way, `badge` is used, `pill`
beside it is not, and `tooltip.ts` is a file nobody imports.

A package that does declare `exports` resolves subpath by subpath, preferring
the source conditions (`types`, `development`, `source`) over `./dist/…`,
which a repository does not contain.

## Frameworks

A framework is a second program that starts the project's files: it lists a
directory and turns what it finds into routes and plugins. No `import` records
that, so basta has to know which framework is at work and what it loads. Both
are data — [`frameworks.yaml`](../../rust/crates/basta/frameworks.yaml), a
table of some fifty frameworks compiled into the binary:

```bash
basta --list-frameworks
# vite           vite.config.{js,mjs,cjs,ts,mts,cts}, dependency vite
# next           next.config.{js,mjs,cjs,ts,mts}, dependency next
# jest           jest.config.{…}, dependency jest, package.json "jest"
# …
```

Any one signal detects a framework: its config file by name, the package among
the dependencies of `package.json`, or its section in `package.json`. Detection
happens per directory, so each package of a monorepo is its own project, and
what the framework starts is anchored at the directory it was found in.

A project is rarely one framework. A router, a plugin loader, a test runner and
a component workshop routinely share one `package.json`, so every framework
whose signal matches is in force at once, and each roots its own files —
nothing picks a winner.

### Ones basta knows

`frameworks/detected/` is a Fastify server with no framework config file at
all, run by two frameworks at the same time. `server.js` hands two directories
to `@fastify/autoload`, which lists them at startup, and Vitest loads
`vitest.setup.js` before every suite. The dependencies in `package.json` are
the only sign of either.

```bash
basta fixtures/dead-code-demo/frameworks/detected --no-colors
# Frameworks: fastify-autoload, vitest
# Unused files (1)
#  - lib/retired-rate-card.js  certain 95%
# Found 1 dead code findings in 6 files (15.5% of 71 lines).
```

Both plugins are alive because the loader reaches them, and the setup file
because the runner does. Turn detection off and all three are reported as
confidently as the file that really is dead:

```bash
basta fixtures/dead-code-demo/frameworks/detected --no-frameworks --no-colors
# Unused files (4)
#  - lib/retired-rate-card.js  certain 95%
#  - plugins/depot-db.js  certain 95%
#  - plugins/dispatch-mailer.js  certain 95%
#  - vitest.setup.js  certain 95%
# Found 4 dead code findings in 6 files (64.8% of 71 lines).
```

`routes/parcels.js` survives either way: a `routes/` directory is a convention
of the language, not of one framework.

### One the project describes

`frameworks/custom/` runs on an in-house router that mounts every
`screens/**/*.screen.js`. basta has never heard of it:

```bash
basta fixtures/dead-code-demo/frameworks/custom --no-colors
# Unused files (3)
#  - screens/account/loyalty.screen.js  certain 95%
#  - screens/checkout/basket.screen.js  certain 95%
#  - screens/checkout/totals.js  certain 95%
# Found 3 dead code findings in 5 files (68.6% of 51 lines).
```

The project says so once, in the same shape as the built-in table —
`basta.frameworks.yaml` (or `.yml`, `.json`), detected here by the
`kioskRouter` section of its `package.json`:

```yaml
frameworks:
  - name: kiosk-router
    detect:
      packageJsonKeys: [kioskRouter]
    variables:
      screensDir: screens
    entry:
      - "${screensDir}/**/*.screen.js"
    globals:
      - names: [guard]
        files: ["${screensDir}/**/*.screen.js"]
```

```bash
basta fixtures/dead-code-demo/frameworks/custom --no-colors \
  --frameworks-config fixtures/dead-code-demo/frameworks/custom/basta.frameworks.yaml
# Frameworks: kiosk-router
# Unused exports (1)
#  - function screens/checkout/totals.js:5:17 splitBetween  high 85%
# Found 1 dead code findings in 5 files (5.9% of 51 lines).
```

With the screens rooted, `totals.js` is reached through the basket, and what is
left is the one export nothing calls. The file is picked up without the flag
when it sits in the working directory:

```bash
(cd fixtures/dead-code-demo/frameworks/custom && basta . --no-colors)
# Found 1 dead code findings in 5 files (5.9% of 51 lines).
```

### Names a framework reads

A framework does not only load files, it looks names up in them: Next calls a
page's `getServerSideProps`, Remix a route's `loader`, Angular a component's
`ngOnInit`. No code in the project mentions those names, so a definition lists
them under `globals`, and a declaration under one of them is used — never
reported, and whatever it calls stays alive. A bare name holds in every file of
the project; `names` with `files` ties them to the files the framework reads
them from, because `loader` is Remix's word in a route and anybody's word
everywhere else.

The kiosk router asks a screen's `guard` before mounting it.
`loyalty.screen.js` exports one, beside a `legacyPromoCode` nothing has called
since the promotion ended. A screen is an entry point, so its exports are
reported only on request — and then the framework's name is not among them:

```bash
(cd fixtures/dead-code-demo/frameworks/custom && \
  basta . --include-entry-exports --min-confidence 0 --no-colors)
# Unused exports (2)
#  - function screens/account/loyalty.screen.js:9:17 legacyPromoCode  low 25%
#  - function screens/checkout/totals.js:5:17 splitBetween  high 85%
# Found 2 dead code findings in 5 files (11.8% of 51 lines).
```

Take the `globals` block out of `basta.frameworks.yaml` and `guard` is reported
next to `legacyPromoCode`, as if the two were the same kind of leftover.

A project's own definitions sit beside the built-in ones rather than instead
of them, so a described framework and detected ones work together. A
definition that carries the name of a built-in framework replaces it, and
`--framework <name>` — repeatable — takes one as present when the scan starts below the
`package.json` that would have named it (`basta src --framework next`).

## In the config file

What a project always wants does not belong on every command line. A jscpd
config takes a dead-code section — `deadCode`, `dead-code` or `basta`, the same
key — and `config/.jscpd.json` has one:

```json
{
  "threshold": 10,
  "deadCode": {
    "minConfidence": 90,
    "threshold": 40,
    "entry": ["tools/*.js"],
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

Without it the label station reads as mostly dead: a job the in-house runner
starts, a tool run by hand, a module for printers that went back to the lessor,
and an export nothing calls. (Config lookup is by working directory, so from the
repository root this project's own file is not the one found.)

```bash
jscpd --dead-code fixtures/dead-code-demo/config --no-colors
# Unused files (3)
#  - jobs/nightly-reprint.job.js  certain 95%
#  - src/legacy-zpl.js  certain 95%
#  - tools/seed-labels.js  certain 95%
# Unused exports (1)
#  - function src/print-queue.js:9:17 drainQueue  high 85%
# Found 4 dead code findings in 5 files (64.0% of 50 lines).
```

With it, the inline definition roots the job, `entry` roots the tool, and the
confidence floor of 90 leaves only what basta is sure of:

```bash
jscpd --dead-code fixtures/dead-code-demo/config --no-colors \
  --config fixtures/dead-code-demo/config/.jscpd.json
# Frameworks: job-runner
# Unused files (1)
#  - src/legacy-zpl.js  certain 95%
# Found 1 dead code findings in 5 files (14.0% of 50 lines).
```

That run exits 0: 14% is over the top-level `threshold` of 10, which is the
budget for duplicated lines, and under the section's own 40. A flag beats the
section — it replaces what the file says for one run:

```bash
jscpd --dead-code fixtures/dead-code-demo/config --no-colors --min-confidence 60 \
  --config fixtures/dead-code-demo/config/.jscpd.json
# Unused files (1)
#  - src/legacy-zpl.js  certain 95%
# Unused exports (1)
#  - function src/print-queue.js:9:17 drainQueue  high 85%
# Found 2 dead code findings in 5 files (22.0% of 50 lines).
```

The section configures the mode without switching it on — a plain `jscpd` in
that directory still looks for clones, unless the section says
`"enabled": true`. The `basta` binary reads the same section from the same
file, found in the working directory or named with `--config`:

```bash
(cd fixtures/dead-code-demo/config && basta . --no-colors)
# Frameworks: job-runner
# Found 1 dead code findings in 5 files (14.0% of 50 lines).
```

## Confidence

Every finding carries a score and, when it is below 100, the reasons it might
be wrong. Raise the floor to see only what basta is sure of:

```bash
jscpd --dead-code fixtures/dead-code-demo --min-confidence 90 --no-colors
# Found 19 dead code findings in 46 files (26.6% of 444 lines).
```

The seven exported names drop out — they are the findings a caller outside the
scan could invalidate.

## Whole directory

```bash
jscpd --dead-code fixtures/dead-code-demo --no-colors
# Found 26 dead code findings in 46 files (31.1% of 444 lines).
```

The eight projects do not interfere with each other: basta resolves imports
within each project's own entry points, and each alias, package name and
framework only within the project that declares it, so scanning them together
reports exactly the union of scanning them apart. (From the repository root
`frameworks/custom/` contributes its three undescribed screens and `config/`
all four of its findings: a definitions file and a jscpd config are looked for
in the working directory, not in every project scanned.)

## The standalone binary

Everything above works the same through `basta`, which is the same engine
without the duplication half of jscpd:

```bash
basta fixtures/dead-code-demo --no-colors
```
