import test, { beforeEach, ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
import { normalize } from 'path';
import { IToken } from '../../interfaces/token/token.interface';
import { tokenize } from '../prism';

const file: string = normalize(
  __dirname + '/../../../../tests/fixtures/markup.html'
);
let code: string;

beforeEach(() => {
  code = readFileSync(file).toString();
});

test('should tokenize markup with embedded parts', (t: ExecutionContext) => {
  const tokens: IToken[] = tokenize(code, 'html');
  t.snapshot(tokens);
});
