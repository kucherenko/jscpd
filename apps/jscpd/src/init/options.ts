import {Command} from 'commander';
import {getModeHandler, IOptions} from '@jscpd-ai/core';
import {getSupportedFormats} from '@jscpd-ai/tokenizer';
import {initIgnore} from './ignore';
import {prepareOptions} from '../options';

export function initOptionsFromCli(cli: Command): IOptions {
	const options: IOptions = prepareOptions(cli);

	options.format = options.format || getSupportedFormats();

	options.mode = getModeHandler(options.mode);

	options.ignore = initIgnore(options);

	return options;
}
