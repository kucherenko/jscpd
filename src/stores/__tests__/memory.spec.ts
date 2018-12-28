import test, { ExecutionContext } from 'ava';
import { IStore } from '../../interfaces/store/store.interface';
import { MemoryStore } from '../memory';

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
