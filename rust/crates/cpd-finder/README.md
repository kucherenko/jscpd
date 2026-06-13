# cpd-finder

File walking and clone detection orchestration for [cpd](https://crates.io/crates/jscpd) — a fast copy/paste detector for code.

This crate handles:

- Directory walking with ignore patterns (`.gitignore`, glob exclusions)
- Tokenization dispatch per file format
- Clone detection and matching
- Git blame enrichment for duplicate origins
- Statistics aggregation

This crate is not intended to be used directly; see the `jscpd` crate for the full CLI.


## License

MIT