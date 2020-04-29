import {IMapFrame} from '.';

export interface ITokensMap {

  getFormat(): string;

  getLinesCount(): number;

  getId(): string;

  next(): IteratorResult<IMapFrame | boolean>;

}
