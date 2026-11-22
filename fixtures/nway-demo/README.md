# N-way copies

The same block in three or more places is reported as pairs anchored on the
copy that was scanned first: with the block in `a`, `b` and `c` the report
holds `a ↔ b` and `a ↔ c`. This directory holds the shapes where that pairing
used to go wrong. Commands run from the repository root at default
thresholds; the `Found N clones.` line is the console reporter's.

| Directory | Shape | Expected |
|-----------|-------|----------|
| `lost-pair/` | a copy, two renamed copies, a copy followed by more code | 4 clones |
| `stolen-head/` | two copies of a table, a third file with its header alone | 1 clone, from line 1 |

## `lost-pair/` — a third file must not steal the anchor

`duration.js` holds `parseDuration`. `timeouts.js` holds the same body twice
under other names, so its windows around the second `export function` are
stored without matching anything. `uptime.js` holds `parseDuration` followed
by another function. While `uptime.js` is scanned, the clone anchored on
`duration.js` covers the body, and the next window (`… } export function`)
matches the window `timeouts.js` stored earlier. Extending on that stored
window would stretch the anchored fragment past the end of `duration.js`, and
the clone was silently dropped ([#1033](https://github.com/kucherenko/jscpd/issues/1033)).
A clone now grows only while its own anchor keeps matching, so the three
pairs with `duration.js` are reported and each covers identical tokens on
both sides.

```bash
jscpd fixtures/nway-demo/lost-pair
# Clone found (javascript)
#  - duration.js [1:30 - 9:2] (9 lines, 107 tokens)
#    timeouts.js [1:29 - 9:2]
# Clone found (javascript)
#  - duration.js [1:30 - 9:2] (9 lines, 107 tokens)
#    timeouts.js [11:30 - 19:2]
# Clone found (javascript)
#  - duration.js [1:1 - 9:2] (9 lines, 110 tokens)
#    uptime.js [1:1 - 9:2]
# Clone found (javascript)
#  - timeouts.js [1:29 - 11:16] (11 lines, 109 tokens)
#    uptime.js [1:30 - 11:16]
# Found 4 clones.
```

The two `duration.js ↔ timeouts.js` pairs start after the function name
because the names differ. The fourth pair is the first function of
`timeouts.js` against `uptime.js`: both go on with `export function` after
the body, two tokens that `duration.js` lacks. That pair was found by the
window with those two tokens and now starts where the two files start to
agree (see `stolen-head/` below); before, it was a few tokens on one line
and fell under `--min-lines`. Scanned alone, the two files give the same
pair.

`--ignore-identifiers` and `--ignore-identifiers=blind` report the same four
pairs, and the pairs where only the names differ start at line 1 as
`renamed` (`Found 4 clones.`).

## `stolen-head/` — a third file must not move the start of a clone

`orders.md` and `users.md` hold the same table. `catalog.md` ends with the
header of that table and shares nothing else with them. It is scanned first,
so the windows of the header are stored for it. `orders.md` matches them
there and stores only the windows `catalog.md` lacks, the first of which
starts in the middle of the second header line. The clone of `orders.md`
and `users.md` used to open on that window, so it was reported from 2:32:
its start depended on a file that is no part of it, and a baseline saved
without `catalog.md` took the clone for a new one
([#1319](https://github.com/kucherenko/jscpd/issues/1319)). A clone now
reaches back from the window it was found by to the first token the two
files share.

```bash
jscpd fixtures/nway-demo/stolen-head --min-tokens 30
# Clone found (markdown)
#  - orders.md:markdown [1:1 - 7:15] (7 lines, 133 tokens)
#    users.md:markdown [1:1 - 7:15]
# Found 1 clones.
```

The header is shorter than the default `--min-tokens 50`, so at the default
no window lies in it and the clone starts at line 1 either way. The two
header-only pairs with `catalog.md` span two lines and fall under
`--min-lines`; `--min-lines 1` shows them (`Found 3 clones.`).
