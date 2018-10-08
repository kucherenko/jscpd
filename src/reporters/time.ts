import { END_PROCESS_EVENT, JscpdEventEmitter } from '../events';
import { IReporter } from '../interfaces/reporter.interface';

export class TimeReporter implements IReporter {
  public attach(eventEmitter: JscpdEventEmitter): void {
    console.time('Execution Time');
    eventEmitter.on(END_PROCESS_EVENT, () => console.timeEnd('Execution Time'));
  }
}
