import { IStoreValue } from './store/store-value.interface';

export interface ISource extends IStoreValue {
  id: string;
  source: string;
  format: string;
  isNew?: boolean;
  detectionDate?: number;
  lastUpdateDate?: number;
  lines?: number;
  range?: number[];
}
