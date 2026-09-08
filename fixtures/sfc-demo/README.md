# Single-file components

Vue, Svelte and Astro files are split into blocks, and each block is scanned
in its own language: the `<script>` body as JavaScript or TypeScript, the
`<style>` body as CSS, SCSS or Less, the markup as HTML. A clone inside a
block is reported with the block's language after the file name
(`Card.vue:html`). Commands run from the repository root at default
thresholds; the `Found N clones.` line is the console reporter's.

| Directory | What it shows | Default scan |
|-----------|---------------|--------------|
| `template-range/` | a shared `<template>` between two Vue files with different scripts and styles | 1 html clone, template lines only |

## `template-range/` — the clone stops at the template

`TicketCard.vue` and `TicketPreview.vue` render the same `<template>`; one
uses `<script setup>` with scoped styles, the other the options API with
global styles. The only duplicate is the markup, and the report says so: the
html clone starts on the first line inside `<template>` and ends on the last
one, and the script and style bodies are not counted as duplicated lines.

```bash
jscpd fixtures/sfc-demo/template-range
# Clone found (html)
#  - TicketCard.vue:html [2:3 - 15:13] (14 lines, 156 tokens)
#    TicketPreview.vue:html [2:3 - 15:13]
# Found 1 clones.
```

The wrapper tags of a Vue file (`<template>`, `</template>`, `<script ...>`,
`</script>`, `<style ...>`, `</style>`) are never part of the html stream,
with or without a template block. They are identical in every Vue file and
sit at both ends of it, so scanning them would stretch every template clone
to the last `</style>` and add the script and style bodies to the
duplicated-line count. Svelte and Astro have no
template wrapper: their top-level markup is the html stream and is scanned
as a whole.
