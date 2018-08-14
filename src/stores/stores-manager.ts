import {IStoreManagerOptions} from '../interfaces/store/store-manager-options.interface';
import {IStoreOptions} from '../interfaces/store/store-options.interface';
import {IStoreValue} from '../interfaces/store/store-value.interface';
import {IStore} from '../interfaces/store/store.interface';
import {MemoryStore} from './memory';
import {FilesStore} from "./files";

class StoreManager<T extends IStoreValue> {
  private registeredStores: {
    [name: string]: { new(options: IStoreOptions): IStore<T> };
  } = {
    memory: MemoryStore,
    files: FilesStore
  };

  private stores: { [key: string]: IStore<T> } = {};

  private options: IStoreManagerOptions = {};

  public initialize(options: IStoreManagerOptions) {
    this.options = options;
  }

  public connect(names: string[]): Promise<any> {
    names.map(name => this.create(name));
    return Promise.all(
      Object
        .values(this.stores)
        .map(store => store.connect())
    );
  }

  public get(name: string): IStore<T> {
    if (!this.has(name)) {
      this.create(name);
    }
    return this.stores[name];
  }

  public has(name: string): boolean {
    return this.stores.hasOwnProperty(name);
  }

  public getRegisteredStore(
    type: string
  ): { new(options: IStoreOptions): IStore<T> } {
    return this.registeredStores[type];
  }

  public isRegistered(type: string): boolean {
    return this.registeredStores.hasOwnProperty(type);
  }

  public registerStore(
    type: string,
    store: { new(options: IStoreOptions): IStore<T> }
  ): void {
    this.registeredStores[type] = store;
  }

  public create(name: string): void {
    // hashes.javascript
    const [main] = name.split('.');

    const options = this.options[main] || {
      type: 'files',
      options: {
        name
      }
    };

    if (!this.has(name)) {
      this.stores[name] = new (this.getRegisteredStore(options.type))({
        ...options.options,
        name
      });
    }
  }
}

export const StoresManager: StoreManager<any> = new StoreManager<any>();
