import {getOption, IClone, ICloneValidator, IOptions, IValidationResult} from '@jscpd/core';
import {isAbsolute, relative} from "path";

export class SkipLocalValidator implements ICloneValidator {

	validate(clone: IClone, options: IOptions): IValidationResult {
		const status = !this.shouldSkipClone(clone, options);
		return {
			status,
			clone,
			message: [
				`Sources of duplication located in same local folder (${clone.duplicationA.sourceId}, ${clone.duplicationB.sourceId})`
			]
		};
	}

	public shouldSkipClone(clone: IClone, options: IOptions): boolean {
		const path: string[] = getOption('path', options);
		return path.some(
			(dir) => SkipLocalValidator.isRelative(clone.duplicationA.sourceId, dir) && SkipLocalValidator.isRelative(clone.duplicationB.sourceId, dir)
		);
	}

	private static isRelative(file: string, path: string): boolean {
		const rel = relative(path, file);
		return rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);
	}

}
