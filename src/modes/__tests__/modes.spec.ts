import test, { ExecutionContext } from 'ava';
import { IToken } from '../../interfaces/token/token.interface';
import { getModeByName } from '../index';

test('should filter with strict mode ignored tokens', (t: ExecutionContext) => {
  const token = { type: 'ignore' };
  t.false(getModeByName('strict')(token as IToken));
});

test('should filter with mild mode empty tokens', (t: ExecutionContext) => {
  const token = { type: 'empty' };
  t.false(getModeByName('mild')(token as IToken));
});

test('should filter with mild mode new_lines tokens', (t: ExecutionContext) => {
  const token = { type: 'new_line' };
  t.false(getModeByName('mild')(token as IToken));
});

test('should filter with weak mode comments tokens', (t: ExecutionContext) => {
  const token = { type: 'comment' };
  t.false(getModeByName('weak')(token as IToken));
});

test('should filter with custom mode tokens described in tokensToSkip', (t: ExecutionContext) => {
  const token = { type: 'comment' };
  t.false(getModeByName('custom')(token as IToken, { tokensToSkip: ['comment'] }));
});

test('should throw error with custom mode if tokensToSkip does not exists', (t: ExecutionContext) => {
  const token = { type: 'comment' };
  t.throws(() => {
    getModeByName('custom')(token as IToken);
  });
});
