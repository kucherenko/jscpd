export interface IStatisticRow {
	lines: number;
	sources: number;
	duplicatedLines: number;
	clones: number;
	percentage: number;
	newDuplicatedLines: number;
	newClones: number;
}

export interface IStatisticFormat {
	sources: Record<string, IStatisticRow>;
	total: IStatisticRow;
}

export interface IStatistic {
	total: IStatisticRow;
	detectionDate: string;
	formats: Record<string, IStatisticFormat>;
}
