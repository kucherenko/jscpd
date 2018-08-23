import { IOptions } from '..';
import { IToken } from '../interfaces/token/token.interface';
import { mild } from './mild';
import { strict } from './strict';
import { weak } from './weak';

export * from './strict';
export * from './mild';
export * from './weak';

export function getModeByName(name: string): (token: IToken) => boolean {
  switch (name) {
    case 'strict':
      return strict;
    case 'weak':
      return weak;
    default:
      return mild;
  }
}

export function getModeHandler(options: IOptions): (token: IToken) => boolean {
  return typeof options.mode === 'string' ? getModeByName(options.mode) : options.mode;
}
