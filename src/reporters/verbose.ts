import { green, grey, red } from 'colors/safe';
import { CLONE_EVENT, MATCH_SOURCE_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IReporter } from '../interfaces/reporter.interface';
import EventEmitter = NodeJS.EventEmitter;
import { ISource } from '../interfaces/source.interface';

export class VerboseReporter implements IReporter {
  public attach(eventEmitter: EventEmitter): void {
    eventEmitter.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    eventEmitter.on(CLONE_EVENT, this.cloneFound.bind(this));
  }

  private matchSource(source: ISource) {
    console.log(green('Source matched:'));
    console.log(grey(JSON.stringify(source, null, '\t')));
  }

  private cloneFound(clone: IClone) {
    console.log(red('Clone found:'));
    console.log(grey(JSON.stringify(clone, null, '\t')));
  }
}
