import {IReporter} from "../interfaces/reporter.interface";
import {IOptions} from "../interfaces/options.interface";
import {Events} from "../events";

export class TimeReporter implements IReporter{

  constructor(private options: IOptions){}

  attach(): void {
    if (this.options.reporter && this.options.reporter.includes('time')) {
      console.time('Execution Time');
      Events.on('end', () => console.timeEnd('Execution Time'));
    }
  }

}
