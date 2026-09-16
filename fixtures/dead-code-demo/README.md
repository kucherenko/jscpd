# Dead code demo

Two small projects — one TypeScript, one Python — that contain one example of
every finding `basta` reports. Commands run from the repository root at
default settings, with no threshold or category flags, so they are the ones a
user would actually type.

Each project is self-contained: scanning just that directory shows the whole
effect, and scanning both together reports the same nine findings.

| Directory     | Language           | Findings                                          |
| ------------- | ------------------ | ------------------------------------------------- |
| `typescript/` | TypeScript, ESM    | 1 unused file, 1 unused export, 1 unused symbol, 1 unused import |
| `python/`     | Python package     | 1 unused file, 1 unused export, 2 unused symbols, 1 unused import |

## What an entry point is

Nothing here is marked dead because it "looks unused" — every finding is the
answer to *no entry point reaches this*. The two projects declare their entry
points the way real projects do:

- `typescript/package.json` sets `"main": "./src/index.ts"`.
- `python/shop/__main__.py` is what `python -m shop` runs, and
  `python/shop/__init__.py` is the package's public surface.

Delete either declaration and the whole project reads as dead, which is the
failure mode the entry-point rules exist to prevent.

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

## Confidence

Every finding carries a score and, when it is below 100, the reasons it might
be wrong. Raise the floor to see only what basta is sure of:

```bash
jscpd --dead-code fixtures/dead-code-demo --min-confidence 90 --no-colors
# Found 7 dead code findings in 9 files (27.7% of 112 lines).
```

The two exported functions drop out — they are the findings a caller outside
the scan could invalidate.

## Whole directory

```bash
jscpd --dead-code fixtures/dead-code-demo --no-colors
# Found 9 dead code findings in 9 files (33.0% of 112 lines).
```

The two projects do not interfere with each other: basta resolves imports
within each project's own entry points, so scanning them together reports
exactly the union of scanning them apart.

## The standalone binary

Everything above works the same through `basta`, which is the same engine
without the duplication half of jscpd:

```bash
basta fixtures/dead-code-demo --no-colors
```
