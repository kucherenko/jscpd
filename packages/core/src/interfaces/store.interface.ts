export interface IStore<TValue> {

	namespace(name: string): void;

	get(key: string): Promise<TValue>;

	set(key: string, value: TValue): Promise<TValue>;

	close(): void;
}
