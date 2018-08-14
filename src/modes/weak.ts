import { IToken } from '../interfaces/token/token.interface';
import { mild } from './mild';

export function weak(token: IToken): boolean {
  return (
    mild(token) && token.type !== 'comment' && token.type !== 'block-comment'
  );
}
