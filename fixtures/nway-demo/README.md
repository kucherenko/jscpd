# N-way copies

The same block in three or more places is reported as pairs anchored on the
copy that was scanned first: with the block in `a`, `b` and `c` the report
holds `a ↔ b` and `a ↔ c`. This directory holds the shapes where that pairing
used to go wrong. Commands run from the repository root at default
thresholds; the `Found N clones.` line is the console reporter's.

| Directory | Shape | Expected |
|-----------|-------|----------|
| `lost-pair/` | a copy, two renamed copies, a copy followed by more code | 3 clones |

## `lost-pair/` — a third file must not steal the anchor

`duration.js` holds `parseDuration`. `timeouts.js` holds the same body twice
under other names, so its windows around the second `export function` are
stored without matching anything. `uptime.js` holds `parseDuration` followed
by another function. While `uptime.js` is scanned, the clone anchored on
`duration.js` covers the body, and the next window (`… } export function`)
matches the window `timeouts.js` stored earlier. Extending on that stored
window would stretch the anchored fragment past the end of `duration.js`, and
the clone was silently dropped ([#1033](https://github.com/kucherenko/jscpd/issues/1033)).
A clone now grows only while its own anchor keeps matching, so all three
pairs are reported and each covers identical tokens on both sides.

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
# Found 3 clones.
```

The two `timeouts.js` pairs start after the function name because the names
differ; with `--ignore-identifiers` they are reported from line 1 as
`renamed`, and a fourth `renamed` pair appears for the `… } export function`
tail shared by `timeouts.js` and `uptime.js` (`Found 4 clones.`). Scan order
decides the anchor, not whether a pair is found: with the files renamed so
that `uptime.js` is scanned first, the report still holds three pairs, now
anchored on `uptime.js`.
