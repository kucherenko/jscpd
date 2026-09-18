# Summary demo: complexity

Five short files, one per language, each written so its cyclomatic complexity
can be counted by hand. `--summary --summary-by complexity` ranks them, and the
`CX` column is meant to be that count: one path per function, plus one per
branch. Commands run from the repository root at default thresholds.

| File                  | What it exercises                                              | Paths |
| --------------------- | -------------------------------------------------------------- | ----- |
| `c/checkout.c`        | functions with no keyword to find them by, `&&`, `\|\|`, `case`  | 13    |
| `rust/status.rs`      | `match` arms, a match guard, a `'static` lifetime              | 9     |
| `swift/Profile.swift` | `String?`, `?.`, `??`, `guard`, a real ternary                 | 8     |
| `python/report.py`    | docstrings full of `if`, `or`, `for`; parameters named `case` and `when` | 5 |
| `typescript/form.ts`  | optional properties `city?:`, `?.`, `??`, an arrow function    | 4     |

## Ranking by complexity

```bash
jscpd fixtures/summary-demo --summary --summary-by complexity --no-colors --no-tips
# Found 0 clones.
#
# Summary (by complexity; 6 files, 6 folders analyzed)
# Top files:
#   TOKENS  LINES  SIZE  CX  DUP%  PATH
#      176     32   710  13   0.0  c/checkout.c
#      112     24   481   9   0.0  rust/status.rs
#       88     21   446   8   0.0  swift/Profile.swift
#      103     19   584   5   0.0  python/report.py
#       96     19   441   4   0.0  typescript/form.ts
#     1142     84  4.3K   0   0.0  README.md
```

Every `CX` value equals the hand count in the table above. This README is
scanned too, as markdown, and scores 0: the "if", "or" and "for" in its prose
are words, not branches. Data formats (JSON, YAML, TOML, lock files) score 0
for the same reason.

## Complexity without clone detection

`--complexity` prints the same tables without running clone detection, so it
is faster on a large codebase and has no `DUP%` column. `--summary-top` limits
the rows; `-r ai` prints the compact form and `-r json` writes
`jscpd-complexity.json`.

```bash
jscpd fixtures/summary-demo --complexity --summary-top 3 --no-colors --no-tips
# Complexity (by complexity; 6 files, 6 folders analyzed)
# Top files:
#   TOKENS  LINES  SIZE  CX  PATH
#      176     32   710  13  c/checkout.c
#      112     24   481   9  rust/status.rs
#       88     21   446   8  swift/Profile.swift
```

## How each file is counted

**`c/checkout.c` — 13.** Three functions, and C gives none of them a keyword:
`double line_total(…) {` is recognized by what precedes its open paren, which
is how it is told apart from the `) {` that ends `if (…) {` or `switch (…) {`.
`line_total` has `if`, `||`, `if`, `&&`; `shipping_zone` has three `case`
labels (the `default` is not a branch); `free_shipping` has `&&`, `||`, `&&`.
That is 3 + 4 + 3 + 3.

**`rust/status.rs` — 9.** A `match` has no keyword per arm, only the `=>`
each arm is written with. Every arm is counted and each `match` takes one back,
because N arms are N paths: `status_for` has five arms and a guard
(`Method::Post if authorized`), so 1 + 4 + 1 = 6; `describe` has three arms,
so 1 + 2 = 3. The `'static` lifetime is a lone quote that must not be mistaken
for the start of a string.

**`swift/Profile.swift` — 8.** In Swift a `?` is usually an optional, not a
ternary. `String?` and `manager?.nickname` are written against the token before
them and are not branches; `age >= 18 ? "adult" : "minor"` puts the `?` in the
open and is one. `??` is a branch, and so is `guard`. The three functions come
to 2 + 3 + 3.

**`python/report.py` — 5.** `summarize` has `for`, `if` and `elif`, so 4, and
`headline` has none, so 1. Everything else that looks like a branch is not one:
the docstring's body mentions `If`, `or`, `while`, `for` and `when` on the
lines after its opening quotes, and `headline(case, when)` uses two branch
keywords as parameter names. A name the file lists as a parameter, assigns to
or reads off an object is that file's own name, whatever another language
reserves.

**`typescript/form.ts` — 4.** `city?: string` is an optional property and
`customer.address?.city` an optional access; neither branches. The `??` in
`shippingLabel` and the `||` in the arrow function `hasContact` do: 2 + 2.

## Against lizard

[lizard](https://github.com/terryyin/lizard) agrees with every row except two,
and in both it is the reference that counts fewer paths than are there:

```bash
pip install lizard
lizard fixtures/summary-demo
```

| File                  | CX | lizard | Why they differ                                   |
| --------------------- | -- | ------ | ------------------------------------------------- |
| `c/checkout.c`        | 13 | 13     |                                                   |
| `rust/status.rs`      | 9  | 5      | lizard counts a `match` once, whatever its arms   |
| `swift/Profile.swift` | 8  | 5      | lizard does not count `??`                        |
| `python/report.py`    | 5  | 5      |                                                   |
| `typescript/form.ts`  | 4  | 4      |                                                   |
