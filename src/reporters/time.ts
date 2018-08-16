import {END_PROCESS_EVENT, Events} from '../events';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';

export class TimeReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {
    if (this.options.reporter && this.options.reporter.includes('time')) {
      console.time('Execution Time');
      Events.on(END_PROCESS_EVENT, () => console.timeEnd('Execution Time'));
    }
  }
}
