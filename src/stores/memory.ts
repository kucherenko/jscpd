import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';

export class MemoryStore<TValue extends IStoreValue> implements IStore<TValue> {
  protected values: { [key: string]: TValue } = {};

  public get(key: string): Promise<TValue> {
    return Promise.resolve(this.values[key]);
  }

  public getAll(): Promise<{ [key: string]: TValue }> {
    return Promise.resolve(this.values);
  }

  public getAllByKeys(keys: string[]): Promise<TValue[]> {
    return Promise.resolve(keys.map(key => this.values[key]));
  }

  public set(key: string, value: TValue): Promise<TValue> {
    this.values[key] = value;
    return Promise.resolve(value);
  }

  public init(values: { [p: string]: TValue }): Promise<any> {
    this.values = values;
    return Promise.resolve(values);
  }

  public has(key: string): Promise<boolean> {
    return Promise.resolve(this.values.hasOwnProperty(key));
  }

  public hasKeys(keys: string[]): Promise<boolean[]> {
    return Promise.resolve(keys.map(key => this.values.hasOwnProperty(key)));
  }

  public count(): Promise<number> {
    return Promise.resolve(Object.keys(this.values).length);
  }

  public connect(): Promise<any> {
    return Promise.resolve();
  }

  public delete(key: string): Promise<any> {
    delete this.values[key];
    return Promise.resolve();
  }

  public update(key: string, value: TValue): Promise<any> {
    this.values[key] = value;
    return Promise.resolve(value);
  }

  public close(): Promise<any> {
    return Promise.resolve();
  }
}
