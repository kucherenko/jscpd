import Table from 'cli-table3';
import { bold, red } from 'colors/safe';
import { CLONE_EVENT, END_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { IStatisticRow } from '../interfaces/statistic.interface';
import { JSCPD } from '../jscpd';
import { SOURCES_DB, STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { getPathConsoleString, getSourceLocation } from '../utils';

export class ConsoleReporter implements IReporter {
  constructor(protected options: IOptions) {}

  public attach(): void {
    JSCPD.getEventsEmitter().on(CLONE_EVENT, this.cloneFound.bind(this));
    JSCPD.getEventsEmitter().on(END_EVENT, this.finish.bind(this));
  }

  protected cloneFound(clone: IClone) {
    const { duplicationA, duplicationB, format } = clone;
    console.log('Clone found (' + format + '):' + (clone.isNew ? red('*') : ''));
    console.log(
      ` - ${getPathConsoleString(
        this.options,
        StoresManager.getStore(SOURCES_DB).get(duplicationA.sourceId).id
      )} [${getSourceLocation(duplicationA.start, duplicationA.end)}]`
    );
    console.log(
      `   ${getPathConsoleString(
        this.options,
        StoresManager.getStore(SOURCES_DB).get(duplicationB.sourceId).id
      )} [${getSourceLocation(duplicationB.start, duplicationB.end)}]`
    );
    console.log('');
  }

  protected finish() {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);
    if (statistic) {
      const table: any[] = new Table({
        head: ['Format', 'Files analyzed', 'Total lines', 'Clones found', 'Duplicated lines', '%']
      });
      Object.keys(statistic.formats)
        .filter(format => statistic.formats[format].sources as boolean)
        .forEach((format: string) => {
          table.push(this.convertStatisticToArray(format, statistic.formats[format].total));
        });
      table.push(this.convertStatisticToArray(bold('Total:'), statistic.total));
      console.log(table.toString());
    }
  }

  private convertStatisticToArray(format: string, statistic: IStatisticRow): string[] {
    return [
      format,
      `${statistic.sources}`,
      `${statistic.lines}`,
      `${statistic.clones}`,
      `${statistic.duplicatedLines}`,
      `${statistic.percentage}%`
    ];
  }
}
