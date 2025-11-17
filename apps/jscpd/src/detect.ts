import {getDefaultOptions, IClone, IMapFrame, IOptions, IStore, Statistic} from '@jscpd/core';
import { grey, italic } from 'colors/safe';
import { EntryWithContent, getFilesToDetect, InFilesDetector } from '@jscpd/finder';
import { createHash } from "crypto";
import { getStore } from './setup/store';
import { getSupportedFormats, Tokenizer } from '@jscpd/tokenizer';
import { registerReporters } from './setup/reporters';
import { registerSubscribers } from './setup/subscribers';
import { registerHooks } from './setup/hooks';

const TIMER_LABEL = 'Detection time:';

export type DetectorContext = {
  options: IOptions;
  store: IStore<IMapFrame>;
  statistic: Statistic;
  tokenizer: Tokenizer;
  detector: InFilesDetector;
  files: EntryWithContent[];
};

export function createDetectorContext(
  opts: Partial<IOptions>,
  providedStore?: IStore<IMapFrame>
): DetectorContext {
  const options: IOptions = {...getDefaultOptions(), ...opts} as IOptions;
  options.format = options.format || getSupportedFormats();

  const hashFunction = (value: string): string => {
    return createHash('md5').update(value).digest('hex');
  };
  options.hashFunction = options.hashFunction || hashFunction;

  const files: EntryWithContent[] = getFilesToDetect(options);
  const store: IStore<IMapFrame> = providedStore || getStore(options.store);
  const statistic = new Statistic();
  const tokenizer = new Tokenizer();
  const detector = new InFilesDetector(tokenizer, store, statistic, options);

  registerReporters(options, detector);
  registerSubscribers(options, detector);
  registerHooks(options, detector);

  return { options, store, statistic, tokenizer, detector, files };
}

export const detectClones = (opts: IOptions, store: IStore<IMapFrame> | undefined = undefined): Promise<IClone[]> => {
  const context = createDetectorContext(opts, store);

  if (!context.options.silent) {
    console.time(italic(grey(TIMER_LABEL)));
  }
  return context.detector.detect(context.files).then((clones: IClone[]) => {
    if (!context.options.silent) {
      console.timeEnd(italic(grey(TIMER_LABEL)));
    }
    return clones;
  });
}

