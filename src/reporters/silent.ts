import { bold } from 'colors/safe';
import { IOptions, IReporter } from '..';
import { IStatistic } from '../interfaces/statistic.interface';
import { STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { getOption } from '../utils/options';

export class SilentReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {}

  public report() {
    const statistic: IStatistic = StoresManager.getStore(STATISTIC_DB).get(getOption('executionId', this.options));
    if (statistic) {
      console.log(
        `Duplications detection: Found ${bold(statistic.total.clones.toString())} ` +
          `exact clones with ${bold(statistic.total.duplicatedLines.toString())}(${statistic.total.percentage}%) ` +
          `duplicated lines in ${bold(statistic.total.sources.toString())} ` +
          `(${Object.keys(statistic.formats).length} formats) files.`
      );
    }
  }
}
