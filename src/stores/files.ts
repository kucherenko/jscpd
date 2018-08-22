import { existsSync } from 'fs';
import { ensureDirSync, readJsonSync, writeJSONSync } from 'fs-extra';
import { IStoreOptions } from '../interfaces/store/store-options.interface';
import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';

export class FilesStore<TValue extends IStoreValue> implements IStore<TValue> {
  protected values: { [key: string]: TValue } = {};
  private pathToFile: string;

  constructor(private options: IStoreOptions) {
    this.pathToFile = `.jscpd/${this.options.name}.db`;
  }

  public connect(): void {
    ensureDirSync('.jscpd');
    this.values = existsSync(this.pathToFile) ? readJsonSync(this.pathToFile) : {};
  }

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

  public delete(key: string): void {
    delete this.values[key];
  }

  public update(key: string, value: TValue): void {
    this.values[key] = value;
  }

  public close(): void {
    writeJSONSync(this.pathToFile, this.values);
  }
}
