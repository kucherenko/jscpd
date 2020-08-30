import {DetectorEvents, IEventPayload, IHandler, IStatistic, IStatisticRow, ISubscriber} from '.';

export class Statistic implements ISubscriber {
	private static getDefaultStatistic(): IStatisticRow {
		return {
      lines: 0,
      tokens: 0,
      sources: 0,
      clones: 0,
      duplicatedLines: 0,
      duplicatedTokens: 0,
      percentage: 0,
      percentageTokens: 0,
      newDuplicatedLines: 0,
      newClones: 0,
    };
	}

	private statistic: IStatistic = {
		detectionDate: new Date().toISOString(),
		formats: {},
		total: Statistic.getDefaultStatistic(),
	};

	constructor(private readonly options) {
	}

	public subscribe(): Partial<Record<DetectorEvents, IHandler>> {
    return {
      CLONE_FOUND: this.cloneFound.bind(this),
      START_DETECTION: this.matchSource.bind(this),
    }
  }

  public getStatistic(): IStatistic {
    return this.statistic;
  }

  private cloneFound(payload: IEventPayload): void {
    const {clone} = payload;
    const id: string = clone.duplicationA.sourceId;
    const id2: string = clone.duplicationB.sourceId;
    const linesCount: number = clone.duplicationA.end.line - clone.duplicationA.start.line;
    const duplicatedTokens: number = clone.duplicationA.end.position - clone.duplicationA.start.position;

    this.statistic.total.clones++;
    this.statistic.total.duplicatedLines += linesCount;
    this.statistic.total.duplicatedTokens += duplicatedTokens;
    this.statistic.formats[clone.format].total.clones++;
    this.statistic.formats[clone.format].total.duplicatedLines += linesCount;
    this.statistic.formats[clone.format].total.duplicatedTokens += duplicatedTokens;

    this.statistic.formats[clone.format].sources[id].clones++;
    this.statistic.formats[clone.format].sources[id].duplicatedLines += linesCount;
    this.statistic.formats[clone.format].sources[id].duplicatedTokens += duplicatedTokens;

    this.statistic.formats[clone.format].sources[id2].clones++;
    this.statistic.formats[clone.format].sources[id2].duplicatedLines += linesCount;
    this.statistic.formats[clone.format].sources[id2].duplicatedTokens += duplicatedTokens;

    this.updatePercentage(clone.format);
  }

  private matchSource(payload: IEventPayload): void {
    const {source} = payload;
    const format = source.getFormat();
    if (!(format in this.statistic.formats)) {
      this.statistic.formats[format] = {
        sources: {},
        total: Statistic.getDefaultStatistic(),
      };
    }
    this.statistic.total.sources++;
    this.statistic.total.lines += source.getLinesCount();
    this.statistic.total.tokens += source.getTokensCount();
    this.statistic.formats[format].total.sources++;
    this.statistic.formats[format].total.lines += source.getLinesCount();
    this.statistic.formats[format].total.tokens += source.getTokensCount();

    this.statistic.formats[format].sources[source.getId()] =
      this.statistic.formats[format].sources[source.getId()] || Statistic.getDefaultStatistic();

    this.statistic.formats[format].sources[source.getId()].sources = 1;
    this.statistic.formats[format].sources[source.getId()].lines += source.getLinesCount();
    this.statistic.formats[format].sources[source.getId()].tokens += source.getTokensCount();
    this.updatePercentage(format);
  }

	private updatePercentage(format: string): void {
    this.statistic.total.percentage = Statistic.calculatePercentage(
      this.statistic.total.lines,
      this.statistic.total.duplicatedLines,
    );
    this.statistic.total.percentageTokens = Statistic.calculatePercentage(
      this.statistic.total.tokens,
      this.statistic.total.duplicatedTokens,
    );

    this.statistic.formats[format].total.percentage = Statistic.calculatePercentage(
      this.statistic.formats[format].total.lines,
      this.statistic.formats[format].total.duplicatedLines,
    );
    this.statistic.formats[format].total.percentageTokens = Statistic.calculatePercentage(
      this.statistic.formats[format].total.tokens,
      this.statistic.formats[format].total.duplicatedTokens,
    );

    Object.entries(this.statistic.formats[format].sources).forEach(([id, stat]) => {
      this.statistic.formats[format].sources[id].percentage = Statistic.calculatePercentage(
        stat.lines,
        stat.duplicatedLines,
      );
      this.statistic.formats[format].sources[id].percentageTokens = Statistic.calculatePercentage(
        stat.tokens,
        stat.duplicatedTokens,
      );
    });
  }

  private static calculatePercentage(total: number, cloned: number): number {
    return total ? Math.round((10000 * cloned) / total) / 100 : 0.0;
  }
}
