import {IClone} from '@jscpd/core';

export interface IHook {
	process(clones: IClone[]): Promise<IClone[]>;
}
