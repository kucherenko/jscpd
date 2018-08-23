import { grey } from 'colors/safe';
import { CLONE_EVENT, END_EVENT, Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { ConsoleReporter } from './console';

export class ConsoleFullReporter extends ConsoleReporter implements IReporter {
  constructor(options: IOptions) {
    super(options);
  }

  public attach(): void {
    Events.on(CLONE_EVENT, this.cloneFullFound.bind(this));
    Events.on(END_EVENT, this.finish.bind(this));
  }

  private cloneFullFound(clone: IClone) {
    if (this.options.reporters && this.options.reporters.includes('consoleFull')) {
      this.cloneFound(clone);
      console.log(grey(clone.duplicationA.fragment as string));
      console.log('');
    }
  }
}
