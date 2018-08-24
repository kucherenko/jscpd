import test, { ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
import { spy, stub } from 'sinon';
import { IOptions, JSCPD } from '..';
import { IClone } from '../interfaces/clone.interface';

let log: any;

const cpd = new JSCPD({} as IOptions);

stub(Date.prototype, 'getTime').returns('date');

test.beforeEach(() => {
  log = console.log;
  console.log = spy();
});

test.afterEach(() => {
  console.log = log;
});

test('should detect clones by source', (t: ExecutionContext) => {
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
  t.is(clonesNew.length, 1);
});

test('should detect clones by source again', (t: ExecutionContext) => {
  const clonesNew: IClone[] = cpd.detectBySource({
    source: readFileSync(__dirname + '/../../tests/fixtures/markup.html').toString() + ';',
    id: '1233',
    format: 'markup'
  });
  t.is(clonesNew.length, 2);
});

test('should detect clones in javascript files with all reporters', async (t: ExecutionContext) => {
  const jscpd = new JSCPD({
    format: ['javascript'],
    reporters: ['json', 'xml', 'console', 'consoleFull'],
    threshold: 10
  } as IOptions);
  const clones: IClone[] = await jscpd.detectInFiles(__dirname + '/../../tests/fixtures/');
  t.snapshot(clones);
});
