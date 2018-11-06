import bytes = require('bytes');
import { bgBlue, green, grey, red } from 'colors/safe';
import EventEmitter = require('eventemitter3');
import { IOptions, IReporter } from '..';
import { CLONE_FOUND_EVENT, MATCH_SOURCE_EVENT, SOURCE_SKIPPED_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { ISourceOptions } from '../interfaces/source-options.interface';

export class VerboseReporter implements IReporter {
  private startTime: [number, number];
  private sourceCount: number = 0;

  constructor(protected options: IOptions) {
    this.startTime = process.hrtime();
  }

  public attach(eventEmitter: EventEmitter): void {
    eventEmitter.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    eventEmitter.on(CLONE_FOUND_EVENT, this.cloneFound.bind(this));
    eventEmitter.on(SOURCE_SKIPPED_EVENT, this.skipSource.bind(this));
  }

  public report(): void {}

  private matchSource(source: ISourceOptions) {
    this.sourceCount++;
    console.log(green('Source matched:'));
    console.log(grey(JSON.stringify(source, null, '\t')));
    this.generateStatistic(source.format);
  }

  private skipSource(source: any) {
    console.log(
      grey(
        `Source skipped ${source.path} (Size: ${bytes(source.size)}${source.lines ? ', Lines: ' + source.lines : ''})`
      )
    );
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
