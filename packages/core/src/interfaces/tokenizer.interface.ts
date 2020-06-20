import {IOptions, ITokensMap} from '.';

export interface ITokenizer {
  generateMaps(id: string, data: string, format: string, options: Partial<IOptions>): ITokensMap[];
}
