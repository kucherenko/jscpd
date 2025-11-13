import {IClone} from '@jscpd-ai/core';

export interface IHook {
	process(clones: IClone[]): Promise<IClone[]>;
}
