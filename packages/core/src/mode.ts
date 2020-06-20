import {IOptions, IToken} from './interfaces';

export type IMode = (token: IToken, options?: IOptions) => boolean;

export function strict(token: IToken): boolean {
	return token.type !== 'ignore';
}

export function mild(token: IToken): boolean {
	return strict(token) && token.type !== 'empty' && token.type !== 'new_line';
}

export function weak(token: IToken): boolean {
  return mild(token)
    && token.format !== 'comment'
    && token.type !== 'comment'
    && token.type !== 'block-comment';
}

const MODES: { [name: string]: IMode } = {
	mild,
	strict,
	weak,
};

export function getModeByName(name: string): IMode {
	if (name in MODES) {
		return MODES[name];
	}
	throw new Error(`Mode ${name} does not supported yet.`);
}

export function getModeHandler(mode: string | IMode): IMode {
	return typeof mode === 'string' ? getModeByName(mode) : mode;
}
