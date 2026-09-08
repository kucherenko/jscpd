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
| `long-line/` | 1 line of 300 tokens | 2 exact clones | 2 exact clones | 2 exact clones |
| `renamed-halves/` | 1 line, names differ | 0 clones | see below | see below |

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

The statistics table under the clone counts the matched lines only: the
merged clone spans 15 lines of `place-order-guarded.js`, but its two gap
lines are not duplicated code, so `Duplicated lines` reads `12`, not `14`,
and `--threshold` does not get stricter because merging is on.

## `long-line/` — a gap wider than the match is not a near-miss

`theme-branded.js` is `theme-default.js` with one line inserted: a 150-entry
colour array, about 300 tokens on a single line. Measured in lines the gap is
1, so `--max-gap-lines 1` would join the halves; measured in tokens the
merged span would be `498` tokens of which at most `195` match, a similarity
under `0.4`. A merge whose similarity would fall below `0.5` is refused and
the halves stay separate, whatever the line limit.

```bash
jscpd fixtures/type3-demo/long-line
# Clone found (javascript)
#  - theme-branded.js [1:1 - 8:8] (8 lines, 78 tokens)
#    theme-default.js [1:1 - 8:8]
# Clone found (javascript)
#  - theme-branded.js [8:1670 - 15:2] (8 lines, 117 tokens)
#    theme-default.js [7:72 - 14:2]
# Found 2 clones.

jscpd fixtures/type3-demo/long-line --max-gap-lines 1
# Found 2 clones.

jscpd fixtures/type3-demo/long-line --max-gap-lines 5
# Found 2 clones.
```

## `renamed-halves/` — `similar` takes precedence over `renamed`

`sync-leads.js` is `sync-contacts.js` with every identifier renamed and one
early-return line inserted. A default scan sees nothing; `--ignore-identifiers`
finds the two halves as `renamed` clones; adding `--max-gap-lines 1` merges
them, and the result is reported as `similar`, not `renamed`: a merged clone
is no longer identical even after normalization, so `similar` wins.

```bash
jscpd fixtures/type3-demo/renamed-halves
# Found 0 clones.

jscpd fixtures/type3-demo/renamed-halves --ignore-identifiers
# Clone found (javascript, renamed)
#  - sync-contacts.js [1:1 - 6:80] (6 lines, 102 tokens)
#    sync-leads.js [1:1 - 6:74]
# Clone found (javascript, renamed)
#  - sync-contacts.js [6:79 - 13:2] (8 lines, 78 tokens)
#    sync-leads.js [7:61 - 14:2]
# Found 2 clones.

jscpd fixtures/type3-demo/renamed-halves --ignore-identifiers --max-gap-lines 1
# Clone found (javascript, similar (gap) ~0.91)
#  - sync-contacts.js [1:1 - 13:2] (13 lines, 179 tokens)
#    sync-leads.js [1:1 - 14:2]
# Found 1 clones.
```

The same pair is also a case for `--similarity`, which ignores names by
construction: `jscpd fixtures/type3-demo/renamed-halves --similarity 0.85`
reports one `similar (ast) ~0.88` clone without any Type-2 flag.

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
# Found 6 clones.   (two exact halves each in inserted-line/, wide-gap/ and long-line/)

jscpd fixtures/type3-demo --max-gap-lines 2
# Found 4 clones.   (inserted-line/ and wide-gap/ merge; long-line/ is refused and keeps its halves)

jscpd fixtures/type3-demo --similarity 0.7
# Found 10 clones.  (the six exact halves, plus one similar function pair in every
#                    directory except long-line/, whose inserted array changes the structure too much)

jscpd fixtures/type3-demo --max-gap-lines 2 --similarity 0.7
# Found 6 clones.   (two merged, two refused halves, and the function pairs of
#                    renamed-halves/ and similar-functions/; merged clones already
#                    cover the whole functions, so those pairs are not reported again)

jscpd fixtures/type3-demo --max-gap-lines 2 --similarity 0.7 --reporters json,silent --output report
# every merged or function-level entry in "duplicates" has "kind": "similar", a
# "similarity" value and "method": "gap" or "ast" — the two scores are not on
# the same scale; the long-line/ halves keep "kind": "exact"
```

In SARIF these clones use the rule `jscpd/similar-code` (renamed clones use
`jscpd/renamed-code`, exact ones `jscpd/duplicate-code`); Code Climate uses
the same names as `check_name`.

Merging only joins clones the exact run already reported, so it never adds a
match that was not there. Merged spans differ from the exact fragments, so a
run with `--max-gap-lines` needs its own `--baseline` file.
