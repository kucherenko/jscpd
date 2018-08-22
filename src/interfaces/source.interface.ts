import { IStoreValue } from './store/store-value.interface';

export interface ISource extends IStoreValue {
  id: string;
  source: string;
  format: string;
  meta?: ISourceMeta;
}

interface ISourceMeta {
  detection_date?: number;
  last_update_date?: number;
  lines?: number;
  clones?: string[];
  hashes?: { [format: string]: string[] };
  formats?: { [key: string]: number };
}
