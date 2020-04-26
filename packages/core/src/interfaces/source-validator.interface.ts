import {IClone} from '..';

export interface ISourceValidator {
	validate(clone: IClone): boolean;
}
