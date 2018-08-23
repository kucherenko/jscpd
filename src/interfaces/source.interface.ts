import { IStoreValue } from './store/store-value.interface';

export interface ISource extends IStoreValue {
  id: string;
  source: string;
  format: string;
  is_new?: boolean;
  detection_date?: number;
  last_update_date?: number;
  lines?: number;
  range?: number[];
}
