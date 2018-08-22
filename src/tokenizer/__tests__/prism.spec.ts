import test, { ExecutionContext } from 'ava';
import { readFileSync } from 'fs';
import { normalize } from 'path';
import { IToken } from '../../interfaces/token/token.interface';
import { tokenize } from '../prism';

test('should tokenize markup with embedded parts', (t: ExecutionContext) => {
  const file: string = normalize(__dirname + '/../../../tests/fixtures/markup.html');
  const code: string = readFileSync(file).toString();
  const tokens: IToken[] = tokenize(code, 'html');
  t.snapshot(tokens);
});

test('should tokenize javascript', (t: ExecutionContext) => {
  const file: string = normalize(__dirname + '/../../../tests/fixtures/javascript/file1.js');
  const code: string = readFileSync(file).toString();
  const tokens: IToken[] = tokenize(code, 'javascript');
  t.snapshot(tokens);
});

test('should tokenize typescript', (t: ExecutionContext) => {
  const file: string = normalize(__dirname + '/../../../tests/fixtures/javascript/file1.ts');
  const code: string = readFileSync(file).toString();
  const tokens: IToken[] = tokenize(code, 'typescript');
  t.snapshot(tokens);
});
