# Statistics for embedded code blocks

Markdown, Vue, Svelte and Astro files hold code written in another language.
jscpd scans those blocks under the embedded language's own format, and this
directory shows how their lines are counted ([issue #1090][issue]).

A block keeps the line numbers of the file it lives in. Up to 5.3.1 the
statistics read those numbers as if the block ran the whole way, so the text
around it — markdown prose, a component's template — was counted as code of
the embedded language, in the total and in the duplicated lines both.

All commands run from the repository root at default thresholds.

| Directory | What is embedded | 5.3.1 typescript row | now |
|-----------|------------------|----------------------|-----|
| `markdown/` | two `ts` blocks per guide, prose between them | `114` lines, `34 (29.82%)` | `26` lines, `13 (50.00%)` |
| `component/` | one `<script lang="ts">` under a long template | `96` lines, `13 (13.54%)` | `26` lines, `13 (50.00%)` |

The quickest way to see whether a run counts blocks correctly is to compare the
two percentage columns. They measure the same duplication, so they should be
close. Where they are far apart — `29.82%` of lines against `50.00%` of tokens
— the line column is counting text that holds no code.

## `markdown/` — prose between two blocks

Each guide has a `ts` block at lines 23-29 and another at lines 51-57, with
prose in between that differs from guide to guide. The two guides share their
code and nothing else: 13 lines of TypeScript each.

```bash
jscpd fixtures/embedded-stats-demo/markdown
# Found 1 clones.
#
# │ markdown   │ 2 │ 130 │ 2584 │ 0 │ 0 (0.00%)   │ 0 (0.00%)   │
# │ typescript │ 2 │  26 │  378 │ 1 │ 13 (50.00%) │ 189 (50.00%) │
```

The clone's fragment runs from line 23 to line 57, because it covers both
blocks, and the console prints the 13 lines it actually duplicates rather than
the 35 the fragment reaches across. The 22 lines of prose in between belong to
the markdown row, which is where they already were.

## `component/` — a script under a long template

`ReportCard.vue` and `ExportCard.vue` have different templates and the same
`<script lang="ts">`, which sits at the bottom of each file.

```bash
jscpd fixtures/embedded-stats-demo/component
# Found 1 clones.
#
# │ html       │ 2 │  60 │ 1134 │ 0 │ 0 (0.00%)   │ 0 (0.00%)   │
# │ typescript │ 2 │  26 │  378 │ 1 │ 13 (50.00%) │ 189 (50.00%) │
# │ vue        │ 2 │  98 │ 1512 │ 0 │ 0 (0.00%)   │ 0 (0.00%)   │
```

Here the clone itself is one unbroken block, so the duplicated lines were
already right in 5.3.1. What was wrong is the total: the script's last token
sits on line 48, and that was taken to mean the source is 48 lines long. It is
13. A correct numerator over a denominator four times too large is what turned
50% into 13.54%.

## What counts as a line

An embedded source counts the lines that carry its code. A blank line inside a
block is not one of them, so the same code measures a line or two shorter in
markdown than it would as a standalone `.ts` file. An ordinary file is
unchanged: it still counts through to its last line with a token.

[issue]: https://github.com/kucherenko/jscpd/issues/1090
