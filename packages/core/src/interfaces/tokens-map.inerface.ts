import {IMapFrame} from '.';

export interface ITokensMap {

  getFormat(): string;

  getLinesCount(): number;

  getTokensCount(): number;

  getId(): string;

  next(): IteratorResult<IMapFrame | boolean>;

}
