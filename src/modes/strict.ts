import { IToken } from '../interfaces/token/token.interface';

export function strict(token: IToken): boolean {
  return token.type !== 'ignore';
}
