import { IStoreValue } from './store/store-value.interface';

export interface ISource extends IStoreValue {
  id: string;
  source: string;
  format: string;
  last_update?: number;
  size?: number;
}
