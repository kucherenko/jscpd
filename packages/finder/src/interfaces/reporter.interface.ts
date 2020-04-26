import {IClone, IStatistic} from '@jscpd/core';

export interface IReporter {
	report(clones: IClone[], statistic: IStatistic | undefined): void;
}
