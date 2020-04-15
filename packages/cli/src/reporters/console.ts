import {DetectorEvents, IClone, IOptions, IReporter, ReporterHandler} from '@jscpd/core';
import {bold, green, grey, red} from 'colors/safe';
import {relative} from "path";
import {cwd} from "process";
import {ITokenLocation} from '@jscpd/tokenizer';

export class ConsoleReporter implements IReporter {
	private readonly options;

	constructor(options: IOptions) {
		this.options = options;
	}

	subscribe(): Partial<Record<DetectorEvents, ReporterHandler>> {
		return {
			CLONE_FOUND: this.cloneFound.bind(this),
		};
	}

	report(clones: IClone[]): void {
		console.log(grey(`Found ${clones.length} clones.`));
	}

	private cloneFound(clone: IClone) {
		const {duplicationA, duplicationB, format, isNew} = clone;
		console.log('Clone found (' + format + '):' + (isNew ? red('*') : ''));
		console.log(
			` - ${getPathConsoleString(duplicationA.sourceId, this.options)} [${getSourceLocation(
				duplicationA.start,
				duplicationA.end,
			)}]`,
		);
		console.log(
			`   ${getPathConsoleString(duplicationB.sourceId, this.options)} [${getSourceLocation(
				duplicationB.start,
				duplicationB.end,
			)}]`,
		);
		console.log('');
	}
}


function getPath(path: string, options: IOptions): string {
	return options.absolute ? path : relative(cwd(), path);
}

function getPathConsoleString(path: string, options: IOptions): string {
	return bold(green(getPath(path, options)));
}

function getSourceLocation(start: ITokenLocation, end: ITokenLocation): string {
	return `${start.line}:${start.column} - ${end.line}:${end.column}`;
}
