import {IClone, IOptions} from '@jscpd/core';
import {IReporter} from '..';
import {generateLine} from '../utils/reports';
import {cloneFound} from '../utils/clone-found';
import {grey} from 'colors/safe';

const Table = require('cli-table3');

const TABLE_OPTIONS = {
	chars: {
		top: '',
		'top-mid': '',
		'top-left': '',
		'top-right': '',
		bottom: '',
		'bottom-mid': '',
		'bottom-left': '',
		'bottom-right': '',
		left: '',
		'left-mid': '',
		mid: '',
		'mid-mid': '',
		right: '',
		'right-mid': '',
		middle: '│',
	},
};

export class ConsoleFullReporter implements IReporter {

	constructor(private readonly options: IOptions) {
	}

	public report(clones: IClone[]): void {
		clones.forEach((clone: IClone) => {
			this.cloneFullFound(clone);
		});
		console.log(grey(`Found ${clones.length} clones.`));
	}

	private cloneFullFound(clone: IClone): void {
		const table = new Table(TABLE_OPTIONS);

		cloneFound(clone, this.options);

		clone.duplicationA.fragment.split('\n').forEach((line: string, position: number) => {
			(table).push(generateLine(clone, position, line));
		});

		console.log(table.toString());
		console.log('');
	}
}
