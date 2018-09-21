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
  sources: { [source: string]: IStatisticRow };
  total: IStatisticRow;
}

export interface IStatistic {
  total: IStatisticRow;
  detectionDate: string;
  formats: {
    [format: string]: IStatisticFormat;
  };
  threshold?: number;
}
