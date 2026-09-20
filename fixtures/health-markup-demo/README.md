# Health markup exclusion

Two Vue components whose `<template>` blocks are identical and whose scripts
are not. Clone detection reports the shared markup — a repeated template is
a real duplicate — but the health score leaves it out: duplication in
markup, stylesheets, templates, prose and data files is not the maintenance
problem duplicated programming logic is, so it never touches the mark.
Commands run from the repository root at default thresholds; the
`Found N clones.` line is the console reporter's.

How single-file components are split into per-block languages is
[`fixtures/sfc-demo`](../sfc-demo/README.md)'s story; this demo is only about
what the health score does with the result.

| Directory   | What it shows                                                       | Default scan | With `--health`               |
|-------------|---------------------------------------------------------------------|--------------|-------------------------------|
| `summary/`  | a `MemberCard.vue` whose template is copied below                    | 1 html clone | duplication `0.0% (no markup/text)` |
| `roster/`   | the copy: same `<template>`, a different `<script lang="ts">` block  | —            | —                        |

## The shared template is a clone, and still does not count

Both files render the same member card, but they get there differently:
`summary/` toggles a skills list from a `ref`, `roster/` computes a ranking
from a vote count. Only the markup between them is duplicated, and the
default scan finds it as an `html` clone — the block's own language, named
after the file it sits in.

```bash
jscpd fixtures/health-markup-demo --no-colors --no-tips
# Clone found (html)
#  - roster/MemberCard.vue:html [17:3 - 33:13] (17 lines, 150 tokens)
#    summary/MemberCard.vue:html [16:3 - 32:13]
# Found 1 clones.
```

The health score reads the same run and skips that clone:

```bash
jscpd fixtures/health-markup-demo --health --no-colors --no-tips
# Health  E   36/100  ████████▋░░░░░░░░░░░░░░░  67 lines of code (XS)
#   duplication   76  █████████▏░░  0.0% in vue (no markup/text)
#   dead code      8  █░░░░░░░░░░░  100.0%
#   complexity    76  █████████▏░░  0.0% in complex files
```

`0.0%`, because the 16 duplicated lines are markup. The `(no markup/text)`
note lists what was left out — the cloned `html` block, in a project that
has no markup *files* at all, plus this README's own prose; standalone HTML,
CSS or template files are excluded the same way, and a project that also
has data files sees `(no markup/text/data)`. Add one duplicated line of
script to these components and the share moves; add a hundred duplicated
template lines and it does not.

The `dead code` line is not the markup story: nothing imports these two
components, so every script line counts as unused. It is why the overall
grade is `E` — dead code is rare in the corpus the score is calibrated on
(median 0.4%), so a project that is all dead code bottoms out in that
dimension — and it would be there whatever the templates duplicated.

## Whole directory

Scanning the demo from the repository root is the command above; it reports
the one clone and nothing else, the scripts and the two files' remaining
markup being unique.