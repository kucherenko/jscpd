import {IClone, IStatistic} from '@jscpd-ai/core';

export interface IReporter {
	report(clones: IClone[], statistic: IStatistic | undefined): void;
}
