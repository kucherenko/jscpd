import {IToken} from './token/token.interface';
import {IStoreManagerOptions} from "./store/store-manager-options.interface";

export interface IOptions {
  executionId: string;
  minLines: number;
  minTokens: number;
  threshold?: number;
  output: string;
  path: string;
  mode: string | ((token: IToken) => boolean);
  storeOptions?: IStoreManagerOptions;
  config?: string;
  ignore?: string[];
  format?: string[];
  reporter?: string[];
  blame?: boolean;
  cache?: boolean;
  silent?: boolean;
  debug?: boolean;
  list?: boolean;
}
