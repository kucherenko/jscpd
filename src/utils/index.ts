import { bold, green } from 'colors/safe';
import { createHash } from 'crypto';
import { relative } from 'path';
import { cwd } from 'process';
import { IOptions } from '../interfaces/options.interface';
import { ISourceOptions } from '../interfaces/source-options.interface';
import { ITokenLocation } from '../interfaces/token/token-location.interface';

const ID_BLOCK_SEPARATOR = ':';

export function generateSourceId(source: ISourceOptions): string {
  return source.format + ID_BLOCK_SEPARATOR + source.id;
}

export function getPathBySourceId(id: string): string {
  const [, path] = id.split(ID_BLOCK_SEPARATOR);
  return path;
}

export function md5(value: string): string {
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
