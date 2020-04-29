import {IOptions, ITokensMap} from '@jscpd/core';

export interface ITokenizer {
  generateMaps(id: string, data: string, format: string, options: Partial<IOptions>): ITokensMap[];
}
