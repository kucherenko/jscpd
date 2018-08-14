import { IToken } from './token/token.interface';

export interface IOptions {
  minLines: number;
  minTokens: number;
  threshold?: number;
  output: string;
  path: string;
  mode: string | ((token: IToken) => boolean);
  config?: string;
  ignore?: string[];
  format?: string[];
  reporter?: string[];
  blame?: boolean;
  silent?: boolean;
  debug?: boolean;
  list?: boolean;
}
