import {IClone, ICloneValidator, IOptions, IValidationResult} from '..';

export function runCloneValidators(clone: IClone, options: IOptions, validators: ICloneValidator[]): IValidationResult {
	return validators.reduce((acc: IValidationResult, validator: ICloneValidator): IValidationResult => {
		const res = validator.validate(clone, options);
		return {
			...acc,
			status: res.status && acc.status,
			message: res.message ? [...acc.message, ...res.message] : acc.message,
		};

	}, {status: true, message: [], clone})
}
