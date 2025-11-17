import { IOptions } from '@jscpd/core';
import { readJSONSync } from 'fs-extra';
import { EntryWithContent, getFilesToDetect } from '@jscpd/finder';
import { initCli, initOptionsFromCli } from './init';
import { printFiles, printOptions, printSupportedFormat } from './print';
import { getStore } from './init/store';
import { detectClones } from './index';

export async function runCli(argv: string[], exitCallback?: (code: number) => {}): Promise<any[]> {
  const packageJson = readJSONSync(__dirname + '/../package.json');
  const cli = initCli(packageJson, argv);
  const options: IOptions = initOptionsFromCli(cli);

  if (options.list) {
    printSupportedFormat();
  }

  if (options.debug) {
    printOptions(options);
  }

  if (!options.path || options.path.length === 0) {
    options.path = [process.cwd()];
  }

  if (options.debug) {
    const files: EntryWithContent[] = getFilesToDetect(options);
    printFiles(files);
    return Promise.resolve([]);
  }

  const store = getStore(options.store);
  return detectClones(options, store)
    .then((clones) => {
      if (clones.length > 0) {
        exitCallback?.(options.exitCode || 0);
      }
      return clones;
    })
    .finally(() => {
      store.close();
    });
}

