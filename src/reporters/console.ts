import {IReporter} from "../interfaces/reporter.interface";
import {IClone} from "../interfaces/clone.interface";
import {StoresManager} from "../stores/stores-manager";
import {bold, green, red} from "colors/safe";
import {IToken} from "../interfaces/token/token.interface";
import {IOptions} from "../interfaces/options.interface";
import {relative} from "path";
import {Events} from "../events";

export class ConsoleReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  private cloneFound(clone: IClone) {
    const {duplicationA, duplicationB, format} = clone;
    console.log('Clone found (' + format + '):' + (clone.is_new ? red('*') : ''));
    console.log(` - ${getPath(this.options, StoresManager.get('source').get(duplicationA.sourceId).id)} [${getSourceLocation(duplicationA.start, duplicationA.end)}]`);
    console.log(`   ${getPath(this.options, StoresManager.get('source').get(duplicationB.sourceId).id)} [${getSourceLocation(duplicationB.start, duplicationB.end)}]`);
    console.log('');
  }

  attach(): void {
    Events.on('clone', this.cloneFound.bind(this));
  }
}

function getPath(options: IOptions, path: string): string {
  return bold(green(relative(options.path, path)));
}

function getSourceLocation(start: IToken, end: IToken): string {
  return `${start.loc.start.line}:${start.loc.start.column} - ${end.loc.start.line}:${end.loc.start.column}`;
}
