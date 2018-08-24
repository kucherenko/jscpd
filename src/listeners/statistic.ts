import { END_EVENT, Events, MATCH_SOURCE_EVENT } from '../events';
import { IOptions, StoresManager } from '../index';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { ISource } from '../interfaces/source.interface';
import { IStatistic } from '../interfaces/statistic.interface';
import { STATISTIC_DB } from '../stores/models';

export class StatisticListener implements IListener {
  private statistic: {
    detection_date: string;
    formats: { [format: string]: IStatistic };
    all: IStatistic;
  } = {
    detection_date: new Date().toISOString(),
    formats: {},
    all: {
      lines: 0,
      sources: 0,
      clones: 0,
      duplicatedLines: 0,
      percentage: 0,
      newDuplicatedLines: 0,
      newClones: 0
    }
  };

  constructor(private options: IOptions) {}

  public attach(): void {
    Events.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    Events.on(END_EVENT, this.calculateClones.bind(this));
  }

  private calculateClones(clones: IClone[]) {
    clones.forEach(clone => this.cloneFound(clone));
    this.saveStatistic();
  }

  private cloneFound(clone: IClone) {
    const linesCount: number = clone.duplicationA.end.line - clone.duplicationA.start.line;
    this.statistic.all.clones++;
    this.statistic.all.duplicatedLines += linesCount;
    this.statistic.formats[clone.format].clones++;
    this.statistic.formats[clone.format].duplicatedLines += linesCount;
    this.updatePercentage(clone.format);
  }

  private matchSource(source: ISource) {
    if (!this.statistic.formats.hasOwnProperty(source.format)) {
      this.statistic.formats[source.format] = this.getDefaultStatistic();
    }
    this.statistic.all.sources++;
    this.statistic.all.lines += source.lines as number;
    this.statistic.formats[source.format].sources++;
    this.statistic.formats[source.format].lines += source.lines as number;
    this.updatePercentage(source.format);
    this.saveStatistic();
  }

  private getDefaultStatistic(): IStatistic {
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

  private saveStatistic() {
    const statisticStore = StoresManager.getStore(STATISTIC_DB);
    statisticStore.set(this.options.executionId, this.statistic);
  }

  private updatePercentage(format: string) {
    this.statistic.all.percentage = this.calculatePercentage(
      this.statistic.all.lines,
      this.statistic.all.duplicatedLines
    );
    this.statistic.formats[format].percentage = this.calculatePercentage(
      this.statistic.formats[format].lines,
      this.statistic.formats[format].duplicatedLines
    );
  }

  private calculatePercentage(totalLines: number, clonedLines: number): number {
    return totalLines ? Math.round((10000 * clonedLines) / totalLines) / 100 : 0.0;
  }
}
