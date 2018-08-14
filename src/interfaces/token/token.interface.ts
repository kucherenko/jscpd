import { ITokenLocation } from './token-location.interface';

export interface IToken {
  type: string;
  value: string;
  length: number;
  format: string;
  sourceId: string;
  range: number[];
  loc: {
    start: ITokenLocation;
    end: ITokenLocation;
  };
}
