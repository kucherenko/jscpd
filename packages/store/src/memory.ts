import {IStore} from './interfaces';

export class MemoryStore<TValue> implements IStore<TValue> {
	private _namespace;

	protected values: Record<string, Record<string, TValue>> = {};

	public namespace(namespace: string): void {
		this._namespace = namespace;
		this.values[namespace] = this.values[namespace] || {};
	}

	public get(key: string): TValue {
		return this.values[this._namespace][key];
	}

	public set(key: string, value: TValue): TValue {
		this.values[this._namespace][key] = value;
		return value;
	}
}
