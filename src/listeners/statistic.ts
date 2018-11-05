import { IOptions } from '..';
import { CLONE_FOUND_EVENT, JscpdEventEmitter, MATCH_SOURCE_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { ISourceOptions } from '../interfaces/source-options.interface';
import { IStatistic, IStatisticRow } from '../interfaces/statistic.interface';
import { getStoreManager } from '../jscpd';
import { STATISTIC_DB } from '../stores/models';
import { getOption } from '../utils/options';

export class StatisticListener implements IListener {
  private static getDefaultStatistic(): IStatisticRow {
    return {
      lines: 0,
      sources: 0,
      clones: 0,
      duplicatedLines: 0,
      percentage: 0,
      newDuplicatedLines: 0,
      newClones: 0
    };
  }

  private statistic: IStatistic = {
    detectionDate: new Date().toISOString(),
    formats: {},
    total: StatisticListener.getDefaultStatistic()
  };

  constructor(private options: IOptions) {
    this.statistic.threshold = this.options.threshold;
  }

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    eventEmitter.on(CLONE_FOUND_EVENT, this.cloneFound.bind(this));
  }

  private cloneFound(clone: IClone) {
    const id: string = clone.duplicationA.sourceId;
    const id2: string = clone.duplicationB.sourceId;
    const linesCount: number = clone.duplicationA.end.line - clone.duplicationA.start.line;

    this.statistic.total.clones++;
    this.statistic.total.duplicatedLines += linesCount;
    this.statistic.formats[clone.format].total.clones++;
    this.statistic.formats[clone.format].total.duplicatedLines += linesCount;

    this.statistic.formats[clone.format].sources[id].clones++;
    this.statistic.formats[clone.format].sources[id].duplicatedLines += linesCount;

    this.statistic.formats[clone.format].sources[id2].clones++;
    this.statistic.formats[clone.format].sources[id2].duplicatedLines += linesCount;

    this.updatePercentage(clone.format);
    this.saveStatistic();
  }

  private matchSource(source: ISourceOptions) {
    source.format = source.format || 'javascript';
    if (!this.statistic.formats.hasOwnProperty(source.format)) {
      this.statistic.formats[source.format] = {
        sources: {},
        total: StatisticListener.getDefaultStatistic()
      };
    }
    this.statistic.total.sources++;
    this.statistic.total.lines += source.lines as number;
    this.statistic.formats[source.format].total.sources++;
    this.statistic.formats[source.format].total.lines += source.lines as number;

    this.statistic.formats[source.format].sources[source.id] =
      this.statistic.formats[source.format].sources[source.id] || StatisticListener.getDefaultStatistic();

    this.statistic.formats[source.format].sources[source.id].sources = 1;
    this.statistic.formats[source.format].sources[source.id].lines += source.lines as number;
    this.updatePercentage(source.format);
    this.saveStatistic();
  }

  private saveStatistic() {
    const statisticStore = getStoreManager().getStore(STATISTIC_DB);
    statisticStore.set(getOption('executionId', this.options), this.statistic);
  }

  private updatePercentage(format: string) {
    this.statistic.total.percentage = this.calculatePercentage(
      this.statistic.total.lines,
      this.statistic.total.duplicatedLines
    );
    this.statistic.formats[format].total.percentage = this.calculatePercentage(
      this.statistic.formats[format].total.lines,
      this.statistic.formats[format].total.duplicatedLines
    );

    Object.entries(this.statistic.formats[format].sources).forEach(([id, stat]) => {
      this.statistic.formats[format].sources[id].percentage = this.calculatePercentage(
        stat.lines,
        stat.duplicatedLines
      );
    });
  }

  private calculatePercentage(totalLines: number, clonedLines: number): number {
    return totalLines ? Math.round((10000 * clonedLines) / totalLines) / 100 : 0.0;
  }
}
