import { IMode } from '../interfaces/mode.type';
import { useMode } from '../utils/use';
import { custom } from './custom';
import { mild } from './mild';
import { strict } from './strict';
import { weak } from './weak';

const MODES: { [name: string]: IMode } = {
  mild,
  strict,
  weak,
  custom,
};

export * from './strict';
export * from './mild';
export * from './weak';

export function getModeByName(name: string): IMode {
  if (MODES.hasOwnProperty(name)) {
    return MODES[name];
  }
  return useMode(name);
}

export function getModeHandler(mode: string | IMode): IMode {
  return typeof mode === 'string' ? getModeByName(mode) : mode;
}
