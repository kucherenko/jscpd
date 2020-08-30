export interface IStatisticRow {
  lines: number;
  tokens: number;
  sources: number;
  duplicatedLines: number;
  duplicatedTokens: number;
  clones: number;
  percentage: number;
  percentageTokens: number;
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
