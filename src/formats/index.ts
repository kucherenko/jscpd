import { extname } from 'path';
import { FORMATS } from './formats';

export function getSupportedFormats(): string[] {
  return Object.keys(FORMATS).filter(name => name !== 'important' && name !== 'url');
}

export function getFormatByFile(path: string, formatsExts?: { [key: string]: string[] }): string | undefined {
  const ext: string = extname(path).slice(1);
  if (formatsExts && Object.keys(formatsExts).length) {
    return Object.keys(formatsExts).find(format => formatsExts[format].includes(ext));
  }
  return Object.keys(FORMATS).find(language => FORMATS[language].exts.includes(ext));
}
