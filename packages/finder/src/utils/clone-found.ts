import {IClone, IOptions} from '@jscpd/core';
import {red} from 'colors/safe';
import {getPathConsoleString, getSourceLocation} from './reports';

export function cloneFound(clone: IClone, options: IOptions): void {
	const {duplicationA, duplicationB, format, isNew} = clone;
	console.log('Clone found (' + format + '):' + (isNew ? red('*') : ''));
	console.log(
		` - ${getPathConsoleString(duplicationA.sourceId, options)} [${getSourceLocation(
			duplicationA.start,
			duplicationA.end,
		)}] (${duplicationA.end.line - duplicationA.start.line} lines${duplicationA.end.position ? ', ' + (duplicationA.end.position - duplicationA.start.position) + ' tokens' : ''})`,
	);
	console.log(
		`   ${getPathConsoleString(duplicationB.sourceId, options)} [${getSourceLocation(
			duplicationB.start,
			duplicationB.end,
		)}]`,
	);
	console.log('');
}
