# jscpd (cpd)

Copy/paste detector — fast Rust-based CLI for code duplication detection.

A high-performance reimplementation of [jscpd](https://github.com/kucherenko/jscpd) in Rust, providing near-identical CLI compatibility with 50x+ speed improvement.

## Installation

```bash
cargo install jscpd
```

## Usage

```bash
# Scan current directory
cpd .

# Scan specific paths
cpd src/ lib/

# Set minimum tokens, lines, and output format
cpd --min-tokens 50 --min-lines 5 --reporters json,console .

# List supported formats
cpd --list

# Enable git blame
cpd --blame .
```

## Output Formats

`console`, `json`, `xml`, `csv`, `html`, `markdown`, `sarif`, `xcode`, `badge`, `silent`, `ai`, `threshold`

## npm Distribution

Also available on npm as [cpd](https://www.npmjs.com/package/cpd) with prebuilt binaries for Linux, macOS, and Windows.

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

MIT