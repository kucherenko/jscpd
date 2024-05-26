import {getOption, IClone, IOptions, IStatistic} from "@jscpd/core";
import {ensureDirSync, writeFileSync} from "fs-extra";
import {green} from "colors/safe";
import {join} from "path";
import {convertStatisticToArray} from "../utils/reports";
import {IReporter} from '..';
import markdownTable from "markdown-table";

export class MarkdownReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  report(clones: IClone[], statistic: IStatistic | undefined): void {
    const report = `
# Copy/paste detection report

> Duplications detection: Found ${clones.length} exact clones with ${(statistic as any).total.duplicatedLines}(${(statistic as any).total.percentage}%) duplicated lines in ${(statistic as any).total.sources} (${Object.keys((statistic as any).formats).length} formats) files.

${markdownTable([
      ['Format', 'Files analyzed', 'Total lines', 'Total tokens', 'Clones found', 'Duplicated lines', 'Duplicated tokens'],
      ...Object.keys((statistic as any).formats).map((format: string) => convertStatisticToArray(format, (statistic as any).formats[format].total)),
      convertStatisticToArray('Total:', (statistic as any).total).map(item => `**${item}**`)
    ])}
`;
    ensureDirSync(getOption('output', this.options));
    writeFileSync(getOption('output', this.options) + '/jscpd-report.md', report);
    console.log(green(`Markdown report saved to ${join(this.options.output as string, 'jscpd-report.md')}`));
  }
}
