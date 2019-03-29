import { green } from 'colors/safe';
import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { join } from 'path';
import { compileFile } from 'pug';
import { IClone, IOptions, IReporter, IStatistic } from '..';
import { generateLine, getPath, getSourceLocation } from '../utils';

export class HtmlReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {}

  public report(clones: IClone[], statistic: IStatistic): void {
    const reportFunction = compileFile(__dirname + '/../../html/report.pug');

    const formatsReports: any[] =
      statistic && statistic.formats
        ? Object.keys(statistic.formats).map(format => {
            return { value: statistic.formats[format].total.lines, name: format };
          })
        : [];

    const html = reportFunction({
      total: {},
      ...statistic,
      formatsReports,
      clones,
      getPath,
      getSourceLocation,
      generateLine,
      options: this.options
    });
    if (this.options.output) {
      ensureDirSync(this.options.output);
      writeFileSync(join(this.options.output, 'jscpd-report.html'), html);
      console.log(green(`HTML report saved to ${join(this.options.output, 'jscpd-report.html')}`));
    }
  }
}
