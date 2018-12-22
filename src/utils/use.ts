import { IMode } from '..';

const detectInstalled = require('detect-installed');

export enum ModuleType {
  reporter = 'reporter',
  mode = 'mode',
  db = 'db',
  tokenizer = 'tokenizer'
}

/**
 * import reporter
 * @param name
 * @deprecated
 */
export function useReporter(name: string) {
  return use(name, ModuleType.reporter);
}

/**
 * import mode
 * @param name
 * @deprecated
 */
export function useMode(name: string): IMode {
  return use(name, ModuleType.mode);
}

export function use<T>(name: string, type: ModuleType): T {
  const packageName = `jscpd-${name}-${type}`;
  if (!detectInstalled.sync(packageName, { local: true })) {
    throw new Error(
      `Module (type: ${type}) "${packageName}" does not found, please check that you have installed the package`
    );
  }
  return require(packageName).default;
}
