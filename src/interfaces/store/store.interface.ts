import { IStoreValue } from './store-value.interface';

export interface IStore<TValue extends IStoreValue> {
  connect(): Promise<any>;

  init(values: { [key: string]: TValue }): Promise<any>;

  get(key: string): Promise<TValue>;

  set(key: string, value: TValue): Promise<TValue>;

  update(key: string, value: TValue): Promise<TValue>;

  delete(key: string): Promise<any>;

  has(key: string): Promise<boolean>;

  hasKeys(keys: string[]): Promise<boolean[]>;

  getAllByKeys(keys: string[]): Promise<TValue[]>;

  close(): Promise<any>;
}
