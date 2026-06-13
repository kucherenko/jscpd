# `@jscpd/html-reporter`

> HTML reporter for [jscpd](https://github.com/kucherenko/jscpd) — generates an interactive HTML report with per-format statistics, a duplication graph, and syntax-highlighted clone diff views.

Output directory: `<output-dir>/html/`  
Entry point: `<output-dir>/html/index.html`  
Raw data: `<output-dir>/html/jscpd-report.json`

## Installation

```bash
npm install @jscpd/html-reporter
```

## Usage

```bash
jscpd --reporters html --output ./reports /path/to/source
```

Then open `./reports/html/index.html` in a browser.

## Programmatic usage

```typescript
import { IClone, IOptions, IStatistic } from '@jscpd/core';
import HtmlReporter from '@jscpd/html-reporter';

const options: IOptions = { output: './reports' };
const reporter = new HtmlReporter(options);

reporter.report(clones, statistic);
// writes ./reports/html/index.html and ./reports/html/jscpd-report.json
```


## License

[MIT](LICENSE) © Andrey Kucherenko
