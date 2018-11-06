import { bold, green } from 'colors/safe';
import { createHash } from 'crypto';
import { relative } from 'path';
import { cwd } from 'process';
import { IOptions } from '..';
import { ITokenLocation } from '../interfaces/token/token-location.interface';

export function hash(value: string): string {
  return createHash('md5')
    .update(value)
    .digest('hex');
}

export function getPath(options: IOptions, path: string): string {
  return options.absolute ? path : relative(cwd(), path);
}

export function getPathConsoleString(options: IOptions, path: string): string {
  return bold(green(getPath(options, path)));
}

export function getSourceLocation(start: ITokenLocation, end: ITokenLocation): string {
  return `${start.line}:${start.column} - ${end.line}:${end.column}`;
}
