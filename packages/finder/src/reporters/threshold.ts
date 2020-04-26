import {red} from 'colors/safe';
import {IClone, IOptions, IStatistic} from '@jscpd/core';
import {IReporter} from '..';

export class ThresholdReporter implements IReporter {
	constructor(private options: IOptions) {
	}

	public report(clones: IClone[], statistic: IStatistic | undefined): void {
		if (statistic) {
			if (this.options.threshold !== undefined && this.options.threshold < statistic.total.percentage) {
				console.error(
					red(
						`ERROR: jscpd found too many duplicates (${statistic.total.percentage}%) over threshold (${this.options.threshold}%)`,
					),
				);
				// process.exit(1);
			}
		}
	}
}
