import test, { ExecutionContext } from 'ava';
import fs from 'fs';
import fsExtra from 'fs-extra';
import { mock, stub } from 'sinon';
import { IStore } from '../../interfaces/store/store.interface';
import { FilesStore } from '../files';

test('should save data to file', (t: ExecutionContext) => {
  const fsMock = mock(fsExtra)
    .expects('writeJSONSync')
    .once()
    .withArgs('.jscpd/test.json', { test: 'test' }, { spaces: '\t' });

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test');
  store.close();
  t.truthy(fsMock.verify());
  fsMock.restore();
});

test('should read data from file', (t: ExecutionContext) => {
  const fsMock = mock(fsExtra)
    .expects('readJsonSync')
    .once()
    .withArgs('.jscpd/test.json');

  fsExtra.ensureDirSync = stub();
  fs.existsSync = stub().returns(true);

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.connect();

  t.truthy(fsMock.verify());
  fsMock.restore();
});

test('should initialize data from file', (t: ExecutionContext) => {
  stub(fsExtra, 'readJsonSync')
    .withArgs('.jscpd/test.json')
    .returns({ test: 'test' });

  fsExtra.ensureDirSync = stub();
  fs.existsSync = stub().returns(true);

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.connect();

  t.is(store.get('test'), 'test');
});

test('should initialize store with values', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.init({ test: 'test' });
  t.deepEqual(store.getAll(), { test: 'test' });
});

test('should delete record by key', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test');
  store.delete('test');
  t.falsy(store.get('test'));
});

test('should update record by key', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test');
  store.update('test', 'test1');
  t.falsy(store.get('test1'));
});

test('should return count of keys in store', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test');
  t.is(store.count(), 1);
});

test('should return all pairs key<>values', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test1');
  t.deepEqual(store.getAll(), { test: 'test1' });
});

test('should return all by keys', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test1');
  t.deepEqual(store.getAllByKeys(['test', 'zzz']), ['test1', undefined]);
});

test('should check key in store', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test1');
  t.truthy(store.has('test'));
});

test('should check array of keys in store', (t: ExecutionContext) => {
  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test1');
  t.deepEqual(store.hasKeys(['test', 'zzz']), [true, false]);
});
