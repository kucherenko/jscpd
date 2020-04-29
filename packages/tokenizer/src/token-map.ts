import {IMapFrame, IToken} from './interfaces';
import {hash} from './hash';

const TOKEN_HASH_LENGTH = 20;

function createTokenHash(token: IToken, hashFunction: (value: string) => string | undefined = undefined): string {
	return hashFunction ?
		hashFunction(token.type + token.value).substr(0, TOKEN_HASH_LENGTH) :
		hash(token.type + token.value).substr(0, TOKEN_HASH_LENGTH);
}

function groupByFormat(tokens: IToken[]): { [key: string]: IToken[] } {
	const result: { [key: string]: IToken[] } = {};
	// TODO change to reduce
	tokens.forEach((token: IToken) => {
		(result[token.format] = result[token.format] ? [...result[token.format], token] : [token])
	});
	return result;
}

export class TokensMap implements Iterator<IMapFrame|boolean>, Iterable<IMapFrame|boolean> {
	private position = 0;
	private hashMap: string;

	constructor(
		private readonly id: string,
		private readonly data: string,
		private readonly tokens: IToken[],
		private readonly format: string,
		private readonly options) {
		this.hashMap = this.tokens.map((token) => {
			if (options.ignoreCase) {
				token.value = token.value.toLocaleLowerCase()
			}
			return createTokenHash(token, this.options.hashFunction)
		}).join('');
	}

	public getId(): string {
		return this.id;
	}

	public getLinesCount(): number {
		return this.tokens[this.tokens.length - 1].loc.end.line - this.tokens[0].loc.start.line;
	}

	public getData(): string {
		return this.data;
	}

	public getStartPosition(): number {
		return this.tokens[0].range[0];
	}

	public getEndPosition(): number {
		return this.tokens[this.getLength() - 1].range[1];
	}

	public getFormat(): string {
		return this.format;
	}

	public getLength(): number {
		return this.tokens.length;
	}

	public [Symbol.iterator](): Iterator<IMapFrame|boolean> {
		return this;
	}

	public next(): IteratorResult<IMapFrame | boolean> {
		const hashFunction: (value: string) => string = this.options.hashFunction ? this.options.hashFunction : hash;
		const mapFrame: string = hashFunction(
			this.hashMap.substring(
				this.position * TOKEN_HASH_LENGTH,
				this.position * TOKEN_HASH_LENGTH + this.options.minTokens * TOKEN_HASH_LENGTH,
			),
		).substring(0, TOKEN_HASH_LENGTH);

		if (this.position < this.tokens.length - this.options.minTokens) {
			this.position++;
			return {
				done: false,
				value: {
					id: mapFrame,
					sourceId: this.getId(),
					start: this.tokens[this.position - 1],
					end: this.tokens[this.position + this.options.minTokens - 1],
				},
			};

		} else {
			return {
				done: true,
				value: false,
			};
		}
	}
}

export function generateMapsForFormats(id: string, data: string, tokens: IToken[], options): TokensMap[] {
  return Object
    .values(groupByFormat(tokens))
    .map((formatTokens: IToken[]) => new TokensMap(id, data, formatTokens, formatTokens[0].format, options));
}

export function createTokensMaps(id: string, data: string, tokens: IToken[], options): TokensMap[] {
  return generateMapsForFormats(id, data, tokens, options);
}
