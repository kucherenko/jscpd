import {IClone, ICloneValidator, IOptions, IValidationResult} from '..';

export class LinesLengthCloneValidator implements ICloneValidator {

	validate(clone: IClone, options: IOptions): IValidationResult {
		const lines = clone.duplicationA.end.line - clone.duplicationA.start.line;
		const status = lines >= Number(options?.minLines);

		return {
			status,
			message: status ? ['ok'] : [`Lines of code less than limit (${lines} < ${options.minLines})`],
		};
	}

}
