import { IStoreValue } from './store-value.interface';

export interface IStore<TValue extends IStoreValue> {

  connect(): Promise<any>;

  init(values: { [key: string]: TValue }): void;

  get(key: string): TValue;

  set(key: string, value: TValue): void;

  has(key: string): boolean;

  count(): number;

  hasKeys(keys: string[]): boolean[];

  getAll(): { [key: string]: TValue };

  getAllByKeys(keys: string[]): TValue[];
}
