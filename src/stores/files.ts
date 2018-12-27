import { existsSync } from 'fs';
import { ensureDirSync, readJsonSync, writeJSON } from 'fs-extra';
import { IStoreOptions } from '../interfaces/store/store-options.interface';
import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';
import { MemoryStore } from './memory';

export class FilesStore<TValue extends IStoreValue> extends MemoryStore<TValue> implements IStore<TValue> {
  protected values: { [key: string]: TValue } = {};
  private pathToFile: string;

  constructor(private options: IStoreOptions) {
    super();
    this.pathToFile = `.jscpd/${this.options.name}.json`;
  }

  public connect(): Promise<any> {
    ensureDirSync('.jscpd');
    return this.init(existsSync(this.pathToFile) ? readJsonSync(this.pathToFile) : {});
  }

  public init(values: { [p: string]: TValue }): Promise<any> {
    this.values = values;
    return Promise.resolve(values);
  }

  public close(): Promise<any> {
    return writeJSON(this.pathToFile, this.values, { spaces: '\t' });
  }
}
