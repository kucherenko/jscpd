import test, { ExecutionContext } from 'ava';
import fs from 'fs';
import fsExtra from 'fs-extra';
import { mock, stub } from 'sinon';
import { IStore } from '../../interfaces/store/store.interface';
import { FilesStore } from '../files';

test('should save data to file', (t: ExecutionContext) => {
  const fsMock = mock(fsExtra)
    .expects('writeJSON')
    .once()
    .withArgs('.jscpd/test.json', { test: 'test' }, { spaces: '\t' });

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.set('test', 'test');
  store.close();
  t.truthy(fsMock.verify());
});

test('should read data from file', (t: ExecutionContext) => {
  const fsMock = mock(fsExtra).expects('readJsonSync').once().withArgs('.jscpd/test.json');

  fsExtra.ensureDirSync = stub();
  fs.existsSync = stub().returns(true);

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.connect();

  t.truthy(fsMock.verify());
  (fsExtra.readJsonSync as any).restore();
});

test('should initialize data from file', async (t: ExecutionContext) => {
  stub(fsExtra, 'readJsonSync').withArgs('.jscpd/test.json').returns({ test: 'test' });

  fsExtra.ensureDirSync = stub();
  fs.existsSync = stub().returns(true);

  const store: IStore<any> = new FilesStore({ name: 'test' });
  store.connect();

  t.is(await store.get('test'), 'test');
  (fsExtra.readJsonSync as any).restore();
});
