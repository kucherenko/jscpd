import {IReporter} from "../interfaces/reporter.interface";
import ora = require("ora");
import {IOptions} from "../interfaces/options.interface";
import {Events} from "../events";

export class SpinnerReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  attach(): void {
    if (this.options.reporter && this.options.reporter.includes('spinner')) {

      const spinner = ora("Searching duplication").start();

      Events.on('match', ({path, format}) => {
        spinner.text = format + ': ' + path;
      });

      Events.on('end', () => spinner.stop());
    }
  }
}
