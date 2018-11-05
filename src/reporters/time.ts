import { IReporter } from '..';
import { END_EVENT, JscpdEventEmitter } from '../events';

export class TimeReporter implements IReporter {
  public attach(eventEmitter: JscpdEventEmitter): void {
    console.time('Execution Time');
    eventEmitter.on(END_EVENT, () => console.timeEnd('Execution Time'));
  }

  public report(): void {}
}
