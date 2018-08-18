import { createHash } from 'crypto';
import { ISource } from '../interfaces/source.interface';

export function generateHashForSource(source: ISource): string {
  return md5(source.id + source.source).substr(0, 10);
}

export function md5(value: string): string {
  return createHash('md5')
    .update(value)
    .digest('hex');
}
