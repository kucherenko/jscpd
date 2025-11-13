import {IOptions} from '@jscpd-ai/core';
import {bold, white} from 'colors/safe';

export function printOptions(options: IOptions): void {
  console.log(bold(white('Options:')));
  console.dir(options);
}
