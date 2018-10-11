import test, { ExecutionContext } from 'ava';
import { getFormatByFile } from '../index';

test('should return format by file', (t: ExecutionContext) => {
  t.is(getFormatByFile('zz.js'), 'javascript');
});

test('should return format by file with custom format rules', (t: ExecutionContext) => {
  t.is(getFormatByFile('zz.js', { ololo: ['js'] }), 'ololo');
});
