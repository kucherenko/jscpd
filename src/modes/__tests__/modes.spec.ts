import test, { ExecutionContext } from 'ava';
import { IToken } from '../../interfaces/token/token.interface';
import { mild, strict, weak } from '../index';

test('should filter with strict mode ignored tokens', (t: ExecutionContext) => {
  const token = { type: 'ignore' };
  t.false(strict(token as IToken));
});

test('should filter with mild mode empty tokens', (t: ExecutionContext) => {
  const token = { type: 'empty' };
  t.false(mild(token as IToken));
});

test('should filter with mild mode new_lines tokens', (t: ExecutionContext) => {
  const token = { type: 'new_line' };
  t.false(mild(token as IToken));
});

test('should filter with weak mode comments tokens', (t: ExecutionContext) => {
  const token = { type: 'comment' };
  t.false(weak(token as IToken));
});
