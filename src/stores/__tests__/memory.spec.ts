import test, { ExecutionContext } from 'ava';
import { IStore } from '../../interfaces/store/store.interface';
import { MemoryStore } from '../memory';

test('should initialize store with values', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.init({ test: 'test' });
  t.deepEqual(store.getAll(), { test: 'test' });
});

test('should delete record by key', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test');
  store.delete('test');
  t.falsy(store.get('test'));
});

test('should update record by key', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test');
  store.update('test', 'test1');
  t.falsy(store.get('test1'));
});

test('should return count of keys in store', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test');
  t.is(store.count(), 1);
});

test('should return all pairs key<>values', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test1');
  t.deepEqual(store.getAll(), { test: 'test1' });
});

test('should return all by keys', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test1');
  t.deepEqual(store.getAllByKeys(['test', 'zzz']), ['test1', undefined]);
});

test('should check key in store', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test1');
  t.truthy(store.has('test'));
});

test('should check array of keys in store', (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.set('test', 'test1');
  t.deepEqual(store.hasKeys(['test', 'zzz']), [true, false]);
});
