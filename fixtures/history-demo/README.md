# history demo

`--history` scans every commit in a git range with the run's own configuration
and prints the duplication trend: a sparkline, one table row per commit, the
change between points, and, with `--threshold`, how far the threshold could be
tightened. A trend needs commits, so this demo builds its own repository in a
temporary directory instead of shipping files; run it from the repository root
at default thresholds.

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

```bash
jscpd src --history-since 2026-01-01 --no-colors
# Found 1 clones.
#
# History (since 2026-01-01: 4 commits + working tree)
#   ▁▆█▆▆  min 0.0%  max 58.1%  now 42.9%
#   COMMIT   DATE        FILES  LINES  CLONES  DUP LINES   DUP%  CHANGE  SUBJECT
#   <sha>    2026-08-01      1     11       0          0   0.0%          initial helpers
#   <sha>    2026-08-08      2     21       1          9  42.9%   +42.9  copy total() into b.js
#   <sha>    2026-08-15      3     31       2         18  58.1%   +15.2  and again into c.js
#   <sha>    2026-08-22      2     21       1          9  42.9%   -15.2  b.js imports total() instead
#   working  <today>         2     21       1          9  42.9%       =  (uncommitted changes)
# Trend: +42.9 points since <sha> (2026-08-01)
```

Commit hashes differ per machine because the author date is fixed but the
author is yours; everything else is identical. With colors on, `+` changes are
red and `-` changes green.

## Threshold headroom instead of an automatic ratchet

```bash
jscpd src --history HEAD~3..HEAD --threshold 50 --no-colors
# History (HEAD~3..HEAD: 3 commits + working tree)
#   ...
# Threshold 50.0% has 7.1 points of headroom: the series never needed it, tighten it with --threshold 42.9
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

## JSON

```bash
jscpd src --history-since 2026-01-01 --reporters json --output report
# report/jscpd-report.json gains "history": { "range", "threshold", "points": [...] }
```

Clean up with `cd - && rm -rf "$demo"`.
