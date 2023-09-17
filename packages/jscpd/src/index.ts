import { getDefaultOptions, IClone, IOptions, IStore, Statistic, IStatistic } from '@jscpd/core';
import { grey, italic } from 'colors/safe';
import { EntryWithContent, getFilesToDetect, InFilesDetector } from '@jscpd/finder';
import { initCli, initOptionsFromCli } from './init';
import { printFiles, printOptions, printSupportedFormat } from './print';
import { createHash } from "crypto";
import { getStore } from './init/store';
import { getSupportedFormats, IMapFrame, Tokenizer } from '@jscpd/tokenizer';
import { registerReporters } from './init/reporters';
import { registerSubscribers } from './init/subscribers';
import { registerHooks } from './init/hooks';

const TIMER_LABEL = 'Detection time:';

export const detectClones = (opts: IOptions, store: IStore<IMapFrame> | undefined = undefined, statisticInstance: Statistic | undefined = undefined): Promise<IClone[]> => {
  const options: Partial<IOptions> = {...getDefaultOptions(), ...opts};
  options.format = options.format || getSupportedFormats();

  const files: EntryWithContent[] = getFilesToDetect(options);
  const hashFunction = (value: string): string => {
    return createHash('md5').update(value).digest('hex')
  }
  options.hashFunction = options.hashFunction || hashFunction;
  const currentStore: IStore<IMapFrame> = store || getStore(options.store);
  const statistic = statisticInstance || new Statistic();
  const tokenizer = new Tokenizer();
  const detector = new InFilesDetector(tokenizer, currentStore, statistic, options);

  registerReporters(options, detector);
  registerSubscribers(options, detector);
  registerHooks(options, detector);

  if (!options.silent) {
    console.time(italic(grey(TIMER_LABEL)));
  }
  return detector.detect(files).then((clones: IClone[]) => {
    if (!options.silent) {
      console.timeEnd(italic(grey(TIMER_LABEL)));
    }
    return clones;
  });
}

export const detectClonesAndStatistic = (opts: IOptions, store: IStore<IMapFrame> | undefined = undefined): Promise<{clones: IClone[], statisticData: IStatistic}> => {
  const statistic =  new Statistic();
  return detectClones(opts, store, statistic).then((clones: IClone[]) => {
    return {clones, statisticData: statistic.getStatistic()};
  });
}

export async function jscpd(argv: string[], exitCallback?: (code: number) => {}) {
  const packageJson = require(__dirname + '/../package.json');

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
  } else {
    const store = getStore(options.store);
    return detectClones(options, store)
      .then((clones) => {
        if (clones.length > 0) {
          exitCallback?.(options.exitCode)
        }
        return clones;
      })
      .finally(() => {
        store.close();
      });
  }
}

