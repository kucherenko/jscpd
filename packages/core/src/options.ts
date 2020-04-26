import {IOptions, TOption} from './interfaces';

export function getOption(name: TOption, options?: IOptions): any {
	const defaultOptions = getDefaultOptions();
	return options ? options[name] || defaultOptions[name] : defaultOptions[name];
}

export function getDefaultOptions(): IOptions {
	return {
		executionId: new Date().toISOString(),
		path: [process.cwd()],
		minLines: 5,
		maxLines: 500,
		maxSize: '30kb',
		minTokens: 50,
		output: './report',
		reporters: ['console'],
		ignore: [],
		mode: 'mild',
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
