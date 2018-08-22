import { END_PROCESS_EVENT, Events } from '../events';
import { IReporter } from '../interfaces/reporter.interface';

export class TimeReporter implements IReporter {
  public attach(): void {
    console.time('Execution Time');
    Events.on(END_PROCESS_EVENT, () => console.timeEnd('Execution Time'));
  }
}
