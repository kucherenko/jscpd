import {IClone, IOptions, IValidationResult} from '..';

export interface ICloneValidator {
	validate(clone: IClone, options: IOptions): IValidationResult;
}
