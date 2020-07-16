import {IClone, IOptions} from '@jscpd/core';
import {IReporter} from '..';
import {getPath} from '../utils/reports';

export class XcodeReporter implements IReporter {
	constructor(private readonly options: IOptions) {
	}

	public report(clones: IClone[]): void {
		clones.forEach((clone: IClone) => {
			this.cloneFound(clone);
		});
		console.log(`Found ${clones.length} clones.`);
	}

	private cloneFound(clone: IClone): void {
		const pathA = getPath(clone.duplicationA.sourceId, {...this.options, absolute: true});
		const pathB = getPath(clone.duplicationB.sourceId, this.options);
		const startLineA = clone.duplicationA.start.line;
		const characterA = clone.duplicationA.start.column;
		const endLineA = clone.duplicationA.end.line;
		const startLineB = clone.duplicationB.start.line;
		const endLineB = clone.duplicationB.end.line;
		console.log(`${pathA}:${startLineA}:${characterA}: warning: Found ${endLineA - startLineA} lines (${startLineA}-${endLineA}) duplicated on file ${pathB} (${startLineB}-${endLineB})`);
	}
}
