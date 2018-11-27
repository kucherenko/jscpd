import { IOptions } from './options.interface';
import { IToken } from './token/token.interface';

export type IMode = (token: IToken, options?: IOptions) => boolean;
