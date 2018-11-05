import bytes = require('bytes');
import Table from 'cli-table3';
import { grey } from 'colors/safe';
import { IOptions, IReporter } from '..';
import { CLONE_FOUND_EVENT, JscpdEventEmitter, SOURCE_SKIPPED_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { ConsoleReporter } from './console';

export class ConsoleFullReporter extends ConsoleReporter implements IReporter {
  constructor(options: IOptions) {
    super(options);
  }

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(CLONE_FOUND_EVENT, this.cloneFullFound.bind(this));
    eventEmitter.on(SOURCE_SKIPPED_EVENT, this.skipSource.bind(this));
  }

  protected skipSource(source: any) {
    console.log(
      grey(
        `Source skipped ${source.path} (Size: ${bytes(source.size)}${source.lines ? ', Lines: ' + source.lines : ''})`
      )
    );
  }

  private cloneFullFound(clone: IClone) {
    if (this.options.reporters && this.options.reporters.includes('consoleFull')) {
      const table = new Table({
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
          middle: '│'
        }
      });

      this.cloneFound(clone);

      clone.duplicationA.fragment.split('\n').forEach((line: string, position: number) => {
        (table as any).push([
          clone.duplicationA.start.line + position,
          clone.duplicationB.start.line + position,
          grey(line)
        ]);
      });

      console.log(table.toString());
      console.log('');
    }
  }
}
