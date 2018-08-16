import { IStoreValue } from './store/store-value.interface';

export interface ISource extends IStoreValue {
  id: string;
  source: string;
  format: string;
  last_update?: number;
  lines?: number;
  clones?: string[];
  hashes?: { [format: string]: string[] };
  formats?: { [key: string]: number };
}
