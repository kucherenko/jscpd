import Table from 'cli-table3';
import { bold, red } from 'colors/safe';
import { IClone, IOptions, IReporter } from '..';
import { CLONE_FOUND_EVENT, JscpdEventEmitter } from '../events';
import { IStatistic, IStatisticRow } from '../interfaces/statistic.interface';
import { getPathConsoleString, getSourceLocation } from '../utils';

export class ConsoleReporter implements IReporter {
  constructor(protected options: IOptions) {}

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(CLONE_FOUND_EVENT, this.cloneFound.bind(this));
  }

  public report(...args: [any, IStatistic]) {
    const [, statistic]: [any, IStatistic] = args;
    if (statistic) {
      const table: any[] = new Table({
        head: ['Format', 'Files analyzed', 'Total lines', 'Clones found', 'Duplicated lines', '%'],
      });
      Object.keys(statistic.formats)
        .filter((format) => statistic.formats[format].sources)
        .forEach((format: string) => {
          table.push(this.convertStatisticToArray(format, statistic.formats[format].total));
        });
      table.push(this.convertStatisticToArray(bold('Total:'), statistic.total));
      console.log(table.toString());
    }
  }

  protected cloneFound(clone: IClone) {
    const { duplicationA, duplicationB, format } = clone;
    console.log('Clone found (' + format + '):' + (clone.isNew ? red('*') : ''));
    console.log(
      ` - ${getPathConsoleString(this.options, duplicationA.sourceId)} [${getSourceLocation(
        duplicationA.start,
        duplicationA.end
      )}]`
    );
    console.log(
      `   ${getPathConsoleString(this.options, duplicationB.sourceId)} [${getSourceLocation(
        duplicationB.start,
        duplicationB.end
      )}]`
    );
    console.log('');
  }

  private convertStatisticToArray(format: string, statistic: IStatisticRow): string[] {
    return [
      format,
      `${statistic.sources}`,
      `${statistic.lines}`,
      `${statistic.clones}`,
      `${statistic.duplicatedLines}`,
      `${statistic.percentage}%`,
    ];
  }
}
