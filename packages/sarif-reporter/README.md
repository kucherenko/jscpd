# `@jscpd/sarif-reporter`

> SARIF reporter for [jscpd](https://github.com/kucherenko/jscpd) — generates Static Analysis Results Interchange Format output for integration with GitHub Code Scanning and other SARIF-compatible tools.

Each detected clone is reported as a `warning`-level SARIF result with precise file locations (line and column). If the overall duplication percentage exceeds the configured `--threshold`, an additional `error`-level result is emitted under the `duplications-threshold` rule.

Output file: `<output-dir>/jscpd-sarif.json`

## Installation

```bash
npm install @jscpd/sarif-reporter
```

## Usage

```bash
jscpd --reporters sarif --output ./reports /path/to/source
```

## GitHub Code Scanning integration

Upload the SARIF output to GitHub to surface duplication findings inline in pull requests:

```yaml
# .github/workflows/jscpd.yml
name: Code duplication check
on: [push, pull_request]

jobs:
  jscpd:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run jscpd
        run: npx jscpd --reporters sarif --output ./reports .

      - name: Upload SARIF to GitHub Code Scanning
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: ./reports/jscpd-sarif.json
```

Results appear in the **Security → Code scanning** tab of your repository and as inline annotations on pull request diffs.

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
