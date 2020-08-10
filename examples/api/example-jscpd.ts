import {IClone} from '@jscpd/core';
import {jscpd} from 'jscpd';

(async () => {
  const clones: IClone[] = await jscpd(['', '', __dirname + '/../fixtures', '-m', 'weak', '--silent']);
  console.log(clones);
})();
