import {red} from 'colors/safe';
import {IClone, IOptions, IStatistic} from '@jscpd/core';
import {IReporter} from '..';

export class ThresholdReporter implements IReporter {
	constructor(private options: IOptions) {
	}

	public report(clones: IClone[], statistic: IStatistic | undefined): void {
    if (statistic && this.options.threshold !== undefined && this.options.threshold < statistic.total.percentage) {
      const message = `ERROR: jscpd found too many duplicates (${statistic.total.percentage}%) over threshold (${this.options.threshold}%)`;
      console.error(red(message));
      throw new Error(message);
    }
	}
}
