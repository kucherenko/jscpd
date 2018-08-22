import { ITokenLocation } from './token/token-location.interface';

export interface IClone {
  format: string;
  is_new?: boolean;
  found_date?: number;
  duplicationA: {
    sourceId: string;
    start: ITokenLocation;
    end: ITokenLocation;
    fragment: string;
    range: number[];
  };
  duplicationB: {
    sourceId: string;
    start: ITokenLocation;
    end: ITokenLocation;
    fragment: string;
    range: number[];
  };
}
