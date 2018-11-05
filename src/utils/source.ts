import { existsSync, readFileSync } from 'fs';
import { ISourceOptions } from '../interfaces/source-options.interface';
import { SOURCES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export function sourceToString(options: ISourceOptions): string {
  if (existsSync(options.id)) {
    return readFileSync(options.id).toString();
  }
  return StoresManager.getStore(SOURCES_DB).get(options.id);
}
