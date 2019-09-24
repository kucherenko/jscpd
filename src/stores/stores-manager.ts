import { existsSync, readdirSync } from 'fs';
import rimraf from 'rimraf';
import { IStoreManagerOptions } from '../interfaces/store/store-manager-options.interface';
import { IStoreOptions } from '../interfaces/store/store-options.interface';
import { IStoreValue } from '../interfaces/store/store-value.interface';
import { IStore } from '../interfaces/store/store.interface';
import { ModuleType, use } from '../utils/use';
import { FilesStore } from './files';
import { LevelDbStore } from './leveldb';
import { MemoryStore } from './memory';

export class StoreManager<T extends IStoreValue> {
  private registeredStores: {
    [name: string]: new (options: IStoreOptions) => IStore<T>;
  } = {
    memory: MemoryStore,
    files: FilesStore,
    leveldb: LevelDbStore
  };

  private stores: { [key: string]: IStore<T> } = {};

  private options: IStoreManagerOptions = {};

  public initialize(options: IStoreManagerOptions = {}) {
    this.options = options;
  }

  public flush() {
    this.stores = {};
  }

  public close() {
    return Promise.all(Object.values(this.stores).map(store => store.close())).then(() => {
      this.flush();
      const path: string = '.jscpd';
      if (existsSync(path)) {
        const subFolders: string[] = readdirSync(path);
        if (subFolders.length === 0) {
          rimraf(path, {}, (err: Error) => {
            if (err) {
              console.log(err);
            }
          });
        }
      }
    });
  }

  public getStore(name: string): IStore<T> {
    if (!this.has(name)) {
      this.create(name);
    }
    return this.stores[name];
  }

  public has(name: string): boolean {
    return this.stores.hasOwnProperty(name);
  }

  public getRegisteredStore(type: string): new (options: IStoreOptions) => IStore<T> {
    if (!this.isRegistered(type)) {
      this.registeredStores[type] = use(type, ModuleType.db);
    }
    return this.registeredStores[type];
  }

  public isRegistered(type: string): boolean {
    return this.registeredStores.hasOwnProperty(type);
  }

  public create(name: string): void {
    if (!this.has(name)) {
      // hashes.javascript
      const [main] = name.split('.');

      const { type, options = {} } = this.options[name] ||
        this.options[main] ||
        this.options['*'] || { type: 'leveldb' };

      this.stores[name] = new (this.getRegisteredStore(type))({
        ...options,
        name
      });
      this.stores[name].connect();
    }
  }
}

export const StoresManager: StoreManager<any> = new StoreManager<any>();
