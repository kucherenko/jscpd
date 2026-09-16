# basta — dead code detector

[![npm version](https://img.shields.io/npm/v/basta.svg)](https://www.npmjs.com/package/basta)
[![license](https://img.shields.io/npm/l/basta.svg)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)

Finds code nothing runs: unused files, exports, module-private declarations
and imports across **JavaScript, TypeScript, JSX, TSX and Python**.

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
literal. `--min-confidence` sets the floor; the default is 60.

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
