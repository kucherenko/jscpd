import {IClone} from '..';

export interface IValidationResult {
	status: boolean;
	message?: string[];
	clone?: IClone;
}
