import { bold } from 'colors/safe';
import { END_EVENT, JscpdEventEmitter } from '../events';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { IStatistic } from '../interfaces/statistic.interface';
import { STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class SilentReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(END_EVENT, this.finish.bind(this));
  }

  private finish() {
    const statistic: IStatistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);
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
