export interface IOptions {
	executionId?: string;
	minLines?: number;
	maxLines?: number;
	maxSize?: string;
	minTokens?: number;
	threshold?: number;
	formatsExts?: Record<string, string[]>;
	output?: string;
	path?: string[];
	mode?: any;
	config?: string;
	ignore?: string[];
	format?: string[];
	reporters?: string[];
	listeners?: string[];
	blame?: boolean;
	cache?: boolean;
	silent?: boolean;
	debug?: boolean;
	verbose?: boolean;
	list?: boolean;
	absolute?: boolean;
	noSymlinks?: boolean;
	skipLocal?: boolean;
	ignoreCase?: boolean;
	gitignore?: boolean;
	reportersOptions?: Record<string, any>;
	tokensToSkip?: string[];
	hashFunction?: (value: string) => string;
}

export type TOption = keyof IOptions;
