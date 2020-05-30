import {IOptions, ITokenizer, ITokensMap} from '@jscpd/core';
import {createTokenMapBasedOnCode} from './tokenize';

export * from './interfaces';
export * from './tokenize';
export * from './token-map';
export * from './formats';

export class Tokenizer implements ITokenizer {
  generateMaps(id: string, data: string, format: string, options: Partial<IOptions>): ITokensMap[] {
    return createTokenMapBasedOnCode(id, data, format, options);
  }
}
