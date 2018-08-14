import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';
import {IStoreOptions} from "../interfaces/store/store-options.interface";
import {ensureDirSync} from "fs-extra";

const dirty = require('dirty');

export class FilesStore<TValue extends IStoreValue> implements IStore<TValue> {
  db: any;

  constructor(private options: IStoreOptions) {}

  connect(): Promise<any> {
    ensureDirSync('.jscpd');
    this.db = dirty(`.jscpd/${this.options.name}.db`);
    return new Promise((resolve) => {
      this.db.on('load', resolve);
    });
  }

  public get(key: string): TValue {
    return this.db.get(key);
  }

  public getAll(): { [key: string]: TValue } {
    return this.db._docs;
  }

  public getAllByKeys(keys: string[]): TValue[] {
    return keys.map(key => this.get(key));
  }

  public set(key: string, value: TValue): void {
    this.db.set(key, value);
  }

  public init(values: { [p: string]: TValue }): void {
    Object.entries(values).map(([key, value]) => this.set(key, value));
  }

  public has(key: string): boolean {
    return this.db.get(key) as boolean;
  }

  public hasKeys(keys: string[]): boolean[] {
    return keys.map(key => this.has(key));
  }

  public count(): number {
    return Object.keys(this.getAll()).length;
  }
}
