import { IClone } from './clone.interface';
import { IOptions } from './options.interface';

export interface ISkiper {
  shouldSkipClone(clone: IClone, options: IOptions): boolean;
}
