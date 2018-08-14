import { IToken } from './token/token.interface';

export interface IClone {
  format: string;
  fragment: string;
  is_new?: boolean;
  duplicationA: {
    sourceId: string;
    start: IToken;
    end: IToken;
  };
  duplicationB: {
    sourceId: string;
    start: IToken;
    end: IToken;
  };
}
