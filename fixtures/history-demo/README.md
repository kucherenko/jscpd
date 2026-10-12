# history demo

`--history` scans every commit in a git range with the run's own configuration
and prints the trend of duplication and of the health score: a bar chart of
each, one table row per commit, the change between points, the score of every
health dimension at both ends of the series, and, with `--threshold`, how far
the threshold could be tightened. A trend needs commits, so this demo builds
its own repository in a temporary directory instead of shipping files; run it
from the repository root at default thresholds.

| Step | Commit | Files | Clones |
|------|--------|-------|--------|
| 1 | `initial helpers` | `a.js` | 0 |
| 2 | `copy total() into b.js` | `a.js`, `b.js` | 1 |
| 3 | `and again into c.js` | `a.js`, `b.js`, `c.js` | 2 |
| 4 | `b.js imports total() instead` | `b.js` rewritten | 1 |

## Build the repository

```bash
demo=$(mktemp -d) && cd "$demo" && git init -q && mkdir src
git config user.email demo@example.com && git config user.name demo && git config commit.gpgsign false
fn() { printf 'export function %s(a, b, c) {\n  const first = a * b + c;\n  const second = first - a / b;\n  const third = second + c * c;\n  const fourth = third - first + a;\n  const fifth = fourth * second - b;\n  console.log(first, second, third, fourth, fifth);\n  return [first, second, third, fourth, fifth];\n}\n' "$1"; }
one() { printf 'export const %s = (n) => n * %d + %d;\n' "$1" "$2" "$3"; }
snap() { git add -A && GIT_COMMITTER_DATE="$2" git commit -q -m "$1" --date="$2"; }
{ fn total; one tax 3 1; one fee 5 2; } > src/a.js && snap 'initial helpers' 2026-08-01T10:00:00
{ fn total; one rate 7 3; } > src/b.js && snap 'copy total() into b.js' 2026-08-08T10:00:00
{ fn total; one discount 9 4; } > src/c.js && snap 'and again into c.js' 2026-08-15T10:00:00
{ one rate 7 3; one rate2 11 5; } > src/b.js && snap 'b.js imports total() instead' 2026-08-22T10:00:00
```

## Trend over every commit

`--history` without a range walks every commit of the current branch, and
`--history-limit` (30 by default) samples a longer history evenly. A range
(`--history HEAD~3..HEAD`) or a date (`--history-since 2026-01-01`) narrows
it.

```bash
jscpd src --history --no-colors
# Found 1 clones.
#
# History (HEAD: 4 commits + working tree)
#   duplicated lines, % of all lines: min 0.0%  max 64.5%  now 47.6%
#     64.5% ┤       ██
#           │       ██
#           │    ▇▇ ██ ▇▇ ▇▇
#     32.3% ┤    ██ ██ ██ ██
#           │    ██ ██ ██ ██
#           │    ██ ██ ██ ██
#           │    ██ ██ ██ ██
#      0.0% ┤ ▁▁ ██ ██ ██ ██
#           └────────────────
#             1  2  3  4  5
#
#   health, 0-100: min 70  max 74  now 72 B
#        74 ┤ ██
#           │ ██
#           │ ██
#        72 ┤ ██
#           │ ██ ▇▇    ▄▄ ▄▄
#           │ ██ ██    ██ ██
#           │ ██ ██    ██ ██
#        70 ┤ ██ ██ ▁▁ ██ ██
#           └────────────────
#             1  2  3  4  5
#
#   #  COMMIT   DATE        FILES  LINES  CLONES  DUP LINES   DUP%  CHANGE  HEALTH  CHANGE  SUBJECT
#   1  <sha>    2026-08-01      1     11       0          0   0.0%            74 B          initial helpers
#   2  <sha>    2026-08-08      2     21       1         10  47.6%   +47.6    72 B      -2  copy total() into b.js
#   3  <sha>    2026-08-15      3     31       2         20  64.5%   +16.9    70 B      -2  and again into c.js
#   4  <sha>    2026-08-22      2     21       1         10  47.6%   -16.9    72 B      +2  b.js imports total() instead
#   5  working  <today>         2     21       1         10  47.6%       =    72 B       =  (uncommitted changes)
# Trend: +47.6 points since <sha> (2026-08-01)
# Health: -2 since <sha> (2026-08-01), 74 B → 72 B: duplication 75 → 72, dead code 72 → 68, complexity 75 → 75
```

Commit hashes differ per machine because the author date is fixed but the
author is yours; everything else is identical. Each chart's y-axis spans the
series' own min and max, the numbers under it are the `#` column of the table,
and with colors on, a change for the worse is red and one for the better green:
more duplication, or a lower health score.

Each point is scored as `jscpd src --health` scores the same tree, so the last
row reads 72 B, as the badge does. Thirty lines barely move the score, since
every share is mixed with 2000 lines of a typical project before it is scored;
in a real project the `Health:` line shows which dimension moved it.

## Threshold headroom instead of an automatic ratchet

```bash
jscpd src --history HEAD~3..HEAD --threshold 50 --no-colors
# History (HEAD~3..HEAD: 3 commits + working tree)
#   ...
# Threshold 50.0% has 2.4 points of headroom: the series never needed it, tighten it with --threshold 47.6
```

When the latest value is over the threshold the usual threshold error fires
instead and the run exits 1.

## Thinning a long series

```bash
jscpd src --history-since 2026-01-01 --history-every 2 --no-colors   # every 2nd commit, newest kept
# History (since 2026-01-01: 2 commits + working tree)
jscpd src --history-since 2026-01-01 --history-limit 2 --no-colors   # first and last only
# History (since 2026-01-01: 2 commits + working tree)
```

## The health badge and the dashboard

```bash
jscpd src --health --history HEAD~1..HEAD --no-colors
# Health  B   72/100  █████████████████▎░░░░░░  21 lines of code (XS)
#   duplication   72  ████████▊░░░  47.6% in javascript
#   dead code     68  ████████▏░░░  100.0%
#   complexity    75  █████████░░░  0.0% in complex files
#
# History (HEAD~1..HEAD: 1 commit + working tree)
#   ...
jscpd src --dashboard --history-since 2026-01-01 --reporters console,markdown,html --output report
# the series is the last section of the dashboard, and of report/jscpd-dashboard.md and .html
```

## JSON and the ai reporter

```bash
jscpd src --history-since 2026-01-01 --reporters json --output report
# report/jscpd-report.json gains "history": { "range", "threshold", "points": [...] },
# and every point a "health": { "score", "grade", "codeLines", "dimensions": [{ "id", "value", "score" }] }
jscpd src --history-since 2026-01-01 --reporters ai
# history since 2026-01-01 (commit/date/files/clones/dup%/health): ▁▆█▆▆
# health █▄▁▄▄ -2 since <sha> (2026-08-01), 74 B -> 72 B: duplication 75 -> 72, dead code 72 -> 68, complexity 75 -> 75
# <sha> 2026-08-01 1/0/0.0/74
# <sha> 2026-08-08 2/1/47.6/72 +47.6
# ...
```

Clean up with `cd - && rm -rf "$demo"`.
