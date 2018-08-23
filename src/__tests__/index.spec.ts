import test, { ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
import { spy } from 'sinon';
import { IOptions, JSCPD } from '..';
import { IClone } from '../interfaces/clone.interface';

let log: any;

const cpd = new JSCPD({} as IOptions);

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
