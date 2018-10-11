import { bgMagenta, bold, green, red } from 'colors/safe';
import { END_PROCESS_EVENT, JscpdEventEmitter } from '../events';
import { IReporter } from '../interfaces/reporter.interface';

const t = require('exectimer');

export class ExecTimerReporter implements IReporter {
  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(END_PROCESS_EVENT, this.generateReport.bind(this));
  }

  private generateReport() {
    let total: number = 0;
    Object.keys(t.timers).forEach(name => {
      const results: any = t.timers[name];
      total += results.duration();
      console.log(bgMagenta(name));
      console.log(
        red(
          `Exec count: ${results.count()}, total time: ${bold(parse(results.duration()))}, tick time: ${bold(
            parse(results.min())
          )} (${parse(results.min())} - ${parse(results.median())} - ${parse(results.max())})`
        )
      );
    });
    console.log(green(`Total time: ${parse(total)}`));
  }
}

function parse(num: number): string {
  if (num < 1e3) {
    return num + ' ns';
  } else if (num >= 1e3 && num < 1e6) {
    return num / 1e3 + ' us';
  } else if (num >= 1e6 && num < 1e9) {
    return num / 1e6 + ' ms';
  } else if (num >= 1e9) {
    return num / 1e9 + ' s';
  }
  return num.toString();
}
