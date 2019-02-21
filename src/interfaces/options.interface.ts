import { IStoreManagerOptions } from './store/store-manager-options.interface';
import { IToken } from './token/token.interface';

export interface IOptions {
  executionId?: string;
  minLines?: number;
  maxLines?: number;
  maxSize?: string;
  minTokens?: number;
  threshold?: number;
  xslHref?: string; // deprecated
  formatsExts?: { [key: string]: string[] };
  output?: string;
  path?: string[];
  mode?: string | ((token: IToken) => boolean);
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
  noSymlinks?: boolean;
  gitignore?: boolean;
  storeOptions?: IStoreManagerOptions;
  reportersOptions?: {
    [name: string]: any;
  };
  tokensToSkip?: string[];
}
