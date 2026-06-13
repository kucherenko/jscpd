# `@jscpd/tokenizer`

> Tokenizer package for [@jscpd](https://github.com/kucherenko/jscpd) — converts source code into a list of tokens for duplicate detection.

Supports **223 programming languages and formats** via a self-contained [reprism](https://github.com/tannerlinsley/reprism)-based grammar engine. Grammars are loaded lazily for fast startup, with O(n) hot paths for high-throughput scanning.

Special tokenization modes handle multi-language files:

- **Vue SFC** (`.vue`) — `<template>`, `<script>`, and `<style>` blocks each tokenized by their own language
- **Svelte** (`.svelte`) — per-block tokenization for HTML, JS, and CSS sections
- **Astro** (`.astro`) — frontmatter and template blocks tokenized independently
- **Markdown** (`.md`) — fenced code blocks tokenized by the declared language

This enables cross-format clone detection: a `<script lang="ts">` block in a `.vue` file can match a plain `.ts` file.

## Installation

```bash
npm install @jscpd/tokenizer --save
```

## Usage

```typescript
import { IOptions, ITokensMap } from '@jscpd/core';
import { Tokenizer } from '@jscpd/tokenizer';

const tokenizer = new Tokenizer();
const options: IOptions = {};

const maps: ITokensMap[] = tokenizer.generateMaps('source_id', 'let a = "11"', 'javascript', options);
```

## Supported formats

The full list of 223 supported formats is available in [FORMATS.md](../../FORMATS.md) at the repository root, or at runtime:

```bash
jscpd --list
```


## License

[MIT](LICENSE) © Andrey Kucherenko
