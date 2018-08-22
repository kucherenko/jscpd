import { bold, green } from 'colors/safe';
import { createHash } from 'crypto';
import { relative } from "path";
import { IOptions } from '../interfaces/options.interface';
import { ISource } from '../interfaces/source.interface';
import { ITokenLocation } from '../interfaces/token/token-location.interface';

export function generateHashForSource(source: ISource): string {
  return md5(source.id + source.source).substr(0, 10);
}

export function md5(value: string): string {
  return createHash('md5')
    .update(value)
    .digest('hex');
}

export function getPath(options: IOptions, path: string): string {
  return options.absolute ? path : relative(options.path, path);
}

export function getPathConsoleString(options: IOptions, path: string): string {
  return bold(green(getPath(options, path)));
}


export function getSourceLocation(start: ITokenLocation, end: ITokenLocation): string {
  return `${start.line}:${start.column} - ${end.line}:${end.column}`;
}
