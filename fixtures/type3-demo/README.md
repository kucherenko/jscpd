# Type-3 clones (near-miss)

A near-miss clone is a copy with a few lines inserted, removed or changed in
the middle. A token-window detector sees it as two shorter exact clones with
a gap between them. `--max-gap-lines N` (config key `maxGapLines`, off at
`0`) merges clones of the same file pair whose fragments follow each other in
both files with at most `N` unmatched lines between them, and reports one
clone of kind `similar` with a `similarity` value (matched tokens over the
tokens of the longer merged span).

Commands run from the repository root at default thresholds; the lines after
`#` are what the console reporter prints.

| Directory | Gap | Default scan | `--max-gap-lines 1` | `--max-gap-lines 2` |
|-----------|-----|--------------|---------------------|---------------------|
| `inserted-line/` | 1 line | 2 exact clones | 1 similar clone | 1 similar clone |
| `wide-gap/` | 2 lines | 2 exact clones | 2 exact clones | 1 similar clone |

`similar-functions/` demonstrates the second mechanism, `--similarity RATIO`,
which compares whole JavaScript/TypeScript functions by syntax-tree structure
instead of joining token runs (see below).

## `inserted-line/` — one inserted guard

`save-account.js` is `save-user.js` with one `if (...) throw` line added in
the middle.

```bash
jscpd fixtures/type3-demo/inserted-line
# Clone found (javascript)
#  - save-account.js [1:1 - 6:5] (6 lines, 60 tokens)
#    save-user.js [1:1 - 6:6]
# Clone found (javascript)
#  - save-account.js [6:66 - 12:2] (7 lines, 100 tokens)
#    save-user.js [5:55 - 11:2]
# Found 2 clones.

jscpd fixtures/type3-demo/inserted-line --max-gap-lines 1
# Clone found (javascript, similar (gap) ~0.91)
#  - save-account.js [1:1 - 12:2] (12 lines, 157 tokens)
#    save-user.js [1:1 - 11:2]
# Found 1 clones.
```

The two exact windows overlap by three tokens at the boundary (the end of
`save-user.js` line 5 is where the second window starts), so the merged
clone matches 157 tokens, not 160.

## `wide-gap/` — a three-line block, two lines of gap

`place-order-guarded.js` is `place-order.js` with a three-line `if` block
inserted. The first exact clone ends on the `if` line and the second starts
on the `const payment` line right after the block, leaving the two lines of
the block's body unmatched between them, so a limit of 1 keeps the halves
apart and a limit of 2 merges them.

```bash
jscpd fixtures/type3-demo/wide-gap --max-gap-lines 1
# Found 2 clones.

jscpd fixtures/type3-demo/wide-gap --max-gap-lines 2
# Clone found (javascript, similar (gap) ~0.91)
#  - place-order-guarded.js [1:1 - 15:2] (15 lines, 172 tokens)
#    place-order.js [1:1 - 12:2]
# Found 1 clones.
```

## `similar-functions/` — edits spread through a function

`credit-note.js` is `invoice.js` after a realistic second use: every name
changed, one `continue` guard and one logging call inserted. No two token
runs are long enough to clear the default thresholds, so neither a default
scan nor `--max-gap-lines` reports anything. `--similarity RATIO` compares
functions by the bag of 4-grams over their syntax-tree node types (names and
values do not take part), so the pair scores by how much structure survived.

```bash
jscpd fixtures/type3-demo/similar-functions
# Found 0 clones.

jscpd fixtures/type3-demo/similar-functions --max-gap-lines 3
# Found 0 clones.

jscpd fixtures/type3-demo/similar-functions --similarity 0.85
# Found 0 clones.

jscpd fixtures/type3-demo/similar-functions --similarity 0.7
# Clone found (javascript, similar (ast) ~0.75)
#  - credit-note.js [1:8 - 19:2] (19 lines, 126 tokens)
#    invoice.js [1:8 - 17:2]
# Found 1 clones.
```

`--similarity 1` is the default and means exact matches only: the pass does
not run. Calibration: a copy that only renames things scores `1.00` (try
`jscpd fixtures/type2-demo/identifiers --similarity 0.9`), a single inserted
line scores about `0.9`, and two inserted statements plus renames, as here,
score `0.75`. Functions must clear `--min-tokens` and `--min-lines` on their
own, and a pair already reported as an exact or merged clone is not reported
again.

## Whole directory

```bash
jscpd fixtures/type3-demo
# Found 4 clones.

jscpd fixtures/type3-demo --max-gap-lines 2
# Found 2 clones.

jscpd fixtures/type3-demo --similarity 0.7
# Found 7 clones.   (the four exact halves, plus one similar function pair per directory)

jscpd fixtures/type3-demo --max-gap-lines 2 --similarity 0.7
# Found 3 clones.   (merged clones cover the whole functions, so only similar-functions/ adds one)

jscpd fixtures/type3-demo --max-gap-lines 2 --similarity 0.7 --reporters json,silent --output report
# every entry in "duplicates" has "kind": "similar", a "similarity" value and
# "method": "gap" or "ast" — the two scores are not on the same scale
```

Merging only joins clones the exact run already reported, so it never adds a
match that was not there. Merged spans differ from the exact fragments, so a
run with `--max-gap-lines` needs its own `--baseline` file.
