import {getOption, IClone, IOptions, IStatistic} from "@jscpd/core";
import {ensureDirSync, writeFileSync} from "fs-extra";
import {green} from "colors/safe";
import {join} from "path";
import {convertStatisticToArray} from "../utils/reports";
import {IReporter} from '..';

const table = require('markdown-table');

export class MarkdownReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  report(clones: IClone[], statistic: IStatistic | undefined): void {
    const report = `
# Copy/paste detection report

> Duplications detection: Found ${clones.length} exact clones with ${statistic.total.duplicatedLines}(${statistic.total.percentage}%) duplicated lines in ${statistic.total.sources} (${Object.keys(statistic.formats).length} formats) files.

${table([
      ['Format', 'Files analyzed', 'Total lines', 'Total tokens', 'Clones found', 'Duplicated lines', 'Duplicated tokens'],
      ...Object.keys(statistic.formats).map((format: string) => convertStatisticToArray(format, statistic.formats[format].total)),
      convertStatisticToArray('Total:', statistic.total).map(item => `**${item}**`)
    ])}
`;
    ensureDirSync(getOption('output', this.options));
    writeFileSync(getOption('output', this.options) + '/jscpd-report.md', report);
    console.log(green(`Markdown report saved to ${join(this.options.output, 'jscpd-report.md')}`));
  }
}
