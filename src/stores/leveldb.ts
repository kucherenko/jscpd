import { ensureDirSync } from 'fs-extra';
import LevelDOWN from 'leveldown';
import LevelUp from 'levelup';
import * as rimraf from 'rimraf';
import { IStoreOptions } from '../interfaces/store/store-options.interface';
import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';

export class LevelDbStore<TValue extends IStoreValue> implements IStore<TValue> {
  private db: any;

  constructor(private options: IStoreOptions) {
    if (!options.persist) {
      rimraf.sync(`.jscpd/${this.options.name}`);
    }
    ensureDirSync(`.jscpd/${this.options.name}`);

    this.db = LevelUp(LevelDOWN(`.jscpd/${this.options.name}`));
  }

  public get(key: string): Promise<TValue> {
    return this.db
      .get(key)
      .then((value: string) => JSON.parse(value))
      .catch(() => undefined);
  }

  public getAllByKeys(keys: string[]): Promise<TValue[]> {
    return Promise.all(keys.map(i => this.get(i)));
  }

  public set(key: string, value: TValue): Promise<TValue> {
    return this.db.put(key, JSON.stringify(value));
  }

  public init(): Promise<any> {
    return Promise.resolve({});
  }

  public has(key: string): Promise<boolean> {
    return this.db
      .get(key)
      .then(() => true)
      .catch(() => false);
  }

  public hasKeys(keys: string[]): Promise<boolean[]> {
    return Promise.all(keys.map(k => this.has(k)));
  }

  public connect(): Promise<any> {
    this.db.open();
    return Promise.resolve();
  }

  public delete(key: string): Promise<any> {
    return this.db.del(key);
  }

  public update(key: string, value: TValue): Promise<any> {
    return this.delete(key).then(() => this.set(key, value));
  }

  public close(): Promise<any> {
    this.db.close();
    return Promise.resolve();
  }
}
