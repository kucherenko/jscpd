import { existsSync, readFileSync } from 'fs';
import { ISourceOptions } from '../interfaces/source-options.interface';

export function sourceToString(options: ISourceOptions): string {
  if (options.hasOwnProperty('source')) {
    return options.source || '';
  }
  if (existsSync(options.id)) {
    return readFileSync(options.id).toString();
  }
  return '';
}
