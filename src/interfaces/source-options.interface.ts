import { IStoreValue } from './store/store-value.interface';

export interface ISourceOptions extends IStoreValue {
  id: string;
  format: string;
  isNew?: boolean;
  detectionDate?: number;
  lastUpdateDate?: number;
  lines?: number;
  range?: number[];
}
