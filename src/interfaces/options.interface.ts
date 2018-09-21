import { IStoreManagerOptions } from './store/store-manager-options.interface';
import { IToken } from './token/token.interface';

export interface IOptions {
  executionId: string;
  minLines: number;
  minTokens: number;
  threshold?: number;
  xslHref?: string;
  formatsExts?: { [key: string]: string[] };
  output: string;
  path: string;
  mode: string | ((token: IToken) => boolean);
  storeOptions?: IStoreManagerOptions;
  config?: string;
  ignore?: string[];
  format?: string[];
  reporters?: string[];
  listeners?: string[];
  blame?: boolean;
  cache?: boolean;
  silent?: boolean;
  debug?: boolean;
  list?: boolean;
  absolute?: boolean;
  gitignore?: boolean;
}
