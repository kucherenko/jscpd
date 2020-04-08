import bytes = require('bytes');
import Table from 'cli-table3';
import { grey } from 'colors/safe';
import { IClone, IOptions, IReporter } from '..';
import { JscpdEventEmitter, SOURCE_SKIPPED_EVENT } from '../events';
import { generateLine } from '../utils';
import { ConsoleReporter } from './console';

const TABLE_OPTIONS = {
  chars: {
    top: '',
    'top-mid': '',
    'top-left': '',
    'top-right': '',
    bottom: '',
    'bottom-mid': '',
    'bottom-left': '',
    'bottom-right': '',
    left: '',
    'left-mid': '',
    mid: '',
    'mid-mid': '',
    right: '',
    'right-mid': '',
    middle: '│',
  },
};

export class ConsoleFullReporter extends ConsoleReporter implements IReporter {
  constructor(options: IOptions) {
    super(options);
  }

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(SOURCE_SKIPPED_EVENT, this.skipSource.bind(this));
  }

  public report(clones: IClone[]): void {
    clones.forEach((clone: IClone) => {
      this.cloneFullFound(clone);
    });
  }

  protected skipSource(source: any) {
    console.log(
      grey(`Skipped ${source.path} (Size: ${bytes(source.size)}${source.lines ? ', Lines: ' + source.lines : ''})`)
    );
  }

  private cloneFullFound(clone: IClone) {
    if (this.options.reporters && this.options.reporters.includes('consoleFull')) {
      const table = new Table(TABLE_OPTIONS);

      this.cloneFound(clone);

      clone.duplicationA.fragment.split('\n').forEach((line: string, position: number) => {
        (table as any).push(generateLine(clone, position, line));
      });

      console.log(table.toString());
      console.log('');
    }
  }
}
