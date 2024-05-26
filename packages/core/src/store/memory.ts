import {IStore} from '..';

export class MemoryStore<IMapFrame> implements IStore<IMapFrame> {
  private _namespace: string = '';

  protected values: Record<string, Record<string, IMapFrame>> = {};

  public namespace(namespace: string): void {
    this._namespace = namespace;
    this.values[namespace] = this.values[namespace] || {};
  }

  public get(key: string): Promise<IMapFrame> {
    return new Promise((resolve, reject) => {
      // @ts-ignore
      if (key in this.values[this._namespace]) {
        // @ts-ignore
        resolve(this.values[this._namespace][key]);
      } else {
        reject(new Error('not found'));
      }
    });
  }

  public set(key: string, value: IMapFrame): Promise<IMapFrame> {
    // @ts-ignore
    this.values[this._namespace][key] = value;
    return Promise.resolve(value);
  }

  close(): void {
    this.values = {};
  }
}
