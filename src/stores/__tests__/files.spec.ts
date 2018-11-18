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
