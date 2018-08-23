import { IStoreValue } from './store/store-value.interface';
import { IToken } from './token/token.interface';

export interface IMapFrame extends IStoreValue {
  id: string;
  sourceId: string;
  format: string;
  start: IToken;
  end: IToken;
}
