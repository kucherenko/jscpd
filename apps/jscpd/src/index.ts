import {getDefaultOptions, IClone, IMapFrame, IOptions, IStore, Statistic} from '@jscpd/core';
import { grey, italic } from 'colors/safe';
import { EntryWithContent, getFilesToDetect, InFilesDetector } from '@jscpd/finder';
import { createHash } from "crypto";
import { getStore } from './init/store';
import { getSupportedFormats, Tokenizer } from '@jscpd/tokenizer';
import { registerReporters } from './init/reporters';
import { registerSubscribers } from './init/subscribers';
import { registerHooks } from './init/hooks';

const TIMER_LABEL = 'Detection time:';

export const detectClones = (opts: IOptions, store: IStore<IMapFrame> | undefined = undefined): Promise<IClone[]> => {
  const options: Partial<IOptions> = {...getDefaultOptions(), ...opts};
  options.format = options.format || getSupportedFormats();

  const files: EntryWithContent[] = getFilesToDetect(options);
  const hashFunction = (value: string): string => {
    return createHash('md5').update(value).digest('hex')
  }
  options.hashFunction = options.hashFunction || hashFunction;
  const currentStore: IStore<IMapFrame> = store || getStore(options.store);
  const statistic = new Statistic();
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

export async function jscpd(argv: string[], exitCallback?: (code: number) => {}) {
  const isServerMode = argv.includes('server');

  if (isServerMode) {
    const { runServer } = await import('./server-entry');
    return runServer(argv, exitCallback);
  }

  const { runCli } = await import('./cli-entry');
  return runCli(argv, exitCallback);
}

