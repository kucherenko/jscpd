import {DetectorEvents, IEventPayload, IHandler, IStatistic, IStatisticRow, ISubscriber} from '..';

export class Statistic implements ISubscriber {
	private static getDefaultStatistic(): IStatisticRow {
		return {
			lines: 0,
			sources: 0,
			clones: 0,
			duplicatedLines: 0,
			percentage: 0,
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

	private cloneFound(payload: IEventPayload) {
		const {clone} = payload;
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
	}

	private matchSource(payload: IEventPayload) {
		const {source} = payload;
		const format = source.getFormat() || 'javascript';
		if (!this.statistic.formats.hasOwnProperty(format)) {
			this.statistic.formats[format] = {
				sources: {},
				total: Statistic.getDefaultStatistic(),
			};
		}
		this.statistic.total.sources++;
		this.statistic.total.lines += source.getLinesCount();
		this.statistic.formats[format].total.sources++;
		this.statistic.formats[format].total.lines += source.getLinesCount();

		this.statistic.formats[format].sources[source.getId()] =
			this.statistic.formats[format].sources[source.getId()] || Statistic.getDefaultStatistic();

		this.statistic.formats[format].sources[source.getId()].sources = 1;
		this.statistic.formats[format].sources[source.getId()].lines += source.getLinesCount();
		this.updatePercentage(format);
	}

	private updatePercentage(format: string) {
		this.statistic.total.percentage = Statistic.calculatePercentage(
			this.statistic.total.lines,
			this.statistic.total.duplicatedLines,
		);
		this.statistic.formats[format].total.percentage = Statistic.calculatePercentage(
			this.statistic.formats[format].total.lines,
			this.statistic.formats[format].total.duplicatedLines,
		);

		Object.entries(this.statistic.formats[format].sources).forEach(([id, stat]) => {
			this.statistic.formats[format].sources[id].percentage = Statistic.calculatePercentage(
				stat.lines,
				stat.duplicatedLines,
			);
		});
	}

	private static calculatePercentage(totalLines: number, clonedLines: number): number {
		return totalLines ? Math.round((10000 * clonedLines) / totalLines) / 100 : 0.0;
	}
}
