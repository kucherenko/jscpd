import { ITokenLocation } from './token/token-location.interface';

export interface IClone {
  format: string;
  isNew?: boolean;
  foundDate?: number;
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
