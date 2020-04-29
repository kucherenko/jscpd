import {bold} from 'colors/safe';
import {IClone, IStatistic} from '@jscpd/core';
import {IReporter} from '..';

export class SilentReporter implements IReporter {

	public report(clones: IClone[], statistic: IStatistic): void {
		if (statistic) {
			console.log(
				`Duplications detection: Found ${bold(clones.length.toString())} ` +
				`exact clones with ${bold(statistic.total.duplicatedLines.toString())}(${statistic.total.percentage}%) ` +
				`duplicated lines in ${bold(statistic.total.sources.toString())} ` +
				`(${Object.keys(statistic.formats).length} formats) files.`,
			);
		}
	}
}
