import {bold, green, red} from 'colors/safe';
import {relative} from 'path';
import {CLONE_EVENT, END_EVENT, Events} from '../events';
import {IClone} from '../interfaces/clone.interface';
import {IOptions} from '../interfaces/options.interface';
import {IReporter} from '../interfaces/reporter.interface';
import {IToken} from '../interfaces/token/token.interface';
import {StoresManager} from '../stores/stores-manager';
import {IStatistic} from "../interfaces/statistic.interface";
import {SOURCES_DB, STATISTIC_DB} from "../stores/models";

const Table = require('cli-table2');

export class ConsoleReporter implements IReporter {
  constructor(private options: IOptions) {
  }

  public attach(): void {
    Events.on(CLONE_EVENT, this.cloneFound.bind(this));
    Events.on(END_EVENT, this.finish.bind(this));
  }

  private cloneFound(clone: IClone) {
    const {duplicationA, duplicationB, format} = clone;
    console.log(
      'Clone found (' + format + '):' + (clone.is_new ? red('*') : '')
    );
    console.log(
      ` - ${getPath(
        this.options,
        StoresManager.getStore(SOURCES_DB).get(duplicationA.sourceId).id
      )} [${getSourceLocation(duplicationA.start, duplicationA.end)}]`
    );
    console.log(
      `   ${getPath(
        this.options,
        StoresManager.getStore(SOURCES_DB).get(duplicationB.sourceId).id
      )} [${getSourceLocation(duplicationB.start, duplicationB.end)}]`
    );
    console.log('');
  }

  private finish() {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);
    if (statistic) {
      const table = new Table({
        head: [
          'Format',
          'Files analyzed',
          'Total lines',
          'Clones found (new)',
          'Duplicated lines (new)',
          '%'
        ]
      });

      Object.keys(statistic.formats).forEach((format: string) => {
        table.push(this.convertStatisticToArray(format, statistic.formats[format]));
      });
      table.push(this.convertStatisticToArray(bold('Total:'), statistic.all));
      console.log(table.toString());
    }
  }


  private convertStatisticToArray(format: string, statistic: IStatistic): string[] {
    return [
      format,
      `${statistic.sources}`,
      `${statistic.lines}`,
      `${statistic.clones} (${statistic.newClones})`,
      `${statistic.duplicatedLines} (${statistic.newDuplicatedLines})`,
      `${statistic.percentage}%`,
    ]
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
