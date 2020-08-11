import {IOptions, TOption} from './interfaces';
import {getModeHandler} from "./mode";


export function getDefaultOptions(): IOptions {
  return {
    executionId: new Date().toISOString(),
    path: [process.cwd()],
    mode: getModeHandler('mild'),
    minLines: 5,
    maxLines: 1000,
    maxSize: '100kb',
    minTokens: 50,
    output: './report',
    reporters: ['console'],
    ignore: [],
    threshold: undefined,
    formatsExts: {},
    debug: false,
		silent: false,
		blame: false,
		cache: true,
		absolute: false,
		noSymlinks: false,
		skipLocal: false,
		ignoreCase: false,
		gitignore: false,
		reportersOptions: {},
	};
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getOption(name: TOption, options?: IOptions): any {
  const defaultOptions = getDefaultOptions();
  return options ? options[name] || defaultOptions[name] : defaultOptions[name];
}
