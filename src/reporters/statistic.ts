import { bold, green, red } from 'colors/safe';
import { relative } from 'path';
import { Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { IToken } from '../interfaces/token/token.interface';
import { StoresManager } from '../stores/stores-manager';

export class ConsoleReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {
    Events.on('clone', this.cloneFound.bind(this));
  }

  private cloneFound(clone: IClone) {
    const { duplicationA, duplicationB, format } = clone;
    console.log(
      'Clone found (' + format + '):' + (clone.is_new ? red('*') : '')
    );
    console.log(
      ` - ${getPath(
        this.options,
        StoresManager.get('source').get(duplicationA.sourceId).id
      )} [${getSourceLocation(duplicationA.start, duplicationA.end)}]`
    );
    console.log(
      `   ${getPath(
        this.options,
        StoresManager.get('source').get(duplicationB.sourceId).id
      )} [${getSourceLocation(duplicationB.start, duplicationB.end)}]`
    );
    console.log('');
  }
}

function getPath(options: IOptions, path: string): string {
  return bold(green(relative(options.path, path)));
}

function getSourceLocation(start: IToken, end: IToken): string {
  return `${start.loc.start.line}:${start.loc.start.column} - ${
    end.loc.start.line
  }:${end.loc.start.column}`;
}
