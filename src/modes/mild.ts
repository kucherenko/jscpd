import { IToken } from '../interfaces/token/token.interface';
import { strict } from './strict';

export function mild(token: IToken): boolean {
  return strict(token) && token.type !== 'empty' && token.type !== 'new_line';
}
