import { IToken } from '../interfaces/token/token.interface';
import { TokensMap } from './token-map';

export * from './prism';

export function groupByFormat(tokens: IToken[]): { [key: string]: IToken[] } {
  const result: { [key: string]: IToken[] } = {};
  tokens.forEach((token) => (result[token.format] = result[token.format] ? [...result[token.format], token] : [token]));
  return result;
}

export function generateMapsForFormats(tokens: IToken[], minTokens: number): TokensMap[] {
  return Object.values(groupByFormat(tokens)).map((toks) => new TokensMap(toks, toks[0].format, minTokens));
}

export function createTokensMaps(tokens: IToken[], minTokens: number): TokensMap[] {
  return generateMapsForFormats(tokens, minTokens);
}
