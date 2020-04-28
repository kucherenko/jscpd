import {IStore} from '..';

export class MemoryStore<TValue> implements IStore<TValue> {
	private _namespace: string;

	protected values: Record<string, Record<string, TValue>> = {};

	public namespace(namespace: string): void {
		this._namespace = namespace;
		this.values[namespace] = this.values[namespace] || {};
	}

	public get(key: string): Promise<TValue> {
		return new Promise((resolve, reject) => {
      if (key in this.values[this._namespace]) {
        resolve(this.values[this._namespace][key]);
      } else {
        reject(new Error('not found'));
      }
    });
	}

	public set(key: string, value: TValue): Promise<TValue> {
		this.values[this._namespace][key] = value;
		return Promise.resolve(value);
	}
}
