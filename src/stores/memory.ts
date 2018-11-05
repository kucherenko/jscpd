import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';

export class MemoryStore<TValue extends IStoreValue> implements IStore<TValue> {
  protected values: { [key: string]: TValue } = {};

  public get(key: string): TValue {
    return this.values[key];
  }

  public getAll(): { [key: string]: TValue } {
    return this.values;
  }

  public getAllByKeys(keys: string[]): TValue[] {
    return keys.map(key => this.get(key));
  }

  public set(key: string, value: TValue): void {
    this.values[key] = value;
  }

  public init(values: { [p: string]: TValue }): void {
    this.values = values;
  }

  public has(key: string): boolean {
    return this.values.hasOwnProperty(key);
  }

  public hasKeys(keys: string[]): boolean[] {
    return keys.map(key => this.has(key));
  }

  public count(): number {
    return Object.keys(this.values).length;
  }

  public connect(): void {
    return;
  }

  public delete(key: string): void {
    delete this.values[key];
  }

  public update(key: string, value: TValue): void {
    this.values[key] = value;
  }

  public close(): void {
    // Object.freeze(this.values);
  }
}
