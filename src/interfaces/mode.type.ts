import { IToken } from './token/token.interface';

export type IMode = (token: IToken) => boolean;
