import { red } from 'colors/safe';
import { END_PROCESS_EVENT } from '../events';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { JSCPD } from '../jscpd';
import { STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class ThresholdReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {
    JSCPD.getEventsEmitter().on(END_PROCESS_EVENT, this.finish.bind(this));
  }

  private finish() {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);
    if (statistic) {
      if (this.options.threshold && this.options.threshold < statistic.total.percentage) {
        console.error(red('ERROR: jscpd found too many duplicates over threshold'));
        process.exit(1);
      }
    }
  }
}
