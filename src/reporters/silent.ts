import { bold } from 'colors/safe';
import { END_EVENT, Events } from '../events';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class SilentReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {
    Events.on(END_EVENT, this.finish.bind(this));
  }

  private finish() {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);
    if (statistic) {
      console.log(
        `Duplications detection: Found ${bold(statistic.all.clones)} ` +
          `exact clones with ${bold(statistic.all.duplicatedLines)}(${statistic.all.percentage}%) ` +
          `duplicated lines in ${bold(statistic.all.sources)} ` +
          `(${Object.keys(statistic.formats).length} formats) files.`
      );
    }
  }
}
