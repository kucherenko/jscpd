import { bgBlue, green, grey, red } from 'colors/safe';
import { CLONE_EVENT, MATCH_SOURCE_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import EventEmitter = NodeJS.EventEmitter;
import { ISource } from '../interfaces/source.interface';

export class VerboseReporter implements IReporter {
  private startTime: [number, number];
  private sourceCount: number = 0;
  constructor(protected options: IOptions) {
    this.startTime = process.hrtime();
  }

  public attach(eventEmitter: EventEmitter): void {
    eventEmitter.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    eventEmitter.on(CLONE_EVENT, this.cloneFound.bind(this));
  }

  private matchSource(source: ISource) {
    this.sourceCount++;
    console.log(green('Source matched:'));
    console.log(grey(JSON.stringify(source, null, '\t')));
    this.generateStatistic(source.format);
  }

  private cloneFound(clone: IClone) {
    console.log(red('Clone found:'));
    console.log(grey(JSON.stringify(clone, null, '\t')));
    this.generateStatistic(clone.format);
  }

  private generateStatistic(format: string) {
    console.log(
      bgBlue([parseHrtimeToSeconds(process.hrtime(this.startTime)), 'sec', format, this.sourceCount].join(' '))
    );
  }
}

function parseHrtimeToSeconds(hrtime: [number, number]): string {
  return (hrtime[0] + hrtime[1] / 1e9).toFixed(3);
}
