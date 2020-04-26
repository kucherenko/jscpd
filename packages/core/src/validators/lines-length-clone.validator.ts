import {IClone, ICloneValidator, IOptions, IValidationResult} from '..';

export class LinesLengthCloneValidator implements ICloneValidator {

	validate(clone: IClone, options: IOptions): IValidationResult {
		const lines = clone.duplicationA.end.line - clone.duplicationA.start.line;
		const status = lines >= options.minLines;

		return {
			status,
			message: status ? ['ok'] : [`Lines of code less then limit (${lines} < ${options.minLines})`],
		};
	}

}
