import { default as test, ExecutionContext } from 'ava';
import { MemoryStore } from '../memory';
import { StoresManager } from '../stores-manager';

test('should register new stores types', (t: ExecutionContext) => {
  StoresManager.registerStore('my-memory', MemoryStore);
  t.truthy(StoresManager.isRegistered('my-memory'));
});
