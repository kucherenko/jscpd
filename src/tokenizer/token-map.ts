import { IMapFrame } from '../interfaces/map-frame.interface';
import { IToken } from '../interfaces/token/token.interface';
import { md5 } from '../utils';

const TOKEN_HASH_LENGTH = 20;

function createTokenHash(token: IToken): string {
  return md5(token.type + token.value).substr(0, TOKEN_HASH_LENGTH);
}

export class TokensMap implements Iterator<IMapFrame>, Iterable<IMapFrame> {
  private position: number = 0;
  private map: string;
  private sourceId!: string;

  constructor(private tokens: IToken[], private format: string, private minTokens: number) {
    this.map = this.tokens.map((token) => createTokenHash(token)).join('');
  }

  public getStartPosition(): number {
    return this.tokens[0].range[0];
  }

  public getEndPosition(): number {
    return this.tokens[this.getLength() - 1].range[1];
  }

  public getFormat() {
    return this.format;
  }

  public getLength(): number {
    return this.tokens.length;
  }

  public getSourceId(): string {
    return this.sourceId;
  }

  public setSourceId(id: string): void {
    this.sourceId = id;
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
    ).substring(0, 10);

    if (this.position < this.tokens.length - this.minTokens) {
      result = {
        done: false,
        value: {
          id: mapFrame,
          format: this.getFormat(),
          sourceId: this.getSourceId(),
          start: this.tokens[this.position],
          end: this.tokens[this.position + this.minTokens],
        },
      };
      this.position++;
    } else {
      result = {
        done: true,
        value: {} as IMapFrame,
      };
    }
    return result;
  }
}
