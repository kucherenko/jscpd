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
  pattern?: string;
  ignorePattern?: string[];
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  mode?: any;
  config?: string;
  ignore?: string[];
  format?: string[];
  store?: string;
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
  skipIsolated?: string[][];
	ignoreCase?: boolean;
	gitignore?: boolean;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
	reportersOptions?: Record<string, any>;
	tokensToSkip?: string[];
	hashFunction?: (value: string) => string;
  exitCode?: number;
}

export type TOption = keyof IOptions;
