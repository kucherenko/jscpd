import { lstatSync, Stats } from 'fs';

export function isFile(path: string): boolean {
  try {
    const stat: Stats = lstatSync(path);
    return stat.isFile();
  } catch (e) {
    // lstatSync throws an error if path doesn't exist
    return false;
  }
}
