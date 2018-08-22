import { IToken } from '../interfaces/token/token.interface';

export * from './prism';

export function groupByFormat(tokens: IToken[]): { [key: string]: IToken[] } {
  const result: { [key: string]: IToken[] } = {};
  tokens.forEach(token => (result[token.format] = result[token.format] ? [...result[token.format], token] : [token]));
  return result;
}
