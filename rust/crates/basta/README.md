# basta

Find dead code: unused files, exports, declarations and imports across
JavaScript, TypeScript and Python.

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

Every finding is the answer to *nothing reaches this*, so the entry points are
the whole basis of the result. basta finds them from `package.json`
(`main`, `module`, `bin`, `exports`, `files`, `scripts`), `pyproject.toml`
(`[project.scripts]` and entry-point tables), scripts in the tree (`*.sh`,
CI workflows, Makefiles, Dockerfiles) that name a source file, and
conventions: `src/index.ts`,
`__main__.py`, `manage.py`, framework routes under `pages/` and `app/`,
`*.config.ts`, `.d.ts` declarations, shebangs, `if __name__ == "__main__"`,
and every `__init__.py`.

When a project does something none of that covers, say so once:

```bash
basta src --entry 'src/handlers/**' --entry 'scripts/*.ts'
```

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
