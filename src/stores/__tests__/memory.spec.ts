import test, { ExecutionContext } from 'ava';
import { IStore } from '../../interfaces/store/store.interface';
import { MemoryStore } from '../memory';

test('should initialize store with values', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  store.init({ test: 'test' });
  t.deepEqual(await store.getAll(), { test: 'test' });
});

test('should delete record by key', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test');
  await store.delete('test');
  t.falsy(await store.get('test'));
});

test('should update record by key', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test');
  await store.update('test', 'test1');
  t.falsy(await store.get('test1'));
});

test('should return count of keys in store', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test');
  t.is(await store.count(), 1);
});

test('should return total pairs key<>values', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test1');
  t.deepEqual(await store.getAll(), { test: 'test1' });
});

test('should return total by keys', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test1');
  t.deepEqual(await store.getAllByKeys(['test', 'zzz']), ['test1', undefined]);
});

test('should check key in store', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test1');
  t.truthy(await store.has('test'));
});

test('should check array of keys in store', async (t: ExecutionContext) => {
  const store: IStore<any> = new MemoryStore();
  await store.set('test', 'test1');
  t.deepEqual(await store.hasKeys(['test', 'zzz']), [true, false]);
});
