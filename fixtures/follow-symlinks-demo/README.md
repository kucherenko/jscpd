# follow-symlinks demo

Shows how `--follow-symlinks` names and counts files reached through
symbolic links (issue #1059). All commands run from `root/` at default
thresholds except `--min-tokens 20`, which the short sample needs.

| Path | Contents |
|------|----------|
| `root/candidate/app.js` | A small function |
| `outside/S1/app.js` | A byte-for-byte copy, outside the scan root |
| `root/corpus -> ../outside` | Directory symlink that brings the copy into the scan root |
| `root/linked.js -> candidate/app.js` | File symlink next to its own target |

## Links are skipped by default

Without the flag the walker neither descends `corpus` nor reads `linked.js`,
so the scan sees one file (jscpd v4 followed links unless `--noSymlinks` was
set; v5 follows them only on request):

```bash
cd fixtures/follow-symlinks-demo/root
jscpd --min-tokens 20 .
# Duplications detection: Found 0 exact clones with 0(0.00%) duplicated lines in 1 (1 formats) files.
```

## A followed file keeps the path it was found at

With `--follow-symlinks` the copy is reported as `corpus/S1/app.js`, the name
the walker used, relative to the scan root like every other file. `linked.js`
resolves to `candidate/app.js`, which is already in the scan, so it is neither
counted a second time nor reported as a clone of itself:

```bash
jscpd --min-tokens 20 --follow-symlinks .
# Clone found (javascript)
#  - candidate/app.js [1:1 - 8:24] (8 lines, 56 tokens)
#    corpus/S1/app.js [1:1 - 8:24]
# ...
# Duplications detection: Found 1 exact clones with 7(43.75%) duplicated lines in 2 (1 formats) files.
```

`--absolute` prints the same names under the scan root
(`.../follow-symlinks-demo/root/corpus/S1/app.js`), not the link target.

## `--ignore` matches the reported name

Because the report and the ignore glob see the same path, excluding what the
report shows works as expected:

```bash
jscpd --min-tokens 20 --follow-symlinks --ignore 'corpus/**' .
# Duplications detection: Found 0 exact clones with 0(0.00%) duplicated lines in 1 (1 formats) files.
```

`--ignore '**/outside/**'` changes nothing: `outside` is the link target, a
name the scan never uses.

Before the fix (jscpd 5.2.0 and earlier) the same scan reported the copy by
its absolute link target (`/.../outside/S1/app.js`), counted `linked.js` as a
third source, and with only the file link in place listed `candidate/app.js`
as a clone of itself.
