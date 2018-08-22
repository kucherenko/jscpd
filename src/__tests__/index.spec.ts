import test, { ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
// import { normalize } from 'path';
import { JSCPD } from '..';
import { IClone } from '../interfaces/clone.interface';
import { getDefaultOptions } from '../utils/options';

const sinon = require('sinon');

// const path: string = normalize(__dirname + '/../../tests/fixtures/');

let log: any;

test.beforeEach(() => {
  log = console.log;
  console.log = sinon.spy();
});

test.afterEach(() => {
  console.log = log;
});

// test('should detect clones by path', async (t: ExecutionContext) => {
//   const cpd = new JSCPD({
//     ...getDefaultOptions(),
//     silent: true,
//     cache: false
//   });
//   const clones: IClone[] = await cpd.detectInFiles(path);
//   t.snapshot(clones);
// });

test('should detect clones by source', (t: ExecutionContext) => {
  const cpd = new JSCPD({ ...getDefaultOptions() });

  const clones: IClone[] = cpd.detectBySource({
    source: readFileSync(__dirname + '/../../tests/fixtures/markup.html').toString(),
    id: '123',
    format: 'markup'
  });
  t.is(clones.length, 0);
  const clonesNew: IClone[] = cpd.detectBySource({
    source: readFileSync(__dirname + '/../../tests/fixtures/markup.html').toString(),
    id: '1233',
    format: 'markup'
  });
  t.not(clonesNew.length, 0);
});
