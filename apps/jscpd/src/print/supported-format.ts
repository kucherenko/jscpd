import {bold, white} from 'colors/safe';
import {getSupportedFormats} from '@jscpd-ai/tokenizer';

export function printSupportedFormat(): void {
	console.log(bold(white('Supported formats: ')));
	console.log(getSupportedFormats().join(', '));
	process.exit(0);
}
