import { red } from 'colors/safe';
import { IOptions, IReporter } from '..';
import { END_EVENT, JscpdEventEmitter } from '../events';
import { STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { getOption } from '../utils/options';

export class ThresholdReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(END_EVENT, this.finish.bind(this));
  }

  public report(): void {}

  private finish() {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(getOption('executionId', this.options));
    if (statistic) {
      if (this.options.threshold && this.options.threshold < statistic.total.percentage) {
        console.error(red('ERROR: jscpd found too many duplicates over threshold'));
        process.exit(1);
      }
    }
  }
}
