import { IStoreValue } from './store-value.interface';

export interface IStore<TValue extends IStoreValue> {
  connect(): Promise<any>;

  init(values: { [key: string]: TValue }): Promise<any>;

  get(key: string): Promise<TValue>;

  set(key: string, value: TValue): Promise<TValue>;

  update(key: string, value: TValue): Promise<TValue>;

  delete(key: string): Promise<any>;

  has(key: string): Promise<boolean>;

  count(): Promise<number>;

  hasKeys(keys: string[]): Promise<boolean[]>;

  getAll(): Promise<{ [key: string]: TValue }>;

  getAllByKeys(keys: string[]): Promise<TValue[]>;

  close(): Promise<any>;
}
