import {IReporter} from "../interfaces/reporter.interface";
import {IClone} from "../interfaces/clone.interface";
import {StoresManager} from "../stores/stores-manager";
import {bold, green, grey} from "colors/safe";
import {IToken} from "../interfaces/token/token.interface";
import {IOptions} from "../interfaces/options.interface";
import {Events} from "../events";

export class ConsoleFullReporter implements IReporter {

  constructor(private options: IOptions) {
  }

  attach(): void {
    Events.on('clone', this.cloneFound.bind(this));
  }

  private cloneFound(clone: IClone) {
    if (this.options.reporter && this.options.reporter.includes('consoleFull')) {
      const {duplicationA, duplicationB, format, fragment} = clone;
      console.log('Clone found (' + format + '):');
      console.log(` - ${getPath(StoresManager.get('source')
        .get(duplicationA.sourceId).id)} [${getSourceLocation(duplicationA.start, duplicationA.end)}]`);
      console.log(`   ${getPath(StoresManager.get('source')
        .get(duplicationB.sourceId).id)} [${getSourceLocation(duplicationB.start, duplicationB.end)}]`);
      console.log(grey(fragment));
      console.log('');
    }
  }
}

function getPath(path: string): string {
  return bold(green(path));
}

function getSourceLocation(start: IToken, end: IToken): string {
  return `${start.loc.start.line}:${start.loc.start.column} - ${end.loc.start.line}:${end.loc.start.column}`;
}
