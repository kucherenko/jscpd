import {IToken} from '@jscpd/tokenizer';
import {IOptions} from './interfaces';

export type IMode = (token: IToken, options?: IOptions) => boolean;

export function strict(token: IToken): boolean {
	return token.type !== 'ignore';
}

export function mild(token: IToken): boolean {
	return strict(token) && token.type !== 'empty' && token.type !== 'new_line';
}

export function weak(token: IToken): boolean {
	return mild(token) && token.type !== 'comment' && token.type !== 'block-comment';
}

export function custom(token: IToken, options?: IOptions): boolean {
	if (!options || !options.hasOwnProperty('tokensToSkip')) {
		throw new Error('Mode `custom` need `tokensToSkip` option in config file');
	}
	const tokensToSkip = options.tokensToSkip || [];
	return !tokensToSkip.includes(token.type);
}

const MODES: { [name: string]: IMode } = {
	mild,
	strict,
	weak,
	custom,
};

export function getModeByName(name: string): IMode {
	if (MODES.hasOwnProperty(name)) {
		return MODES[name];
	}
	throw new Error(`Mode ${name} does not supported yet.`);
}

export function getModeHandler(mode: string | IMode): IMode {
	return typeof mode === 'string' ? getModeByName(mode) : mode;
}
