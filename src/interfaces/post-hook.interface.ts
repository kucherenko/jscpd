import { IClone } from './clone.interface';

export interface IPostHook {
  use(closes: IClone[]): Promise<any>;
}
