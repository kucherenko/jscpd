import {Events} from '../events';
import {IClone} from '../interfaces/clone.interface';
import {IReporter} from '../interfaces/reporter.interface';
import {IOptions, StoresManager} from "..";
import {IStatistic} from "../interfaces/statistic.interface";

export class StatisticReporter implements IReporter {

  private statistic: {
    formats: {
      [format: string]: IStatistic
    }
    all: IStatistic
  } = {
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

  constructor(private options: IOptions) {
  }

  public attach(): void {
    if (this.options.reporter && this.options.reporter.includes('stat')) {
      Events.on('clone', this.cloneFound.bind(this));
      Events.on('match', this.matchFile.bind(this));
      // Events.on('end', this.saveStatistic.bind(this));
    }
  }

  private cloneFound(clone: IClone) {
    if (!this.statistic.formats.hasOwnProperty(clone.format)) {
      this.statistic.formats[clone.format] = this.getDefaultStatistic();
    }
    const linesCount: number = clone.duplicationA.end.loc.end.line - clone.duplicationA.start.loc.start.line;
    this.statistic.all.clones++;
    this.statistic.all.duplicatedLines += linesCount;
    this.statistic.formats[clone.format].clones++;
    this.statistic.formats[clone.format].duplicatedLines += linesCount;
    if (clone.is_new) {
      this.statistic.all.newClones++;
      this.statistic.all.newDuplicatedLines += linesCount;
      this.statistic.formats[clone.format].newClones++;
      this.statistic.formats[clone.format].newDuplicatedLines += linesCount;
    }
    this.updatePercentage(clone.format);
    this.saveStatistic();
  }

  private matchFile(match: { path: string, format: string, source: string }) {
    if (!this.statistic.formats.hasOwnProperty(match.format)) {
      this.statistic.formats[match.format] = this.getDefaultStatistic();
    }
    const linesCount: number = this.getFragmentLinesCount(match.source);
    this.statistic.all.sources++;
    this.statistic.all.lines += linesCount;
    this.statistic.formats[match.format].sources++;
    this.statistic.formats[match.format].lines += linesCount;
    this.updatePercentage(match.format);
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
    }
  }

  private getFragmentLinesCount(fragment: string) {
    return fragment.split('\n').length;
  }

  private saveStatistic() {
    const statisticStore = StoresManager.get('statistic');

    if (statisticStore.has(this.options.executionId)) {
      statisticStore.delete(this.options.executionId);
    }
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
    return totalLines ? Math.round(10000 * clonedLines / totalLines) / 100 : 0.00;
  }
}

