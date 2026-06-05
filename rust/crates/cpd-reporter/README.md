# cpd-reporter

Output format reporters for [cpd](https://crates.io/crates/jscpd) — a fast copy/paste detector for code.

Supported output formats:

- Console (human-readable, with optional blame info)
- JSON
- XML
- CSV
- HTML
- Markdown
- SARIF
- Xcode
- Badge (SVG)
- AI (machine-readable)
- Silent (no output)
- Threshold (exit code on duplication percentage)

This crate is not intended to be used directly; see the `jscpd` crate for the full CLI.

## License

MIT