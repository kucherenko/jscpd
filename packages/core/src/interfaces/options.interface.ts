export interface IOptions {
	executionId?: string;
	minLines?: number;
	maxLines?: number;
	maxSize?: string;
	minTokens?: number;
	threshold?: number;
	formatsExts?: { [key: string]: string[] };
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
	list?: boolean;
	absolute?: boolean;
	noSymlinks?: boolean;
	skipLocal?: boolean;
	ignoreCase?: boolean;
	gitignore?: boolean;
	reportersOptions?: {
		[name: string]: any;
	};
	tokensToSkip?: string[];
}

export type TOption = keyof IOptions;
