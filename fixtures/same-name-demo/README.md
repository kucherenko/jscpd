# Files of one name in two scan paths

`billing/rates.js` and `returns/rates.js` hold the same rate code, and `billing/invoice.js` uses it. When the two folders are passed as two scan paths, the report names each file by its path from the folder that holds both paths, so the two `rates.js` stay apart.

Up to jscpd 5.4.3 a file was named from its own scan path, and both files showed as `rates.js` (issue #1324).

Run the commands from the repository root. They use the default thresholds.

| Command | JavaScript files | Clones | First file | Second file |
|---------|------------------|--------|------------|-------------|
| `billing` and `returns` as two paths | 3 | 1 | `billing/rates.js` | `returns/rates.js` |
| the whole demo as one path | 3 | 1 | `billing/rates.js` | `returns/rates.js` |

The scan of the whole demo also reads this README, so it counts 4 files.

## Two paths

```bash
jscpd fixtures/same-name-demo/billing fixtures/same-name-demo/returns
# Clone found (javascript)
#  - billing/rates.js [3:1 - 20:2] (18 lines, 142 tokens)
#    returns/rates.js [3:1 - 20:2]
```

5.4.3 printed the same clone as:

```
 - rates.js [3:1 - 20:2] (18 lines, 142 tokens)
   rates.js [3:1 - 20:2]
```

A single path is shown as before: its files are named from that path. With `--absolute` every file is named by its absolute path, in both versions.
