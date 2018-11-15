import { bold } from 'colors/safe';
import { IClone, IReporter } from '..';
import { IStatistic } from '../interfaces/statistic.interface';

export class SilentReporter implements IReporter {

  public attach(): void {
  }

  public report(clones: IClone[], statistic: IStatistic) {
    if (statistic) {
      console.log(
        `Duplications detection: Found ${bold(clones.length.toString())} ` +
        `exact clones with ${bold(statistic.total.duplicatedLines.toString())}(${statistic.total.percentage}%) ` +
        `duplicated lines in ${bold(statistic.total.sources.toString())} ` +
        `(${Object.keys(statistic.formats).length} formats) files.`
      );
    }
  }
}
