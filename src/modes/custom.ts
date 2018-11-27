import { IOptions } from '..';
import { IToken } from '../interfaces/token/token.interface';
export function custom(token: IToken, options?: IOptions): boolean {
  if (!options || !options.hasOwnProperty('tokensToSkip')) {
    throw new Error('Mode `custom` need `tokensToSkip` option in config file');
  }
  const tokensToSkip = options.tokensToSkip || [];
  return !tokensToSkip.includes(token.type);
}
