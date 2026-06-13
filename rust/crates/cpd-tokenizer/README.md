# cpd-tokenizer

Source code tokenizers for [cpd](https://crates.io/crates/jscpd) — a fast copy/paste detector for code.

This crate provides language-aware tokenization for 200+ programming languages, producing token streams suitable for duplicate detection. It includes:

- Generic tokenizer (comments, strings, keywords, punctuation)
- JavaScript/TypeScript/JSX/TSX tokenizer (Oxc-based)
- Markdown tokenizer (code fences, front matter, embedded languages)
- SFC tokenizer (Vue, Svelte, Astro)
- Embedded language detection and cross-format tokenization

This crate is not intended to be used directly; see the `jscpd` crate for the full CLI.

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

MIT