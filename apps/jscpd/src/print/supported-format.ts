import {bold, white} from 'colors/safe';
import {getSupportedFormats} from '@jscpd/tokenizer';

export function printSupportedFormat(): void {
	console.log(bold(white('Supported formats: ')));
	console.log(getSupportedFormats().join(', '));
	process.exit(0);
}
