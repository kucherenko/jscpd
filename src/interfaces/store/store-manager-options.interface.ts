import { IStoreOptions } from './store-options.interface';

export interface IStoreManagerOptions {
  [name: string]: {
    type: string;
    options: IStoreOptions;
  };
}
