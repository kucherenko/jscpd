import { IMapFrame } from './interfaces/map-frame.interface';
import { IOptions } from './interfaces/options.interface';
import { IToken } from './interfaces/token/token.interface';
import { md5 } from './utils';

const TOKEN_VALUE_HASH_LENGTH = 10;
const TOKEN_TYPE_HASH_LENGTH = 10;
const TOKEN_HASH_LENGTH = TOKEN_VALUE_HASH_LENGTH + TOKEN_TYPE_HASH_LENGTH;

function createTokenHash(token: IToken): string {
  return (
    md5(token.type).substr(0, TOKEN_TYPE_HASH_LENGTH) +
    md5(token.value).substr(0, TOKEN_VALUE_HASH_LENGTH)
  );
}

export class TokensMap implements Iterator<IMapFrame>, Iterable<IMapFrame> {
  private position: number = 0;
  private map: string;
  private readonly minTokens: number;

  constructor(
    private tokens: IToken[],
    private format: string,
    { minTokens }: IOptions
  ) {
    this.minTokens = minTokens;
    this.map = this.tokens.map(token => createTokenHash(token)).join('');
  }

  public getFormat() {
    return this.format;
  }

  public getLength(): number {
    return this.tokens.length;
  }

  public [Symbol.iterator](): Iterator<IMapFrame> {
    return this;
  }

  public next(): IteratorResult<IMapFrame> {
    let result: IteratorResult<IMapFrame>;
    const mapFrame: string = md5(
      this.map.substring(
        this.position * TOKEN_HASH_LENGTH,
        this.position * TOKEN_HASH_LENGTH + this.minTokens * TOKEN_HASH_LENGTH
      )
    );

    if (this.position < this.tokens.length - this.minTokens) {
      result = {
        done: false,
        value: {
          id: mapFrame,
          start: this.tokens[this.position],
          end: this.tokens[this.position + this.minTokens]
        }
      };
      this.position++;
    } else {
      result = {
        done: true,
        value: {} as IMapFrame
      };
    }
    return result;
  }
}
