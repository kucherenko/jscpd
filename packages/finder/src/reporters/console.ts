import {IClone, IOptions, IStatistic, IStatisticRow} from '@jscpd/core';
import {bold, grey} from 'colors/safe';
import {IReporter} from '..';

const Table = require('cli-table3');

export class ConsoleReporter implements IReporter {
	private readonly options;

	constructor(options: IOptions) {
		this.options = options;
	}

	report(clones: IClone[], statistic: IStatistic | undefined = undefined): void {
		if (statistic) {
			const table: any[] = new Table({
				head: ['Format', 'Files analyzed', 'Total lines', 'Clones found', 'Duplicated lines', '%'],
			});
			Object.keys(statistic.formats)
				.filter((format) => statistic.formats[format].sources)
				.forEach((format: string) => {
					table.push(ConsoleReporter.convertStatisticToArray(format, statistic.formats[format].total));
				});
			table.push(ConsoleReporter.convertStatisticToArray(bold('Total:'), statistic.total));
			console.log(table.toString());
		}
		console.log(grey(`Found ${clones.length} clones.`));
	}

	private static convertStatisticToArray(format: string, statistic: IStatisticRow): string[] {
		return [
			format,
			`${statistic.sources}`,
			`${statistic.lines}`,
			`${statistic.clones}`,
			`${statistic.duplicatedLines}`,
			`${statistic.percentage}%`,
		]
	}
}
