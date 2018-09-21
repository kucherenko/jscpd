import { END_PROCESS_EVENT } from '../events';
import { IReporter } from '../interfaces/reporter.interface';
import { JSCPD } from '../jscpd';

export class TimeReporter implements IReporter {
  public attach(): void {
    console.time('Execution Time');
    JSCPD.getEventsEmitter().on(END_PROCESS_EVENT, () => console.timeEnd('Execution Time'));
  }
}
