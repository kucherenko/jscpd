# Nested scan paths

A path passed to jscpd can lie inside another one: `jscpd . src`, or a
config file listing `src` next to `src/generated`. jscpd scans the files of
the inner path once. Version 5.3.2 scanned them once per path and reported
each such file as a clone of itself; version 4 scanned them once.

Commands run from the repository root at default thresholds.

| Directory | What it holds | `jscpd` on the demo | With `app/lib` as a second path |
|-----------|---------------|---------------------|---------------------------------|
| `app/` | a monthly report, and in `app/lib/` the money helpers it uses | 0 clones, 2 files | 0 clones, 2 files |

## Commands

```bash
jscpd fixtures/nested-paths-demo
# Found 0 clones.

jscpd fixtures/nested-paths-demo fixtures/nested-paths-demo/app/lib
# Found 0 clones.
```

For the second command 5.3.2 counted three files and paired `money.py` with
itself:

```
Clone found (python)
 - app/lib/money.py [1:1 - 22:66] (22 lines, 156 tokens)
   app/lib/money.py [1:1 - 22:66]
Found 1 clones.
```

`--semantic` reads each function once as well, so no function pairs with
itself.
