import {IReporter} from '..';
import {getOption, IClone, IOptions, IStatistic} from "@jscpd/core";
import {ensureDirSync, writeFileSync} from "fs-extra";
import {green} from "colors/safe";
import {join} from "path";
import {convertStatisticToArray} from "../utils/reports";

export class CSVReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  report(clones: IClone[], statistic: IStatistic | undefined): void {
    const report = [
      ['Format', 'Files analyzed', 'Total lines', 'Total tokens', 'Clones found', 'Duplicated lines', 'Duplicated tokens'],
      ...Object.keys(statistic.formats).map((format: string) => convertStatisticToArray(format, statistic.formats[format].total)),
      convertStatisticToArray('Total:', statistic.total)
    ].map((arr) => arr.join(',')).join('\n');

    ensureDirSync(getOption('output', this.options));
    writeFileSync(getOption('output', this.options) + '/jscpd-report.csv', report);
    console.log(green(`CSV report saved to ${join(this.options.output, 'jscpd-report.csv')}`));
  }


}
