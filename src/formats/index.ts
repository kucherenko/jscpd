import { extname } from 'path';
import { FORMATS } from './formats';

export function getSupportedFormats(): string[] {
  return Object.keys(FORMATS);
}

export function getFormatByFile(path: string): string | undefined {
  const ext: string = extname(path).slice(1);
  return Object.keys(FORMATS).find(language =>
    FORMATS[language].exts.includes(ext)
  );
}
