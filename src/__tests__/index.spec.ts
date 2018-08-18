import test, { ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
import { normalize } from 'path';
import { spy } from 'sinon';
import { JSCPD } from '..';
import { IClone } from '../interfaces/clone.interface';
import { getDefaultOptions } from '../utils/options';

const path: string = normalize(__dirname + '/../../tests/fixtures/');

let log: any;

test.beforeEach(() => {
  log = console.log;
  console.log = spy();
});

test.afterEach(() => {
  console.log = log;
});

test('should detect clones by source', async (t: ExecutionContext) => {
  const cpd = new JSCPD({ ...getDefaultOptions(), path });

  const clones: IClone[] = await cpd.detectBySource({
    source: readFileSync(
      __dirname + '/../../tests/fixtures/markup.html'
    ).toString(),
    id: '123',
    format: 'markup'
  });
  t.is(clones.length, 0);
  const clonesNew: IClone[] = await cpd.detectBySource({
    source: readFileSync(
      __dirname + '/../../tests/fixtures/markup.html'
    ).toString(),
    id: '1233',
    format: 'markup'
  });
  t.not(clonesNew.length, 0);
});
